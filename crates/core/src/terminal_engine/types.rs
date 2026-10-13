//! Compact, engine-owned terminal values. No renderer or PTY dependencies.

use std::sync::Arc;

/// Marks retained per cell. The longest standard emoji ZWJ suffix is about
/// 31 bytes; a larger cap only lets hostile output grow every cell (and makes
/// each append, which copies the suffix, quadratic in the cap).
pub(super) const MAX_COMBINING_BYTES: usize = 64;

/// A color occupies four bytes, including its default/indexed/RGB tag. Bytes
/// rather than a `u32` keep [`Style`] 1-aligned and free of padding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color([u8; 4]);

impl Color {
    pub const DEFAULT: Self = Self([0; 4]);
    pub const fn indexed(index: u8) -> Self {
        Self([1, 0, 0, index])
    }
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([2, r, g, b])
    }
    pub const fn as_indexed(self) -> Option<u8> {
        if self.0[0] == 1 {
            Some(self.0[3])
        } else {
            None
        }
    }
    pub const fn as_rgb(self) -> Option<(u8, u8, u8)> {
        if self.0[0] == 2 {
            Some((self.0[1], self.0[2], self.0[3]))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum UnderlineStyle {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// Ten 1-aligned bytes, so a cell with its flags and extra pointer is 24 B.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub foreground: Color,
    pub background: Color,
    pub attributes: u8,
    pub underline: UnderlineStyle,
}

impl Style {
    pub const BOLD: u8 = 1;
    pub const DIM: u8 = 1 << 1;
    pub const ITALIC: u8 = 1 << 2;
    pub const INVERSE: u8 = 1 << 3;
    pub const HIDDEN: u8 = 1 << 4;
    pub const STRIKE: u8 = 1 << 5;
    pub const BLINK: u8 = 1 << 6;
    pub const PROTECTED: u8 = 1 << 7;

    pub fn set(&mut self, flags: u8, enabled: bool) {
        if enabled {
            self.attributes |= flags;
        } else {
            self.attributes &= !flags;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hyperlink {
    pub id: String,
    pub uri: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellExtra {
    pub combining: String,
    pub hyperlink: Option<Arc<Hyperlink>>,
    /// SGR 58. Rare, so it lives here instead of on every cell's style.
    pub underline_color: Color,
}

impl CellExtra {
    fn is_empty(&self) -> bool {
        self.combining.is_empty()
            && self.hyperlink.is_none()
            && self.underline_color == Color::DEFAULT
    }
}

/// Plain cells allocate nothing. Uncommon combining text, OSC 8 metadata and
/// underline colors live behind a shared pointer so row copies do not copy
/// strings.
/// `repr(C)` keeps every field naturally aligned with no padding, so the
/// print loops store a cell in a few aligned writes.
#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Cell {
    pub extra: Option<Arc<CellExtra>>,
    pub style: Style,
    pub flags: u16,
    pub character: char,
}

// 24 B on 64-bit targets; the extra pointer is 4 B on wasm32, so 20 B there.
const _: () = assert!(size_of::<Style>() == 10 && size_of::<Cell>() == size_of::<usize>() + 16);

/// A metadata-free cell as the words it occupies, so the print and clear
/// loops store each cell with two or three full-width writes instead of one
/// per field (a 10-byte style and 2-byte flags otherwise split into narrow
/// stores, reloaded every iteration because the output may alias them).
#[derive(Clone, Copy)]
pub(super) struct PlainCell {
    #[cfg(target_pointer_width = "64")]
    words: [u64; 3],
    #[cfg(not(target_pointer_width = "64"))]
    style: Style,
    #[cfg(not(target_pointer_width = "64"))]
    flags: u16,
}

impl PlainCell {
    #[inline(always)]
    fn new(style: Style, flags: u16) -> Self {
        #[cfg(target_pointer_width = "64")]
        {
            let cell = std::mem::ManuallyDrop::new(Cell {
                extra: None,
                style,
                flags,
                character: '\0',
            });
            // SAFETY: `Cell` is `repr(C)`, 24 bytes and 8-aligned with no
            // padding (asserted above), so every byte is initialized and it
            // reads as three words. `ManuallyDrop` keeps the `None` alive.
            let words = unsafe { std::ptr::read((&raw const *cell).cast::<[u64; 3]>()) };
            Self { words }
        }
        #[cfg(not(target_pointer_width = "64"))]
        Self { style, flags }
    }

    /// Overwrite `cell`, which must hold no metadata (it isn't released).
    #[inline(always)]
    pub(super) fn store(self, cell: &mut Cell, character: char) {
        debug_assert!(cell.extra.is_none(), "unflagged row holds cell metadata");
        #[cfg(target_pointer_width = "64")]
        {
            // `character` is the last field: the high half of the last word.
            let shift = if cfg!(target_endian = "little") {
                32
            } else {
                0
            };
            let last = self.words[2] | (u64::from(u32::from(character)) << shift);
            // SAFETY: as in `new`; the words are a valid `Cell` whose `extra`
            // is the null `None`, and the replaced `None` owns nothing.
            unsafe {
                std::ptr::from_mut(cell).cast::<[u64; 3]>().write([
                    self.words[0],
                    self.words[1],
                    last,
                ]);
            }
        }
        #[cfg(not(target_pointer_width = "64"))]
        cell.overwrite_plain(character, self.style, self.flags);
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            style: Style::default(),
            flags: 0,
            extra: None,
        }
    }
}

impl Cell {
    pub const WIDE: u16 = 1;
    pub const WIDE_SPACER: u16 = 1 << 1;
    pub const LEADING_WIDE_SPACER: u16 = 1 << 2;
    pub fn combining(&self) -> &str {
        self.extra
            .as_ref()
            .map_or("", |extra| extra.combining.as_str())
    }
    pub fn hyperlink(&self) -> Option<&Arc<Hyperlink>> {
        self.extra
            .as_ref()
            .and_then(|extra| extra.hyperlink.as_ref())
    }
    pub fn underline_color(&self) -> Color {
        self.extra
            .as_ref()
            .map_or(Color::DEFAULT, |extra| extra.underline_color)
    }
    pub(super) fn set_underline_color(&mut self, color: Color) {
        if self.underline_color() == color {
            return;
        }
        let extra = self
            .extra
            .get_or_insert_with(|| Arc::new(CellExtra::default()));
        Arc::make_mut(extra).underline_color = color;
        if extra.is_empty() {
            self.extra = None;
        }
    }
    /// A word-store template for metadata-free cells in `style`; see
    /// [`PlainCell`].
    #[inline(always)]
    pub(super) fn plain(style: Style, flags: u16) -> PlainCell {
        PlainCell::new(style, flags)
    }

    /// Fill `cells`, which hold no metadata (see [`Self::overwrite_plain`]),
    /// with a plain character in `style`.
    #[inline]
    pub(super) fn fill_plain(cells: &mut [Cell], character: char, style: Style) {
        let template = PlainCell::new(style, 0);
        for cell in cells {
            template.store(cell, character);
        }
    }

    /// Write printable ASCII into `cells`, which hold no metadata.
    #[inline]
    pub(super) fn write_plain_ascii(cells: &mut [Cell], text: &[u8], style: Style) {
        let template = PlainCell::new(style, 0);
        for (cell, &byte) in cells.iter_mut().zip(text) {
            template.store(cell, char::from(byte));
        }
    }

    /// Overwrite a cell that holds no metadata, without its drop glue. Rows
    /// that `has_extras` doesn't flag hold none, and skipping the per-cell
    /// release check lets runs of writes compile to plain wide stores.
    #[cfg(not(target_pointer_width = "64"))]
    #[inline(always)]
    fn overwrite_plain(&mut self, character: char, style: Style, flags: u16) {
        debug_assert!(self.extra.is_none(), "unflagged row holds cell metadata");
        // Forgetting an `extra` of `None` releases nothing.
        std::mem::forget(std::mem::replace(
            self,
            Self {
                extra: None,
                style,
                flags,
                character,
            },
        ));
    }
    /// The right half of a wide character. It keeps the style and underline
    /// color so decorations span both columns, but no text or link.
    #[inline]
    pub(super) fn wide_spacer(&self) -> Self {
        Self {
            character: ' ',
            style: self.style,
            flags: Self::WIDE_SPACER,
            extra: self.spacer_extra(&mut None),
        }
    }
    /// [`Self::wide_spacer`] written in place, without a temporary cell.
    #[inline]
    pub(super) fn become_wide_spacer_of(
        &mut self,
        wide: &Cell,
        cache: &mut Option<Arc<CellExtra>>,
    ) {
        self.character = ' ';
        self.style = wide.style;
        self.flags = Self::WIDE_SPACER;
        self.extra = wide.spacer_extra(cache);
    }
    /// Metadata for this wide cell's spacer: only its underline color.
    /// `cache` keeps the last color-only extra built for a cell that also has
    /// text or a link, so a run of such glyphs shares one allocation.
    #[inline]
    fn spacer_extra(&self, cache: &mut Option<Arc<CellExtra>>) -> Option<Arc<CellExtra>> {
        self.extra
            .as_ref()
            .and_then(|extra| Self::spacer_extra_of(extra, cache))
    }
    #[cold]
    fn spacer_extra_of(
        extra: &Arc<CellExtra>,
        cache: &mut Option<Arc<CellExtra>>,
    ) -> Option<Arc<CellExtra>> {
        if extra.underline_color == Color::DEFAULT {
            None
        } else if extra.combining.is_empty() && extra.hyperlink.is_none() {
            Some(Arc::clone(extra))
        } else {
            // Cells copy-on-write shared extras, so the cached one never changes.
            if cache
                .as_ref()
                .is_none_or(|cached| cached.underline_color != extra.underline_color)
            {
                *cache = Some(Arc::new(CellExtra {
                    underline_color: extra.underline_color,
                    ..CellExtra::default()
                }));
            }
            cache.clone()
        }
    }
    pub(super) fn push_combining(&mut self, character: char) {
        // Bound a hostile stream of combining marks attached to a single cell.
        if self.combining().len().saturating_add(character.len_utf8()) > MAX_COMBINING_BYTES {
            return;
        }
        let extra = self
            .extra
            .get_or_insert_with(|| Arc::new(CellExtra::default()));
        Arc::make_mut(extra).combining.push(character);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorShape {
    #[default]
    Block,
    Beam,
    Underline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub visible: bool,
    pub shape: CursorShape,
    pub blinking: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
            shape: CursorShape::Block,
            blinking: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirtySpan {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

/// Column ranges are half open, matching Rust slice indexing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Damage {
    Full,
    Partial(Vec<DirtySpan>),
}

/// A viewport row rotation to replay before applying the accompanying damage.
/// `bottom` is exclusive; positive `lines` move rows up, negative values down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewportScroll {
    pub top: usize,
    pub bottom: usize,
    pub lines: i32,
}

/// Ordered text-grid operations consumed by the graphics placement bridge.
/// Scroll regions and column spans both use exclusive upper bounds. Positive
/// line counts move cells up; negative counts move them down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GridEffect {
    Scroll {
        alternate: bool,
        top: usize,
        bottom: usize,
        lines: i64,
        retains_history: bool,
        history_before: usize,
        history_after: usize,
    },
    Clear {
        alternate: bool,
        history_size: usize,
    },
    ClearHistory {
        removed: usize,
    },
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub cols: usize,
    pub rows: usize,
}

impl Default for Size {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

impl Size {
    pub const MAX_COLS: usize = 4096;
    pub const MAX_ROWS: usize = 4096;
    pub const MAX_CELLS: usize = 1_048_576;
    pub(super) fn clamped(self) -> Self {
        let cols = self.cols.clamp(1, Self::MAX_COLS);
        let rows = self
            .rows
            .clamp(1, Self::MAX_ROWS)
            .min(Self::MAX_CELLS / cols);
        Self { cols, rows }
    }
}
