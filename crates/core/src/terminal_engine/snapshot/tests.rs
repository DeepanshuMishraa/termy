use super::super::{Engine, Options, Size};
use super::{SNAPSHOT_VERSION, SnapshotError, snapshot_version};

fn engine(cols: usize, rows: usize, history: usize) -> Engine {
    Engine::new(
        Size { cols, rows },
        Options {
            scrollback_history: history,
        },
    )
}

fn roundtrip(engine: &mut Engine) -> Engine {
    let bytes = engine.encode_snapshot();
    Engine::decode_snapshot(&bytes).expect("decode snapshot")
}

/// Feed `tail` to `engine` and to a copy decoded from its snapshot, and
/// require identical states (and replies) afterwards.
fn assert_continues_identically(engine: &mut Engine, tail: &[u8], context: &str) {
    let mut decoded = roundtrip(engine);
    assert_eq!(
        decoded.canonical_snapshot(),
        engine.canonical_snapshot(),
        "decoded state differs before the tail ({context})"
    );
    engine.feed(tail);
    decoded.feed(tail);
    // The decoded engine ran no synchronized update timer; commit both.
    engine.stop_synchronized_update();
    decoded.stop_synchronized_update();
    let (mut expected, mut actual) = (Vec::new(), Vec::new());
    engine.drain_replies(&mut expected);
    decoded.drain_replies(&mut actual);
    assert_eq!(actual, expected, "replies differ ({context})");
    assert_eq!(
        decoded.canonical_snapshot(),
        engine.canonical_snapshot(),
        "states differ after the tail ({context})"
    );
    for line in -(engine.history_size() as i32)..engine.size().rows as i32 {
        assert_eq!(
            decoded.line(line),
            engine.line(line),
            "line {line} ({context})"
        );
        assert_eq!(decoded.line_wrapped(line), engine.line_wrapped(line));
    }
    assert_eq!(decoded.cursor(), engine.cursor());
    assert_eq!(decoded.modes(), engine.modes());
    assert_eq!(decoded.title(), engine.title());
    assert_eq!(decoded.working_directory(), engine.working_directory());
}

/// A deterministic stream exercising most of the parser and grid.
fn corpus() -> Vec<u8> {
    let mut out = Vec::new();
    let pieces: &[&[u8]] = &[
        b"plain ascii line\r\n",
        b"\x1b[1;31mbold red\x1b[0m \x1b[4:3;58;2;10;20;30mcurly\x1b[24;59m\r\n",
        "wide \u{4e2d}\u{6587} and e\u{301}\u{302} \u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467} flag \u{1f1e9}\u{1f1ea}\r\n"
            .as_bytes(),
        b"\x1b]8;id=a;https://example.com\x07link\x1b]8;;\x07 after\r\n",
        b"\x1b]0;window title\x07\x1b[22;0t\x1b]2;second\x07",
        b"\x1b]7;file://host/tmp/some%20dir\x07",
        b"\x1b]4;1;rgb:12/34/56\x07\x1b]10;#abcdef\x07\x1b]11;rgb:01/02/03\x1b\\",
        b"\x1b[?1h\x1b=\x1b[?2004h\x1b[?1004h\x1b[?1002h\x1b[?1006h\x1b[>5u\x1b[?7727h",
        b"\x1b(0lqqk\x1b(B\x0e\x0f",
        b"\x1b[3;10r\x1b[5;1Htext in region\x1bD\x1bM\x1b[r",
        b"\x1b[?69h\x1b[5;40s\x1b[2;3Hmargins\x1b[?69l",
        b"\x1b7\x1b[10;10Hsaved\x1b8",
        b"\x1b[3g\x1b[1;5H\x1bH\x1b[1;13H\x1bH\r\tA\tB\r\n",
        b"\x1b[?1049hALT SCREEN\x1b[5;5Hx\x1b[>1u",
        b"\x1b[?1049l",
        b"\x1b[?6h\x1b[2;2Horigin\x1b[?6l\x1b[4hins\x1b[4l\x1b[20h\x1b[20l",
        b"\x1b[2 q\x1b[?12h\x1b[?25l\x1b[?25h",
        b"\x1b]7501;id=a;state=working;app=test\x1b\\",
        b"\x1b[5ba\x1b[3brepeat\r\n",
        b"\x1b[38;5;200;48;2;1;2;3mcolored\x1b[K\x1b[0m\r\n",
        b"\x1b[2J\x1b[H",
        b"line that is long enough to wrap around the right margin of the screen ok\r\n",
        b"\x1b[6n\x1b[c\x1b[?2004$p\x1b]10;?\x07\x1bP$qm\x1b\\",
        b"\x1b[?2026hsynchronized \x1b[1mtext",
        b"\x1b[?2026l",
        b"\x1b[3Sscrolled\x1b[2T\x1b[2L\x1b[M\x1b[3@\x1b[2P\x1b[4X",
        "\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\r\n".as_bytes(),
        b"\x1b#8\x1b[H\x1b[J",
    ];
    for round in 0..6 {
        for piece in pieces {
            out.extend_from_slice(piece);
        }
        for line in 0..12 {
            out.extend_from_slice(
                format!("round {round} line {line} \x1b[3{}m", line % 8).as_bytes(),
            );
            out.extend_from_slice(b"text\r\n");
        }
    }
    out
}

/// A small deterministic generator, so failures reproduce.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

fn random_stream(random: &mut Lcg, len: usize) -> Vec<u8> {
    const ALPHABET: &[&[u8]] = &[
        b"a",
        b"Z",
        b" ",
        b"\r",
        b"\n",
        b"\t",
        b"\x08",
        b"\x1b",
        b"[",
        b"]",
        b";",
        b"?",
        b"0",
        b"1",
        b"2",
        b"4",
        b"7",
        b"m",
        b"H",
        b"J",
        b"K",
        b"h",
        b"l",
        b"r",
        b"P",
        b"\\",
        b"\x07",
        b"(",
        b"q",
        b"u",
        b">",
        b"8",
        b":",
        "\u{301}".as_bytes(),
        "\u{4e2d}".as_bytes(),
        "\u{1f600}".as_bytes(),
        b"\xe2",
        b"\x80",
        b"\xf0\x9f",
        b"\x1b[",
        b"\x1b]",
        b"\x1bP",
        b"\x1b_G",
        b"\x1b[?2026h",
        b"\x1b[?1049h",
        b"\x1b[?1049l",
        b"\x1b]8;;http://x\x07",
        b"\x1b]8;;\x07",
        b"\x1b[38;2;1;2;3m",
        b"\x1b[0m",
        b"\x1b[?69h",
        b"\x1b[2;9s",
    ];
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        out.extend_from_slice(ALPHABET[random.below(ALPHABET.len())]);
    }
    out
}

#[test]
fn snapshot_has_versioned_header() {
    let bytes = engine(20, 5, 10).encode_snapshot();
    assert_eq!(&bytes[..8], b"TRMYSNAP");
    assert_eq!(snapshot_version(&bytes), Ok(SNAPSHOT_VERSION));

    let mut future = bytes.clone();
    future[8..12].copy_from_slice(&(SNAPSHOT_VERSION + 1).to_le_bytes());
    assert_eq!(
        Engine::decode_snapshot(&future).err(),
        Some(SnapshotError::UnsupportedVersion(SNAPSHOT_VERSION + 1))
    );
    assert_eq!(
        Engine::decode_snapshot(b"not a snapshot at all").err(),
        Some(SnapshotError::NotASnapshot)
    );
    assert_eq!(
        Engine::decode_snapshot(&bytes[..10]).err(),
        Some(SnapshotError::Truncated)
    );
}

#[test]
fn truncated_or_damaged_snapshots_are_rejected_without_panicking() {
    let mut source = engine(30, 6, 50);
    source.feed(&corpus());
    let bytes = source.encode_snapshot();
    for len in (0..bytes.len()).step_by(7) {
        assert!(
            Engine::decode_snapshot(&bytes[..len]).is_err(),
            "length {len}"
        );
    }
    let mut random = Lcg(7);
    for _ in 0..400 {
        let mut damaged = bytes.clone();
        let index = 24 + random.below(damaged.len() - 24);
        damaged[index] ^= 1 << random.below(8);
        // Either rejected or a structurally valid engine that can keep going.
        if let Ok(mut decoded) = Engine::decode_snapshot(&damaged) {
            decoded.feed(b"\x1b[2J\x1b[Hstill alive\r\n\x1b[?1049h\x1b[?1049l");
            decoded.resize(Size { cols: 12, rows: 3 });
            decoded.feed(b"more text after resize\r\n");
        }
    }
}

#[test]
fn corpus_split_at_every_boundary_continues_identically() {
    let corpus = corpus();
    // Every split of the first rounds, then a stride through the rest.
    let splits = (0..corpus.len() / 3).chain((corpus.len() / 3..corpus.len()).step_by(17));
    for split in splits {
        let mut source = engine(40, 8, 64);
        source.feed(&corpus[..split]);
        let mut sink = Vec::new();
        source.drain_replies(&mut sink);
        assert_continues_identically(&mut source, &corpus[split..], &format!("split {split}"));
    }
}

#[test]
fn random_streams_split_at_random_points_continue_identically() {
    let mut random = Lcg(0x5eed);
    for case in 0..300 {
        let cols = 4 + random.below(60);
        let rows = 2 + random.below(14);
        let history = random.below(80);
        let len = 64 + random.below(2048);
        let stream = random_stream(&mut random, len);
        let split = random.below(stream.len() + 1);
        let mut source = engine(cols, rows, history);
        // Feed the prefix in random fragments, as a PTY would.
        let mut offset = 0;
        while offset < split {
            let end = (offset + 1 + random.below(64)).min(split);
            source.feed(&stream[offset..end]);
            offset = end;
        }
        if random.below(4) == 0 {
            source.compact_history();
        }
        let mut sink = Vec::new();
        source.drain_replies(&mut sink);
        assert_continues_identically(
            &mut source,
            &stream[split..],
            &format!("case {case}, {cols}x{rows}, history {history}, split {split}"),
        );
    }
}

#[test]
fn resize_and_scrolling_after_decode_match() {
    let mut source = engine(50, 10, 200);
    source.feed(&corpus());
    source.scroll_display(7);
    let mut decoded = roundtrip(&mut source);
    assert_eq!(decoded.display_offset(), 7);
    for engine in [&mut source, &mut decoded] {
        engine.resize(Size { cols: 23, rows: 7 });
        engine.feed(b"after resize \xe4\xb8\xad\r\n");
        engine.resize(Size { cols: 61, rows: 12 });
    }
    assert_eq!(decoded.canonical_snapshot(), source.canonical_snapshot());
}

#[test]
fn packed_history_keeps_its_representation_and_charge() {
    let mut source = engine(120, 10, 2000);
    for line in 0..1500 {
        source.feed(format!("\x1b[3{}mhistory line {line} \u{e9}\u{301}\r\n", line % 8).as_bytes());
    }
    source.compact_history();
    let decoded = roundtrip(&mut source);
    assert_eq!(
        decoded.state.grid.history_bytes(),
        source.state.grid.history_bytes()
    );
    assert_eq!(decoded.history_size(), source.history_size());
}

#[test]
fn large_scrollback_encodes_quickly() {
    // About 16 MiB of dense scrollback: 20,000 rows of 100 cells.
    let mut source = engine(100, 50, 20_000);
    let line: Vec<u8> = (0..99).map(|index| b'!' + (index % 90) as u8).collect();
    for row in 0..20_050 {
        if row % 50 == 0 {
            source.feed(b"\x1b[1;32m");
        }
        source.feed(&line);
        source.feed(b"\r\n");
    }
    let started = std::time::Instant::now();
    let bytes = source.encode_snapshot();
    let encode = started.elapsed();
    let started = std::time::Instant::now();
    let decoded = Engine::decode_snapshot(&bytes).expect("decode");
    let decode = started.elapsed();
    assert_eq!(decoded.history_size(), source.history_size());
    eprintln!(
        "snapshot of {} history rows: {} bytes, encode {encode:?}, decode {decode:?}",
        source.history_size(),
        bytes.len()
    );
    // Generous for unoptimized test builds; release builds are far faster.
    assert!(encode.as_secs() < 5, "{encode:?}");
}

#[test]
fn history_byte_budget_evicts_identically_after_decode() {
    // 300 dense columns cross the 16 MiB history budget well before the
    // row limit, so eviction depends on how every row is charged.
    let mut source = engine(300, 8, 20_000);
    let line: Vec<u8> = (0..299).map(|index| b'#' + (index % 80) as u8).collect();
    let feed = |engine: &mut Engine, rows: std::ops::Range<usize>| {
        for row in rows {
            engine.feed(format!("\x1b[3{}m", row % 8).as_bytes());
            engine.feed(&line);
            engine.feed(b"\r\n");
        }
    };
    feed(&mut source, 0..3000);
    source.compact_history();
    feed(&mut source, 3000..3500);
    let mut decoded = roundtrip(&mut source);
    for engine in [&mut source, &mut decoded] {
        feed(engine, 3500..9000);
        engine.compact_history();
        feed(engine, 9000..14_000);
    }
    // The budget, not the row limit, bounded history.
    assert!(
        source.history_size() < 14_000 - 8,
        "{}",
        source.history_size()
    );
    assert_eq!(decoded.history_size(), source.history_size());
    assert_eq!(
        decoded.state.grid.history_bytes(),
        source.state.grid.history_bytes()
    );
    assert_eq!(decoded.canonical_snapshot(), source.canonical_snapshot());
}
