//! Legacy frame cells: engine cells with their colors and attributes
//! resolved for hosts that draw `TermyCell`s (the C ABI's frame updates).
use super::*;
use std::mem::MaybeUninit;

/// Converts runs of engine cells to legacy frame cells. Neighbouring cells
/// almost always share a style, so its colors and attribute flags are
/// resolved once per run instead of once per cell.
pub(super) struct LegacyCells<'a> {
    palette: &'a TerminalPalette,
    fallback: TerminalQueryColors,
    style: Option<(engine::Style, Template)>,
    // Erase blanks on `blank_background`, copied for each row's blank
    // suffix; see `extend_row`.
    blank_row: Vec<TermyCell>,
    blank_background: engine::Color,
}

impl<'a> LegacyCells<'a> {
    pub(super) fn new(palette: &'a TerminalPalette, fallback: TerminalQueryColors) -> Self {
        Self {
            palette,
            fallback,
            style: None,
            blank_row: Vec::new(),
            blank_background: engine::Color::DEFAULT,
        }
    }

    /// Append `line`, whose last cell ends a wrapped row when `wrapped`.
    pub(super) fn extend(
        &mut self,
        cells: &mut Vec<TermyCell>,
        line: &[engine::Cell],
        wrapped: bool,
    ) {
        if line.is_empty() {
            return;
        }
        cells.reserve(line.len());
        let start = cells.len();
        // Writing into the reserved tail, rather than pushing, keeps the
        // vector's length and capacity out of the per-cell loop.
        let slots = &mut cells.spare_capacity_mut()[..line.len()];
        let mut done = 0;
        while let Some(cell) = line.get(done) {
            let style = cell.style;
            let template = match self.style {
                Some((key, template)) if key == style => template,
                _ => {
                    let template = self.template(style);
                    self.style = Some((style, template));
                    template
                }
            };
            // The run's first cell has its style, so every pass stores one.
            done += template.store_run(&mut slots[done..], &line[done..], style.bits());
        }
        // SAFETY: `reserve` made room for `line.len()` cells after `start`,
        // and the runs above initialized every one of them.
        unsafe { cells.set_len(start + line.len()) };
        if wrapped && let Some(last) = cells.last_mut() {
            last.line_wrapped = true;
        }
    }

    /// [`Self::extend`] for a row read with `Engine::with_viewport_row_blanks`:
    /// the cells of `line` from `blanks` on are erase blanks on `background`,
    /// so they are stored from one template without being read.
    pub(super) fn extend_row(
        &mut self,
        cells: &mut Vec<TermyCell>,
        line: &[engine::Cell],
        wrapped: bool,
        blanks: usize,
        background: engine::Color,
    ) {
        let blanks = blanks.min(line.len());
        self.extend(cells, &line[..blanks], false);
        let count = line.len() - blanks;
        if count != 0 {
            // Every row's blanks are the same frame cell: copied from one
            // row of them, they cost a copy instead of a conversion each.
            if self.blank_background != background || self.blank_row.len() < count {
                self.fill_blank_row(background, count);
            }
            cells.extend_from_slice(&self.blank_row[..count]);
        }
        if wrapped
            && !line.is_empty()
            && let Some(last) = cells.last_mut()
        {
            last.line_wrapped = true;
        }
    }

    #[inline(never)]
    fn fill_blank_row(&mut self, background: engine::Color, count: usize) {
        let blank = engine::Cell {
            style: engine::Style {
                background,
                ..engine::Style::default()
            },
            ..engine::Cell::default()
        };
        self.blank_row.clear();
        self.blank_row.resize(
            count,
            legacy_cell(&blank, false, self.palette, self.fallback),
        );
        self.blank_background = background;
    }

    /// The frame cell fields `style` determines, for a printable character.
    #[inline(never)]
    fn template(&self, style: engine::Style) -> Template {
        let blank = engine::Cell {
            style,
            character: 'x',
            ..engine::Cell::default()
        };
        Template::new(legacy_cell(&blank, false, self.palette, self.fallback))
    }
}

// `Template` stores frame cells as these byte ranges: the character and the
// foreground, the background and four flags (`render_text` and the spacer
// flag last), then the other four flags.
const _: () = assert!(
    size_of::<TermyCell>() == 20
        && std::mem::offset_of!(TermyCell, char) == 0
        && std::mem::offset_of!(TermyCell, fg) == 4
        && std::mem::offset_of!(TermyCell, bg) == 8
        && std::mem::offset_of!(TermyCell, uses_terminal_default_bg) == 12
        && std::mem::offset_of!(TermyCell, bold) == 13
        && std::mem::offset_of!(TermyCell, render_text) == 14
        && std::mem::offset_of!(TermyCell, wide_character_spacer) == 15
        && std::mem::offset_of!(TermyCell, line_wrapped) == 16
        && std::mem::offset_of!(TermyCell, italic) == 17
        && std::mem::offset_of!(TermyCell, underline) == 18
        && std::mem::offset_of!(TermyCell, strikethrough) == 19
);

/// A style's frame cell as the words it is stored with, so each cell is
/// three register stores rather than one per field. The character,
/// `render_text` and spacer bytes are left clear for `store` to fill in.
#[derive(Clone, Copy)]
pub(super) struct Template {
    low: u64,
    middle: u64,
    high: u32,
    render_text: bool,
}

impl Template {
    fn new(cell: TermyCell) -> Self {
        // SAFETY: `TermyCell` is `repr(C)` with no padding (asserted above):
        // every byte of a valid cell is initialized.
        let bytes = unsafe { std::mem::transmute::<TermyCell, [u8; 20]>(cell) };
        let word = |range: std::ops::Range<usize>| {
            let mut word = [0; 8];
            word[..range.len()].copy_from_slice(&bytes[range]);
            word
        };
        let mut low = word(0..8);
        low[..4].fill(0);
        let mut middle = word(8..16);
        middle[6..].fill(0);
        let [h0, h1, h2, h3, ..] = word(16..20);
        Self {
            low: u64::from_ne_bytes(low),
            middle: u64::from_ne_bytes(middle),
            high: u32::from_ne_bytes([h0, h1, h2, h3]),
            render_text: cell.render_text,
        }
    }

    /// Store the leading cells of `line` whose style has `key` bits into
    /// `slots`, returning how many. Out of line, so the template and key
    /// stay in registers across the loop.
    #[inline(never)]
    fn store_run(
        self,
        slots: &mut [MaybeUninit<TermyCell>],
        line: &[engine::Cell],
        key: (u64, u16),
    ) -> usize {
        const SPACERS: u16 = engine::Cell::WIDE_SPACER | engine::Cell::LEADING_WIDE_SPACER;
        // The middle word of most cells: their text renders unless hidden
        // (when the template has `render_text` clear).
        let text = self.middle | u64::from_ne_bytes([0, 0, 0, 0, 0, 0, self.render_text.into(), 0]);
        let spacer_middle = self.middle | u64::from_ne_bytes([0, 0, 0, 0, 0, 0, 0, 1]);
        let count = slots.len().min(line.len());
        let (slots, line) = (&mut slots[..count], &line[..count]);
        let mut index = 0;
        while let Some(cell) = line.get(index) {
            if cell.style.bits() != key {
                return index;
            }
            // C0 and C1 controls don't render, nor do spacers.
            let code = u32::from(cell.character);
            let control = code < 0x20 || code.wrapping_sub(0x7f) <= 0x9f - 0x7f;
            if cell.flags == 0 && !control {
                self.store(&mut slots[index], cell.character, text);
                index += 1;
                continue;
            }
            let middle = if cell.flags & SPACERS != 0 {
                spacer_middle
            } else if control {
                self.middle
            } else {
                text
            };
            self.store(&mut slots[index], cell.character, middle);
            index += 1;
            // A wide glyph's spacer usually follows it, and is stored
            // without the checks only rendered text needs.
            if cell.flags & engine::Cell::WIDE != 0
                && let Some(next) = line.get(index)
                && next.flags & SPACERS != 0
                && next.style.bits() == key
            {
                self.store(&mut slots[index], next.character, spacer_middle);
                index += 1;
            }
        }
        count
    }

    /// Store a cell of `character` whose middle word, the template's with
    /// its `render_text` and spacer bytes set, is `middle`.
    #[inline(always)]
    fn store(self, slot: &mut MaybeUninit<TermyCell>, character: char, middle: u64) {
        let [c0, c1, c2, c3] = u32::from(character).to_ne_bytes();
        let low = self.low | u64::from_ne_bytes([c0, c1, c2, c3, 0, 0, 0, 0]);
        let cell = slot.as_mut_ptr().cast::<u8>();
        // SAFETY: the three writes cover the slot's 20 bytes with the bytes
        // of a valid cell: the template's, a character's and two booleans.
        // Cells are 4-aligned, so the 8-byte writes are unaligned.
        unsafe {
            cell.cast::<u64>().write_unaligned(low);
            cell.add(8).cast::<u64>().write_unaligned(middle);
            cell.add(16).cast::<u32>().write_unaligned(self.high);
        }
    }
}

fn legacy_cell(
    cell: &engine::Cell,
    wrapped: bool,
    palette: &TerminalPalette,
    fallback: TerminalQueryColors,
) -> TermyCell {
    let attributes = cell.style.attributes;
    let bold = attributes & engine::Style::BOLD != 0;
    let inverse = attributes & engine::Style::INVERSE != 0;
    let foreground = render_color(cell.style.foreground, true);
    let background = render_color(cell.style.background, false);
    let (mut foreground, background) = if inverse {
        (background, foreground)
    } else {
        (foreground, background)
    };
    if bold && let TerminalRenderColor::Indexed(index @ 0..=7) = foreground {
        foreground = TerminalRenderColor::Indexed(index + 8);
    }
    let mut fg = resolve_color(foreground, palette, fallback);
    if attributes & engine::Style::DIM != 0 {
        fg.r /= 2;
        fg.g /= 2;
        fg.b /= 2;
    }
    let spacer = cell.flags & (engine::Cell::WIDE_SPACER | engine::Cell::LEADING_WIDE_SPACER) != 0;
    TermyCell {
        char: cell.character,
        fg,
        bg: resolve_color(background, palette, fallback),
        uses_terminal_default_bg: background == TerminalRenderColor::DefaultBackground,
        bold,
        italic: attributes & engine::Style::ITALIC != 0,
        underline: cell.style.underline != engine::UnderlineStyle::None,
        strikethrough: attributes & engine::Style::STRIKE != 0,
        render_text: !spacer
            && attributes & engine::Style::HIDDEN == 0
            && cell.character != '\0'
            && !cell.character.is_control(),
        wide_character_spacer: spacer,
        line_wrapped: wrapped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The run conversion stores each cell from its style's template; it
    // must produce exactly what resolving every cell on its own does.
    #[test]
    fn templated_runs_match_cells_resolved_one_by_one() {
        let mut palette = TerminalPalette {
            indexed: [None; 256],
            foreground: Some(TerminalColor { r: 1, g: 2, b: 3 }),
            background: None,
            cursor: None,
            revision: 0,
        };
        palette.indexed[9] = Some(TerminalColor { r: 9, g: 8, b: 7 });
        let fallback = TerminalQueryColors::default();
        let styles = [
            engine::Style::default(),
            engine::Style {
                foreground: engine::Color::indexed(1),
                background: engine::Color::rgb(4, 5, 6),
                attributes: engine::Style::BOLD | engine::Style::ITALIC,
                underline: engine::UnderlineStyle::Curly,
            },
            engine::Style {
                foreground: engine::Color::rgb(200, 100, 50),
                attributes: engine::Style::DIM
                    | engine::Style::INVERSE
                    | engine::Style::STRIKE
                    | engine::Style::HIDDEN,
                ..engine::Style::default()
            },
        ];
        let characters = [
            'a',
            ' ',
            '\0',
            '\u{1b}',
            '\u{85}',
            '\u{a0}',
            '界',
            '\u{1f642}',
        ];
        let flags = [
            0,
            engine::Cell::WIDE,
            engine::Cell::WIDE_SPACER,
            engine::Cell::LEADING_WIDE_SPACER,
        ];
        let mut line = Vec::new();
        for (index, &character) in characters.iter().cycle().take(97).enumerate() {
            line.push(engine::Cell {
                character,
                style: styles[index / 5 % styles.len()],
                flags: flags[index % flags.len()],
                ..engine::Cell::default()
            });
        }
        for wrapped in [false, true] {
            let mut cells = Vec::new();
            let mut legacy = LegacyCells::new(&palette, fallback);
            // Two calls: the second starts from the cached style.
            legacy.extend(&mut cells, &line[..40], false);
            legacy.extend(&mut cells, &line[40..], wrapped);
            let expected: Vec<_> = line
                .iter()
                .enumerate()
                .map(|(col, cell)| {
                    legacy_cell(cell, wrapped && col + 1 == line.len(), &palette, fallback)
                })
                .collect();
            assert_eq!(cells, expected);
        }
    }
}
