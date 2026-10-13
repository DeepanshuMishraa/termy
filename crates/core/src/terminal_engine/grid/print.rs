//! Streaming graphemes and the ASCII output fast path.
use super::super::parser::complete_utf8;
use super::super::types::{CellExtra, ExtraSlot, MAX_COMBINING_BYTES, PlainCell};
use super::{Cell, Grid};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// The display width of `cell` with its `combining` text.
pub(super) fn cell_width(cell: &Cell, combining: &str) -> usize {
    if combining.is_empty() {
        return cell.character.width().unwrap_or(0);
    }
    let mut bytes = [0; MAX_COMBINING_BYTES + 4];
    let len = cell.character.encode_utf8(&mut bytes).len();
    bytes[len..len + combining.len()].copy_from_slice(combining.as_bytes());
    std::str::from_utf8(&bytes[..len + combining.len()])
        .expect("cell UTF-8")
        .width()
}

// These base characters have a grapheme break between one another, even
// after a suffix without a trailing ZWJ. Hangul Jamo, Indic scripts, prepend
// marks, regional indicators, and emoji modifiers use the segmentation tables.
// Box drawing, braille, arrows, private-use (Powerline/Nerd Font) glyphs, CJK
// punctuation and fullwidth forms are common in TUIs, so they stay on this path,
// as do Hebrew, Arabic (minus prepend marks) and Thai consonants. Indic
// consonants and independent vowels too: a virama between two consonants joins
// them (GB9c), so appending one ends the fast path (`ends_ordinary_run`).
#[inline(always)]
fn ordinary_base(c: char) -> bool {
    // Branch on the block first so CJK and Hangul text, the common non-ASCII
    // case, tests only the ranges of its own plane.
    match c {
        '\u{3400}'..='\u{9fff}' | '\u{ac00}'..='\u{d7a3}' => true,
        '\u{900}'..='\u{dff}' => indic_base(c),
        '\u{0}'..='\u{2fff}' => {
            matches!(c, '\u{20}'..='\u{2ff}' | '\u{370}'..='\u{482}' | '\u{48a}'..='\u{52f}' | '\u{5d0}'..='\u{5f2}' | '\u{620}'..='\u{64a}' | '\u{660}'..='\u{66f}' | '\u{671}'..='\u{6d3}' | '\u{6d5}' | '\u{6ee}'..='\u{6ff}' | '\u{e01}'..='\u{e2e}' | '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205e}' | '\u{20a0}'..='\u{20cf}' | '\u{2100}'..='\u{2bff}')
        }
        '\u{3000}'..='\u{ffff}' => {
            matches!(c, '\u{3000}'..='\u{3029}' | '\u{3030}'..='\u{303f}' | '\u{3041}'..='\u{3096}' | '\u{309b}'..='\u{30ff}' | '\u{e000}'..='\u{f8ff}' | '\u{ff01}'..='\u{ff9d}' | '\u{ffa0}'..='\u{ffef}')
        }
        _ => {
            matches!(c, '\u{1f300}'..='\u{1f3fa}' | '\u{1f400}'..='\u{1faff}' | '\u{f0000}'..='\u{10fffd}')
        }
    }
}

/// Consonants and independent vowels of the major Indic scripts, excluding
/// prepend marks (e.g. Malayalam dot reph). Out of line so the common
/// scripts keep `ordinary_base` small enough to inline.
#[inline(never)]
fn indic_base(c: char) -> bool {
    matches!(c,
        // Devanagari, Bengali, Gurmukhi
        '\u{904}'..='\u{939}' | '\u{958}'..='\u{961}' | '\u{972}'..='\u{97f}'
        | '\u{985}'..='\u{98c}' | '\u{98f}'..='\u{990}' | '\u{993}'..='\u{9a8}' | '\u{9aa}'..='\u{9b0}' | '\u{9b2}' | '\u{9b6}'..='\u{9b9}' | '\u{9dc}'..='\u{9dd}' | '\u{9df}'..='\u{9e1}' | '\u{9f0}'..='\u{9f1}'
        | '\u{a05}'..='\u{a0a}' | '\u{a0f}'..='\u{a10}' | '\u{a13}'..='\u{a28}' | '\u{a2a}'..='\u{a30}' | '\u{a32}'..='\u{a33}' | '\u{a35}'..='\u{a36}' | '\u{a38}'..='\u{a39}' | '\u{a59}'..='\u{a5c}' | '\u{a5e}'
        // Gujarati, Oriya, Tamil
        | '\u{a85}'..='\u{a8d}' | '\u{a8f}'..='\u{a91}' | '\u{a93}'..='\u{aa8}' | '\u{aaa}'..='\u{ab0}' | '\u{ab2}'..='\u{ab3}' | '\u{ab5}'..='\u{ab9}' | '\u{ae0}'..='\u{ae1}' | '\u{af9}'
        | '\u{b05}'..='\u{b0c}' | '\u{b0f}'..='\u{b10}' | '\u{b13}'..='\u{b28}' | '\u{b2a}'..='\u{b30}' | '\u{b32}'..='\u{b33}' | '\u{b35}'..='\u{b39}' | '\u{b5c}'..='\u{b5d}' | '\u{b5f}'..='\u{b61}' | '\u{b71}'
        | '\u{b85}'..='\u{b8a}' | '\u{b8e}'..='\u{b90}' | '\u{b92}'..='\u{b95}' | '\u{b99}'..='\u{b9a}' | '\u{b9c}' | '\u{b9e}'..='\u{b9f}' | '\u{ba3}'..='\u{ba4}' | '\u{ba8}'..='\u{baa}' | '\u{bae}'..='\u{bb9}'
        // Telugu, Kannada, Malayalam
        | '\u{c05}'..='\u{c0c}' | '\u{c0e}'..='\u{c10}' | '\u{c12}'..='\u{c28}' | '\u{c2a}'..='\u{c39}' | '\u{c58}'..='\u{c5a}' | '\u{c60}'..='\u{c61}'
        | '\u{c85}'..='\u{c8c}' | '\u{c8e}'..='\u{c90}' | '\u{c92}'..='\u{ca8}' | '\u{caa}'..='\u{cb3}' | '\u{cb5}'..='\u{cb9}' | '\u{ce0}'..='\u{ce1}'
        | '\u{d05}'..='\u{d0c}' | '\u{d0e}'..='\u{d10}' | '\u{d12}'..='\u{d3a}' | '\u{d60}'..='\u{d61}')
}

/// Indic viramas (canonical combining class 9). One may join the next
/// consonant to the current cluster, so ordinary bases must be segmented.
fn ends_ordinary_run(c: char) -> bool {
    matches!(
        c,
        '\u{94d}'
            | '\u{9cd}'
            | '\u{a4d}'
            | '\u{acd}'
            | '\u{b4d}'
            | '\u{bcd}'
            | '\u{c4d}'
            | '\u{ccd}'
            | '\u{d3b}'
            | '\u{d3c}'
            | '\u{d4d}'
            | '\u{dca}'
    )
}

/// These common character ranges have uniform scalar properties. Resolve both
/// properties together instead of looking up width and classifying again.
/// Every printed scalar takes this path, so keep it inlined.
#[inline(always)]
fn scalar_properties(character: char) -> (usize, bool) {
    match character {
        '\u{300}'..='\u{36f}' => (0, false),
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{ac00}'..='\u{d7a3}' => (2, true),
        '\u{c0}'..='\u{2ff}' | '\u{370}'..='\u{482}' | '\u{48a}'..='\u{52f}' => (1, true),
        // Emoticons, all with emoji presentation.
        '\u{1f600}'..='\u{1f64f}' => (2, true),
        _ => {
            let width = character.width().unwrap_or(0);
            (width, width != 0 && ordinary_base(character))
        }
    }
}

/// Extend, ZWJ and spacing marks join any preceding base, so checking them
/// against one ASCII letter avoids segmenting a long retained cluster.
#[cold]
#[inline(never)]
fn always_extends(c: char) -> bool {
    let mut bytes = [b'a'; 5];
    let len = 1 + c.encode_utf8(&mut bytes[1..]).len();
    std::str::from_utf8(&bytes[..len])
        .expect("grapheme UTF-8")
        .graphemes(true)
        .nth(1)
        .is_none()
}

/// A three-byte UTF-8 scalar in the wide ordinary blocks `scalar_properties`
/// resolves first (CJK ideographs, extension A, Hangul syllables).
#[inline(always)]
fn wide_cjk(lead: u8, second: u8, third: u8) -> Option<char> {
    wide_cjk_word(u32::from_le_bytes([lead, second, third, 0]))
}

/// `wide_cjk` of the low three bytes of `word`, read little-endian; the
/// high byte is ignored.
#[inline(always)]
fn wide_cjk_word(word: u32) -> Option<char> {
    // The lead and continuation bit patterns are checked at once:
    // 1110xxxx 10xxxxxx 10xxxxxx.
    if word & 0x00c0_c0f0 != 0x0080_80e0 {
        return None;
    }
    let value = (word & 0x0f) << 12 | (word & 0x3f00) >> 2 | (word >> 16) & 0x3f;
    // Values from 0x800 can't be overlong, and these blocks hold no
    // surrogates, so each is a scalar encoded canonically. Ideographs come
    // first: they are most of this text.
    (matches!(value, 0x4e00..=0x9fff) || matches!(value, 0x3400..=0x4dbf | 0xac00..=0xd7a3))
        // SAFETY: every value in these blocks is a Unicode scalar value.
        .then(|| unsafe { char::from_u32_unchecked(value) })
}

/// A two-byte UTF-8 scalar in the narrow ordinary blocks `scalar_properties`
/// resolves first (Latin, Greek, Cyrillic).
#[inline(always)]
fn narrow_two_byte(lead: u8, second: u8) -> Option<char> {
    if lead & 0xe0 != 0xc0 || second & 0xc0 != 0x80 {
        return None;
    }
    let value = u32::from(lead & 0x1f) << 6 | u32::from(second & 0x3f);
    // Overlong leads (0xc0, 0xc1) decode below these blocks, and two bytes
    // encode no surrogates.
    matches!(value, 0xc0..=0x2ff | 0x370..=0x482 | 0x48a..=0x52f)
        // SAFETY: every value in these blocks is a Unicode scalar value.
        .then(|| unsafe { char::from_u32_unchecked(value) })
}

/// Store the leading run of `bytes` that `wide_cjk` accepts as wide cells
/// from `col`, stopping before the last cell of `cells` (where `put_cell`
/// handles the margin) and before overwriting half of a wide glyph (cells
/// from `occupied` on are erase blanks). Returns the column and byte count
/// reached. For runs of at least four scalars, most of which it stores four
/// at a time; out of line, so its loops keep the cell templates in
/// registers rather than reloading them per glyph.
#[inline(never)]
fn write_wide_cjk(
    cells: &mut [Cell],
    col: usize,
    occupied: usize,
    bytes: &[u8],
    wide: PlainCell,
) -> (usize, usize) {
    let spacer = wide.with_flags(Cell::WIDE_SPACER);
    // Whole pairs that end before the last cell, and whole scalars: with
    // the trip count fixed up front, the loops index nothing.
    let room = cells.len().saturating_sub(col + 1) / 2;
    let (pairs, _) = cells[col..].as_chunks_mut::<2>();
    let (scalars, _) = bytes.as_chunks::<3>();
    let count = room.min(scalars.len());
    // Pairs from this one on start at or past `occupied`.
    let checked = occupied.saturating_sub(col).div_ceil(2);
    // Groups skip the repair check, so they start only past `occupied`.
    let mut written = 0;
    if checked == 0 {
        written = super::cjk::store_quads(&mut pairs[..count], bytes, wide, spacer);
    }
    let written = write_wide_scalars(pairs, scalars, written..count, checked, wide, spacer);
    (col + 2 * written, 3 * written)
}

/// `write_wide_cjk`'s loop over the pairs and scalars of `range`, returning
/// where it stopped. Pairs before `checked` must not hold half of a glyph.
#[inline(always)]
fn write_wide_scalars(
    pairs: &mut [[Cell; 2]],
    scalars: &[[u8; 3]],
    range: std::ops::Range<usize>,
    checked: usize,
    wide: PlainCell,
    spacer: PlainCell,
) -> usize {
    let mut written = range.start;
    for ([glyph, right_half], &[lead, second, third]) in
        pairs[range.clone()].iter_mut().zip(&scalars[range])
    {
        let Some(character) = wide_cjk(lead, second, third) else {
            break;
        };
        if written < checked && (glyph.flags | right_half.flags) != 0 {
            break;
        }
        wide.store(glyph, character);
        spacer.store(right_half, ' ');
        written += 1;
    }
    written
}

impl Grid {
    fn extend_grapheme(&mut self, character: char) -> bool {
        if !self.grapheme_open {
            return false;
        }
        let row = self.cursor.row;
        let mut col = self.cursor.col;
        if !self.pending_wrap && !(self.grapheme_open && self.grapheme_at_margin) {
            let Some(previous) = col.checked_sub(1) else {
                return false;
            };
            col = previous;
        }
        if self.screen().rows[row].cells[col].flags & Cell::WIDE_SPACER != 0 {
            col = col.saturating_sub(1);
        }
        let previous = self.screen().rows[row].cells[col];
        let mut extra = self.screen().rows[row].extra(col).cloned();
        // Match the existing bound on untrusted combining sequences. Discard
        // excess suffixes without moving the cursor or splitting a cluster.
        let over_limit = extra.combining().len() + character.len_utf8() > MAX_COMBINING_BYTES;
        if over_limit && always_extends(character) {
            return true;
        }
        let mut bytes = [0; MAX_COMBINING_BYTES + 8];
        let mut len = previous.character.encode_utf8(&mut bytes).len();
        bytes[len..len + extra.combining().len()].copy_from_slice(extra.combining().as_bytes());
        len += extra.combining().len();
        len += character.encode_utf8(&mut bytes[len..]).len();
        let text = std::str::from_utf8(&bytes[..len]).expect("grapheme UTF-8");
        if text.graphemes(true).nth(1).is_some() {
            return false;
        }
        if over_limit {
            return true;
        }
        let mut width = text.width().clamp(1, 2).min(self.size.cols);
        let old_width = 1 + usize::from(previous.flags & Cell::WIDE != 0);
        let right = if col < self.right_margin {
            self.right_margin
        } else {
            self.size.cols
        };
        if !self.autowrap && col + width > right {
            width = old_width;
        }
        let cell = previous;
        self.combining_cache.append(&mut extra, character);
        if width == old_width {
            let active = &mut self.screen_mut().rows[row];
            active.put(col, cell, extra);
            active.occupied = active.occupied.max(col + width);
            self.mark(row, col, col + width);
            return true;
        }
        let old_cursor = self.cursor;
        self.cursor.col = col;
        self.pending_wrap = false;
        let insert_mode = self.insert_mode;
        if insert_mode && width > old_width && col + width <= self.size.cols {
            self.cursor.col += old_width;
            self.insert_chars(width - old_width);
            self.cursor.col = col;
        }
        let blank = self.blank();
        let (cleared_start, cleared_end) =
            Self::clear_wide_at(&mut self.screen_mut().rows[row], col, &blank);
        // A narrower grapheme leaves an erased spacer outside the new write.
        // Mark it before put_cell can wrap or scroll, even with a hidden cursor.
        self.mark(row, cleared_start, cleared_end);
        if insert_mode && width < old_width {
            self.cursor.col = col + width;
            self.delete_chars(old_width - width);
            self.cursor.col = col;
        }
        self.insert_mode = false;
        self.put_cell(cell, extra, width);
        self.insert_mode = insert_mode;
        self.mark_cursor(old_cursor);
        true
    }

    pub(in crate::terminal_engine) fn put_char(&mut self, character: char) {
        self.observe_output();
        let (width, ordinary) = scalar_properties(character);
        if (width != 0 || matches!(character, '\u{fe0e}' | '\u{fe0f}' | '\u{20e3}'))
            && !(self.grapheme_ordinary && ordinary)
            && self.extend_grapheme(character)
        {
            return;
        }
        if width == 0 {
            // A ZWJ can connect the next pictograph, and a virama the next
            // consonant (GB9c), to this cell. Other zero-width marks do not
            // turn ordinary base pairs into a cluster.
            if character == '\u{200d}' || ends_ordinary_run(character) {
                self.grapheme_ordinary = false;
            }
            let mut row = self.cursor.row;
            let mut col = self.cursor.col;
            if !self.pending_wrap && !(self.grapheme_open && self.grapheme_at_margin) {
                if self.grapheme_at_margin {
                    // A closed grapheme at the margin had its wide base
                    // dropped. Its marks must not attach to the cell before.
                    return;
                }
                if col > 0 {
                    col -= 1;
                } else if row > 0 && self.screen().rows[row - 1].wrapped {
                    row -= 1;
                    col = self.size.cols - 1;
                } else {
                    return;
                }
            }
            if self.screen().rows[row].cells[col].flags & Cell::WIDE_SPACER != 0 && col > 0 {
                col -= 1;
            }
            let active = if self.alternate_active {
                &mut self
                    .alternate
                    .as_mut()
                    .expect("active alternate screen")
                    .rows[row]
            } else {
                &mut self.primary.rows[row]
            };
            active.occupied = active.occupied.max(col + 1);
            let mut extra = active.take_extra(col);
            self.combining_cache.append(&mut extra, character);
            active.set_extra(col, extra);
            self.mark(row, col, col + 1);
            return;
        }

        let cell = Cell {
            character,
            style: self.pen.style,
            flags: 0,
        };
        self.grapheme_ordinary = ordinary;
        self.put_cell(cell, self.pen.extra.clone(), width);
    }

    fn put_cell(&mut self, mut cell: Cell, extra: Option<Arc<CellExtra>>, width: usize) {
        if self.pending_wrap {
            if self.autowrap {
                self.wrap_line();
            }
            self.pending_wrap = false;
        }
        let width = width.min(self.size.cols).min(2);
        if width == 2 && self.cursor.col + 1 == self.right_limit() {
            if !self.autowrap {
                // The glyph is discarded, so nothing may extend the cell to
                // its left as though it were this cluster.
                self.grapheme_open = false;
                self.grapheme_at_margin = true;
                return;
            }
            let row = self.cursor.row;
            let col = self.cursor.col;
            let blank = self.blank();
            let full_width = self.margins_span_screen();
            let active = &mut self.screen_mut().rows[row];
            Self::clear_wide_at(active, col, &blank);
            if full_width {
                active.cells[col].flags = Cell::LEADING_WIDE_SPACER;
                active.wrapped = true;
            }
            self.mark(row, col.saturating_sub(1), col + 1);
            self.cursor.col = self.left_limit();
            self.linefeed();
        }
        if self.insert_mode {
            self.insert_chars(width);
        }

        let row = self.cursor.row;
        let col = self.cursor.col;
        let right = self.right_limit();
        // Built only for the rare wide-glyph repairs below.
        let background = self.pen.style.background;
        cell.flags = if width == 2 { Cell::WIDE } else { 0 };
        // Borrow the screen by field so the spacer cache stays available.
        let screen = if self.alternate_active {
            self.alternate.as_mut().expect("active alternate screen")
        } else {
            &mut self.primary
        };
        let active = &mut screen.rows[row];
        let mut start = col;
        let mut end = col + width;
        // Only the outside halves of overwritten wide glyphs need erasing.
        // The destination cells are replaced below, so blanking them first
        // would write/drop each cell twice on ordinary Unicode output.
        if active.cells[col].flags & Cell::WIDE_SPACER != 0 && col > 0 {
            start -= 1;
            active.blank_cell(start, Self::blank_on(background));
        }
        if active.cells[end - 1].flags & Cell::WIDE != 0 && end < active.cells.len() {
            active.blank_cell(end, Self::blank_on(background));
            end += 1;
        }
        if width == 2 {
            let spacer_extra = extra.spacer_extra(&mut self.spacer_extra);
            active.put(col + 1, cell.wide_spacer(), spacer_extra);
        }
        active.put(col, cell, extra);
        active.occupied = active.occupied.max(end);
        self.grapheme_at_margin = col + width >= right;
        if self.grapheme_at_margin {
            self.cursor.col = right - 1;
            self.pending_wrap = self.autowrap;
        } else {
            self.cursor.col += width;
        }
        self.grapheme_open = true;
        if self.full_damage {
            // Graphics revisions must observe scalar edits even while text
            // consumers already need a full redraw and the cursor is hidden.
            self.mark(row, start, end);
            return;
        }
        if self.cursor.visible {
            // The written span covers the old cursor without wrapping; both
            // wrap paths mark its cell before moving or scrolling the row.
            end = end.max(self.cursor.col + 1);
        }
        self.mark(row, start, end);
    }

    /// Autowrap from a pending wrap to the next line's left margin.
    fn wrap_line(&mut self) {
        let row = self.cursor.row;
        // Only a full-width row continues on the next one; text wrapped
        // inside left/right margins doesn't reflow as one line.
        if self.margins_span_screen() {
            self.screen_mut().rows[row].wrapped = true;
        }
        // Wrap metadata belongs to the last cell even when a hidden
        // cursor produces no damage at its previous position.
        self.mark(row, self.cursor.col, self.cursor.col + 1);
        self.cursor.col = self.left_limit();
        self.linefeed();
    }

    /// REP output beyond one screen and a row only scrolls identical rows, so
    /// an untrusted count is clamped. Ordinary bases keep the count modulo a
    /// row, which leaves the same screen and cursor as the full repetition.
    pub(in crate::terminal_engine) fn repeat_char(&mut self, character: char, count: usize) {
        let (width, ordinary) = scalar_properties(character);
        let count = if width == 0 {
            count.min(MAX_COMBINING_BYTES)
        } else {
            // Inside left/right margins rows repeat at the margin width,
            // after at most one screen-wide row outside them.
            let span = self.right_margin - self.left_margin;
            let per_row = (span / width.min(span)).max(1);
            let outside = usize::from(span != self.size.cols) * self.size.cols.div_ceil(per_row);
            let rows = self.size.rows + 1 + outside;
            let limit = per_row * rows;
            match count {
                count if count <= limit => count,
                count if ordinary => limit + count % per_row,
                _ => limit,
            }
        };
        if character.is_ascii() && !character.is_ascii_control() {
            let run = [character as u8; 256];
            let mut remaining = count;
            while remaining > 0 {
                let len = remaining.min(run.len());
                self.write_ascii(&run[..len]);
                remaining -= len;
            }
            return;
        }
        for _ in 0..count {
            self.put_char(character);
        }
    }

    /// Write the leading ordinary bases of UTF-8 `bytes` (complete scalars
    /// and printable ASCII) that fit before the right limit, returning the
    /// bytes consumed and the last scalar written. The result equals
    /// `put_char` per scalar: two ordinary bases never form one grapheme, and
    /// a run stops before any wrap, insert, metadata or wide-cell repair that
    /// `put_cell` would do, leaving that scalar to the caller.
    #[inline]
    pub(in crate::terminal_engine) fn write_utf8_run(
        &mut self,
        bytes: &[u8],
    ) -> (usize, Option<char>) {
        // An open grapheme that isn't ordinary may absorb the next scalar.
        // A combining diacritical mark (U+0300..U+036F) never starts a run:
        // turning it away here spares decomposed text the run's setup.
        if self.pending_wrap
            || self.insert_mode
            || self.pen.extra.is_some()
            || (self.grapheme_open && !self.grapheme_ordinary)
            || matches!(bytes, [0xcc, ..] | [0xcd, 0x80..=0xaf, ..])
        {
            return (0, None);
        }
        self.write_ordinary_run(bytes)
    }

    #[inline(never)]
    fn write_ordinary_run(&mut self, bytes: &[u8]) -> (usize, Option<char>) {
        let row = self.cursor.row;
        let start = self.cursor.col;
        let right = self.right_limit();
        let style = self.pen.style;
        let active = &mut self.screen_mut().rows[row];
        if active.has_extras() {
            return (0, None);
        }
        let occupied = active.occupied;
        let plain = Cell::plain(style, 0);
        let wide = Cell::plain(style, Cell::WIDE);
        let spacer = Cell::plain(style, Cell::WIDE_SPACER);
        // Every store is left of the limit: bounding the cells by it lets
        // the loops index them without further checks.
        let cells = &mut active.cells[..right];
        let mut col = start;
        let mut consumed = 0;
        // Where the last scalar written starts: it is decoded once, at the
        // end, rather than carried through the loops.
        let mut last = 0;
        while let Some(&byte) = bytes.get(consumed) {
            // The most common scalars skip the general decode and
            // classification below: CJK ideographs and Hangul syllables
            // (three-byte, wide), the common two-byte scripts' letters and
            // printable ASCII (narrow), all ordinary bases. Each fast path
            // stores only where the general path would neither break nor
            // repair a wide glyph, and leaves anything else to it. Taking the
            // cells up to one past the scalar's checks the right limit (the
            // general path breaks at it) and bounds the stores at once.
            if byte & 0xf0 == 0xe0 {
                // A run of four or more is stored out of line, mostly four at
                // a time.
                if bytes
                    .get(consumed + 9)
                    .is_some_and(|lead| lead & 0xf0 == 0xe0)
                {
                    let (end, len) = write_wide_cjk(cells, col, occupied, &bytes[consumed..], wide);
                    if len != 0 {
                        col = end;
                        consumed += len;
                        last = consumed - 3;
                        continue;
                    }
                }
                if let Some(&[lead, second, third]) =
                    bytes.get(consumed..).and_then(<[u8]>::first_chunk)
                    && let Some(character) = wide_cjk(lead, second, third)
                    && let Some([glyph, half, _]) = cells.get_mut(col..col + 3)
                    && (col >= occupied || glyph.flags | half.flags == 0)
                {
                    wide.store(glyph, character);
                    spacer.store(half, ' ');
                    col += 2;
                    last = consumed;
                    consumed += 3;
                    continue;
                }
            } else if byte & 0xe0 == 0xc0 {
                // Their words are runs of letters, stored by a loop.
                let from = col;
                while let Some([cell, _]) = cells.get_mut(col..col + 2)
                    && (col >= occupied || cell.flags == 0)
                    && let Some(&[lead, second]) =
                        bytes.get(consumed..).and_then(<[u8]>::first_chunk)
                    && let Some(character) = narrow_two_byte(lead, second)
                {
                    plain.store(cell, character);
                    col += 1;
                    last = consumed;
                    consumed += 2;
                }
                if col != from {
                    continue;
                }
            } else if (0x20..=0x7e).contains(&byte)
                && let Some([cell, _]) = cells.get_mut(col..col + 2)
                && (col >= occupied || cell.flags == 0)
            {
                // Spaces between words come one at a time, other ASCII text
                // in runs: the rest of a run is stored by a loop.
                plain.store(cell, char::from(byte));
                col += 1;
                last = consumed;
                consumed += 1;
                while let Some(&byte @ 0x20..=0x7e) = bytes.get(consumed)
                    && let Some([cell, _]) = cells.get_mut(col..col + 2)
                    && (col >= occupied || cell.flags == 0)
                {
                    plain.store(cell, char::from(byte));
                    col += 1;
                    last = consumed;
                    consumed += 1;
                }
                continue;
            }
            // Printable ASCII and the common two-byte scripts are narrow and
            // ordinary, and other ASCII is a control, which ends the run;
            // anything else is looked up.
            let (character, len, width, ordinary) = if (0x20..=0x7e).contains(&byte) {
                (char::from(byte), 1, 1, true)
            } else if byte < 0x80 {
                break;
            } else if let Some(&second) = bytes.get(consumed + 1)
                && let Some(character) = narrow_two_byte(byte, second)
            {
                (character, 2, 1, true)
            } else {
                let Some((character, len)) = complete_utf8(&bytes[consumed..]) else {
                    break;
                };
                let (width, ordinary) = scalar_properties(character);
                (character, len, width, ordinary)
            };
            // Reaching the limit sets a pending wrap (or wraps a wide glyph);
            // `put_cell` owns that.
            if !ordinary || col + width >= right {
                break;
            }
            // Overwriting either half of a wide glyph needs its repair.
            if col < occupied && cells[col..col + width].iter().any(|cell| cell.flags != 0) {
                break;
            }
            if width == 2 {
                wide.store(&mut cells[col], character);
                spacer.store(&mut cells[col + 1], ' ');
            } else {
                plain.store(&mut cells[col], character);
            }
            col += width;
            last = consumed;
            consumed += len;
        }
        if consumed == 0 {
            return (0, None);
        }
        let last = match bytes[last] {
            byte @ ..0x80 => Some(char::from(byte)),
            _ => complete_utf8(&bytes[last..]).map(|(character, _)| character),
        };
        active.occupied = occupied.max(col);
        // As in `write_ascii`: a pending row is blanked around the text.
        self.write_over_pending_clear(row, start, col);
        self.observe_output();
        self.cursor.col = col;
        self.grapheme_open = true;
        self.grapheme_ordinary = true;
        self.grapheme_at_margin = false;
        self.mark(row, start, (col + 1).min(self.size.cols));
        (consumed, last)
    }

    /// `write_ascii` of one byte in the middle of a plain row, without the
    /// general path's setup. Returns false, having changed nothing, when any
    /// condition that path handles applies.
    #[inline]
    pub(in crate::terminal_engine) fn write_ascii_cell(&mut self, byte: u8) -> bool {
        let (row, col) = (self.cursor.row, self.cursor.col);
        if !self.grapheme_ordinary
            || self.pending_wrap
            || self.insert_mode
            || self.has_pending_clear()
            || self.pen.extra.is_some()
            || col + 1 >= self.right_limit()
        {
            return false;
        }
        let style = self.pen.style;
        let active = &mut self.screen_mut().rows[row];
        if active.has_extras() || (col < active.occupied && active.cells[col].flags != 0) {
            return false;
        }
        Cell::plain(style, 0).store(&mut active.cells[col], char::from(byte));
        active.occupied = active.occupied.max(col + 1);
        self.observe_output();
        self.cursor.col = col + 1;
        self.grapheme_open = true;
        self.grapheme_at_margin = false;
        self.mark(row, col, col + 2);
        true
    }

    /// Ordinary ASCII uses one bounds/damage update per row-local run.
    #[inline]
    pub(in crate::terminal_engine) fn write_ascii(&mut self, mut text: &[u8]) {
        debug_assert!(text.iter().all(|byte| (0x20..=0x7e).contains(byte)));
        // A prepend character can absorb the first ASCII scalar. Ordinary
        // ASCII runs still take the row-local fast path.
        if !self.grapheme_ordinary {
            self.flush_pending_clear();
            if let Some(&first) = text.first()
                && self.extend_grapheme(char::from(first))
            {
                text = &text[1..];
            }
        }
        while !text.is_empty() {
            self.observe_output();
            if self.pending_wrap || self.insert_mode {
                self.flush_pending_clear();
                self.put_char(char::from(text[0]));
                text = &text[1..];
                continue;
            }
            // Without autowrap, each byte at the margin replaces the last one,
            // so only the final byte of the run is visible there.
            let right = self.right_limit();
            if !self.autowrap && self.cursor.col + 1 == right && text.len() > 1 {
                text = &text[text.len() - 1..];
            }
            let row = self.cursor.row;
            let col = self.cursor.col;
            let count = text.len().min(right - col);
            let style = self.pen.style;
            let extra = self.pen.extra.clone();
            let active = &mut self.screen_mut().rows[row];
            // Wide-cell repair is uncommon, and the scalar path handles both
            // ends of an overwritten glyph without a second general scan.
            // Cells past `occupied` are erase blanks, so a fresh row skips it.
            let scanned = col..(col + count).min(active.occupied).max(col);
            if active.cells[scanned].iter().any(|cell| cell.flags != 0) {
                self.flush_pending_clear();
                for &byte in &text[..count] {
                    self.put_char(char::from(byte));
                }
                text = &text[count..];
                continue;
            }
            Cell::write_plain_ascii(&mut active.cells[col..col + count], text, style);
            match &extra {
                Some(extra) => active.fill_extras(col..col + count, extra),
                None => active.clear_extras(col..col + count),
            }
            active.occupied = active.occupied.max(col + count);
            // A row scrolled in by the last line feed is blanked only around
            // this text; its cells hold no metadata, so the plain writes hold.
            self.write_over_pending_clear(row, col, col + count);
            self.cursor.col += count;
            let at_margin = self.cursor.col == right;
            if at_margin {
                self.cursor.col -= 1;
                self.pending_wrap = self.autowrap;
            }
            self.grapheme_open = true;
            self.grapheme_ordinary = true;
            self.grapheme_at_margin = at_margin;
            self.mark(row, col, (col + count + 1).min(self.size.cols));
            text = &text[count..];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_properties_match_width_and_classification_for_every_unicode_scalar() {
        for character in (0..=0x10ffff).filter_map(char::from_u32) {
            let width = character.width().unwrap_or(0);
            assert_eq!(
                scalar_properties(character),
                (width, width != 0 && ordinary_base(character)),
                "U+{:04X}",
                character as u32
            );
        }
    }

    // The fast path skips segmentation between two ordinary bases, so every
    // such base must start a new cluster after another one.
    #[test]
    fn ordinary_bases_always_break_from_each_other() {
        for c in (0..=0x10ffff).filter_map(char::from_u32) {
            if !ordinary_base(c) || c.width().unwrap_or(0) == 0 {
                continue;
            }
            for text in [format!("a{c}"), format!("{c}a"), format!("{c}{c}")] {
                assert_eq!(text.graphemes(true).count(), 2, "U+{:04X}", c as u32);
            }
        }
    }

    // After a mark that can join the next Indic base to the cluster, the fast
    // path must give way to segmentation.
    #[test]
    fn marks_that_join_indic_bases_end_the_ordinary_run() {
        let bases: Vec<_> = ('\u{900}'..='\u{dff}').filter(|c| indic_base(*c)).collect();
        let marks = ('\u{900}'..='\u{dff}').filter(|c| c.width() == Some(0));
        for mark in marks {
            for base in &bases {
                let joined = format!("{base}{mark}{base}").graphemes(true).count() == 1;
                assert!(
                    !joined || ends_ordinary_run(mark),
                    "U+{:04X} joins U+{:04X}",
                    mark as u32,
                    *base as u32
                );
            }
        }
    }

    // The CJK run loop must accept exactly the three-byte scalars the general
    // path writes as wide ordinary bases.
    #[test]
    fn wide_cjk_matches_the_general_decode_and_classification() {
        for lead in 0xe0..=0xefu8 {
            for second in 0..=0xffu8 {
                for third in 0..=0xffu8 {
                    let bytes = [lead, second, third];
                    let general = complete_utf8(&bytes)
                        .filter(|&(character, _)| scalar_properties(character) == (2, true))
                        .map(|(character, _)| character);
                    let fast = wide_cjk(lead, second, third);
                    if fast.is_some() {
                        assert_eq!(fast, general, "{bytes:02x?}");
                    }
                    let block = |c: char| matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{ac00}'..='\u{d7a3}');
                    assert_eq!(fast, general.filter(|&c| block(c)), "{bytes:02x?}");
                }
            }
        }
    }

    // Likewise for the two-byte scalars the run loop decodes itself.
    #[test]
    fn narrow_two_byte_matches_the_general_decode_and_classification() {
        for lead in 0..=0xffu8 {
            for second in 0..=0xffu8 {
                let bytes = [lead, second];
                let general = complete_utf8(&bytes)
                    .filter(|&(character, len)| {
                        len == 2 && scalar_properties(character) == (1, true)
                    })
                    .map(|(character, _)| character);
                let block = |c: char| matches!(c, '\u{c0}'..='\u{2ff}' | '\u{370}'..='\u{482}' | '\u{48a}'..='\u{52f}');
                assert_eq!(
                    narrow_two_byte(lead, second),
                    general.filter(|&c| block(c)),
                    "{bytes:02x?}"
                );
            }
        }
    }

    // `write_utf8_run` turns away these byte patterns undecoded: they
    // encode exactly the combining diacritical marks, at which a run stops.
    #[test]
    fn turned_away_lead_bytes_are_combining_diacritical_marks() {
        for lead in [0xcc_u8, 0xcd] {
            for second in 0..=0xffu8 {
                let bytes = [lead, second];
                let decoded = complete_utf8(&bytes).map(|(character, _)| character);
                let turned_away = matches!(bytes, [0xcc, ..] | [0xcd, 0x80..=0xaf, ..]);
                assert_eq!(
                    turned_away && decoded.is_some(),
                    decoded.is_some_and(|c| ('\u{300}'..='\u{36f}').contains(&c)),
                    "{bytes:02x?}"
                );
            }
        }
        for character in '\u{300}'..='\u{36f}' {
            assert!(!scalar_properties(character).1, "{character:?}");
        }
    }

    #[test]
    fn always_extending_scalars_join_regardless_of_the_base() {
        for c in [
            '\u{301}',
            '\u{200d}',
            '\u{fe0f}',
            '\u{20e3}',
            '\u{1f3fd}',
            '\u{93f}',
        ] {
            assert!(always_extends(c), "U+{:04X}", c as u32);
        }
        for c in ['a', '💻', '🇩', '\u{1100}'] {
            assert!(!always_extends(c), "U+{:04X}", c as u32);
        }
    }
}
