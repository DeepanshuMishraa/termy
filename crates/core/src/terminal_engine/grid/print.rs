//! Streaming graphemes and the ASCII output fast path.
use super::super::types::MAX_COMBINING_BYTES;
use super::{Cell, Grid};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(super) fn cell_width(cell: &Cell) -> usize {
    if cell.combining().is_empty() {
        return cell.character.width().unwrap_or(0);
    }
    let mut bytes = [0; MAX_COMBINING_BYTES + 4];
    let len = cell.character.encode_utf8(&mut bytes).len();
    bytes[len..len + cell.combining().len()].copy_from_slice(cell.combining().as_bytes());
    std::str::from_utf8(&bytes[..len + cell.combining().len()])
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
        let previous = &self.screen().rows[row].cells[col];
        // Match the existing bound on untrusted combining sequences. Discard
        // excess suffixes without moving the cursor or splitting a cluster.
        let over_limit = previous.combining().len() + character.len_utf8() > MAX_COMBINING_BYTES;
        if over_limit && always_extends(character) {
            return true;
        }
        let mut bytes = [0; MAX_COMBINING_BYTES + 8];
        let mut len = previous.character.encode_utf8(&mut bytes).len();
        bytes[len..len + previous.combining().len()]
            .copy_from_slice(previous.combining().as_bytes());
        len += previous.combining().len();
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
        let mut cell = previous.clone();
        self.combining_cache.append(&mut cell, character);
        if width == old_width {
            let active = &mut self.screen_mut().rows[row];
            active.has_extras = true;
            active.cells[col] = cell;
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
        self.put_cell(cell, width);
        self.flag_extras_at_cursor();
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
            active.has_extras = true;
            let cell = &mut active.cells[col];
            self.combining_cache.append(cell, character);
            self.mark(row, col, col + 1);
            return;
        }

        let cell = Cell {
            character,
            style: self.pen.style,
            flags: 0,
            extra: self.pen.extra.clone(),
        };
        self.grapheme_ordinary = ordinary;
        self.put_cell(cell, width);
        // `put_cell` leaves the cursor on the row it wrote. Flagging here
        // rather than per stored cell keeps plain pens off this path.
        if self.pen.extra.is_some() {
            self.flag_extras_at_cursor();
        }
    }

    /// Note that the cursor row may hold cell metadata; see `Row::has_extras`.
    #[cold]
    #[inline(never)]
    fn flag_extras_at_cursor(&mut self) {
        let row = self.cursor.row;
        self.screen_mut().rows[row].has_extras = true;
    }

    fn put_cell(&mut self, mut cell: Cell, width: usize) {
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
            active.cells[start] = Self::blank_on(background);
        }
        if active.cells[end - 1].flags & Cell::WIDE != 0 && end < active.cells.len() {
            active.cells[end] = Self::blank_on(background);
            end += 1;
        }
        if width == 2 {
            active.cells[col + 1].become_wide_spacer_of(&cell, &mut self.spacer_extra);
        }
        active.cells[col] = cell;
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

    /// Ordinary ASCII uses one bounds/damage update per row-local run.
    #[inline]
    pub(in crate::terminal_engine) fn write_ascii(&mut self, mut text: &[u8]) {
        debug_assert!(text.iter().all(|byte| (0x20..=0x7e).contains(byte)));
        // A prepend character can absorb the first ASCII scalar. Ordinary
        // ASCII runs still take the row-local fast path.
        if !self.grapheme_ordinary
            && let Some(&first) = text.first()
            && self.extend_grapheme(char::from(first))
        {
            text = &text[1..];
        }
        while !text.is_empty() {
            self.observe_output();
            if self.pending_wrap || self.insert_mode {
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
            // Wide-cell repair is uncommon, and the scalar path handles both
            // ends of an overwritten glyph without a second general scan.
            if self.screen().rows[row].cells[col..col + count]
                .iter()
                .any(|cell| cell.flags != 0)
            {
                for &byte in &text[..count] {
                    self.put_char(char::from(byte));
                }
                text = &text[count..];
                continue;
            }
            let style = self.pen.style;
            let extra = self.pen.extra.clone();
            let active = &mut self.screen_mut().rows[row];
            let cells = active.cells[col..col + count].iter_mut().zip(text);
            // Build whole cells so a plain pen is stored without reloading it.
            if let Some(extra) = extra {
                active.has_extras = true;
                for (cell, &byte) in cells {
                    *cell = Cell {
                        character: char::from(byte),
                        style,
                        flags: 0,
                        extra: Some(Arc::clone(&extra)),
                    };
                }
            } else {
                for (cell, &byte) in cells {
                    *cell = Cell {
                        character: char::from(byte),
                        style,
                        flags: 0,
                        extra: None,
                    };
                }
            }
            active.occupied = active.occupied.max(col + count);
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
