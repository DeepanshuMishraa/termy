//! Cold rows retain scalar/flag arrays, style runs and sparse shared metadata.
//! The live screen keeps dense cells. Borrowed public reads materialize a row
//! until the next mutation; streaming reads use caller-owned scratch instead.
use super::super::types::{CellExtra, MAX_COMBINING_BYTES};
use super::{Cell, Color, Row, Size, Style};
use std::sync::{Arc, OnceLock};

/// A character (21 bits) and its cell flags (top byte) in four bytes.
#[derive(Clone, Copy, Debug)]
struct Scalar(u32);

impl Scalar {
    const FLAGS_SHIFT: u32 = 24;

    fn new(cell: &Cell) -> Self {
        // Cell flags use the low three bits.
        Self(u32::from(cell.character) | u32::from(cell.flags as u8) << Self::FLAGS_SHIFT)
    }

    fn character(self) -> char {
        char::from_u32(self.0 & ((1 << Self::FLAGS_SHIFT) - 1)).unwrap_or(' ')
    }

    fn flags(self) -> u16 {
        (self.0 >> Self::FLAGS_SHIFT) as u16
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct PackedCells {
    decoded: OnceLock<Vec<Cell>>,
    cols: usize,
    background: Color,
    scalars: Vec<Scalar>,
    // Run ends (exclusive column) and styles; `u32` keeps a run at 16 bytes.
    styles: Vec<(u32, Style)>,
    extras: Vec<(usize, Arc<CellExtra>)>,
    // What `ExtraMeter` measured for `extras` while encoding; see `Row`.
    extra_bytes: usize,
}

impl PackedCells {
    /// Pack `cells`, measuring their metadata into `extra_bytes` when
    /// `has_extras` (even when packing fails, so the caller can charge it).
    fn encode(&mut self, cells: &[Cell], cols: usize, background: Color, has_extras: bool) -> bool {
        self.decoded.take();
        self.cols = cols;
        self.background = background;
        self.scalars.clear();
        self.styles.clear();
        self.extras.clear();
        // Buffers recycled from a longer row must not keep its capacity.
        if self.scalars.capacity() > 2 * cells.len() {
            self.scalars.shrink_to(cells.len());
        }
        if self.scalars.capacity() < cells.len() {
            self.scalars.reserve_exact(cells.len() - self.scalars.len());
        }
        if has_extras {
            // Measured in this pass because packing may stop before the end.
            let mut meter = ExtraMeter::default();
            self.scalars.extend(cells.iter().map(|cell| {
                if let Some(extra) = &cell.extra {
                    meter.add(extra);
                }
                Scalar::new(cell)
            }));
            self.extra_bytes = meter.bytes;
        } else {
            self.scalars.extend(cells.iter().map(Scalar::new));
            self.extra_bytes = 0;
        }
        let fixed_bytes = self.scalars.capacity() * size_of::<Scalar>();
        let budget = cols * size_of::<Cell>() / 2;
        if fixed_bytes >= budget {
            return false;
        }
        if !has_extras {
            debug_assert!(
                cells.iter().all(|cell| cell.extra.is_none()),
                "unflagged row holds cell metadata"
            );
            // No cell holds metadata: only style runs remain, found by
            // scanning for each run's end instead of probing per cell.
            let mut start = 0;
            while let Some(first) = cells.get(start) {
                let style = first.style;
                let end = cells[start..]
                    .iter()
                    .position(|cell| cell.style != style)
                    .map_or(cells.len(), |len| start + len);
                if self.styles.is_empty() {
                    // Most rows have one run; don't reserve room for four.
                    self.styles.reserve_exact(1);
                }
                self.styles.push((end as u32, style));
                if fixed_bytes
                    + self.styles.capacity() * size_of::<(u32, Style)>()
                    + self.extras.capacity() * size_of::<(usize, Arc<CellExtra>)>()
                    >= budget
                {
                    return false;
                }
                start = end;
            }
            if self.styles.capacity() > 2 * self.styles.len() + 4 {
                self.styles.shrink_to(self.styles.len());
            }
            if self.extras.capacity() > 4 {
                self.extras.shrink_to(0);
            }
            return true;
        }
        for (col, cell) in cells.iter().enumerate() {
            if self
                .styles
                .last()
                .is_none_or(|(_, style)| *style != cell.style)
            {
                if let Some((end, _)) = self.styles.last_mut() {
                    *end = col as u32;
                } else {
                    // Most rows have one run; don't reserve room for four.
                    self.styles.reserve_exact(1);
                }
                self.styles.push((cells.len() as u32, cell.style));
                if fixed_bytes
                    + self.styles.capacity() * size_of::<(u32, Style)>()
                    + self.extras.capacity() * size_of::<(usize, Arc<CellExtra>)>()
                    >= budget
                {
                    return false;
                }
            }
            if let Some(extra) = &cell.extra {
                self.extras.push((col, Arc::clone(extra)));
                if fixed_bytes
                    + self.styles.capacity() * size_of::<(u32, Style)>()
                    + self.extras.capacity() * size_of::<(usize, Arc<CellExtra>)>()
                    >= budget
                {
                    return false;
                }
            }
        }
        // Bounded slack keeps reuse cheap without retaining a wide row's runs.
        if self.styles.capacity() > 2 * self.styles.len() + 4 {
            self.styles.shrink_to(self.styles.len());
        }
        if self.extras.capacity() > 2 * self.extras.len() + 4 {
            self.extras.shrink_to(self.extras.len());
        }
        true
    }

    fn decode(&self, cells: &mut Vec<Cell>) {
        let blank = Cell {
            style: Style {
                background: self.background,
                ..Style::default()
            },
            ..Cell::default()
        };
        cells.clear();
        cells.resize(self.cols, blank);
        for (cell, scalar) in cells.iter_mut().zip(&self.scalars) {
            cell.character = scalar.character();
            cell.flags = scalar.flags();
        }
        let mut start = 0;
        for &(end, style) in &self.styles {
            let end = end as usize;
            for cell in &mut cells[start..end] {
                cell.style = style;
            }
            start = end;
        }
        for (col, extra) in &self.extras {
            cells[*col].extra = Some(Arc::clone(extra));
        }
    }
}

/// Rows store metadata bytes in units of this many, so a row of 4096
/// maximal composed cells still fits a `u16` beside the row's other fields.
const EXTRA_UNIT: usize = 8;
/// One extra's allocation: `ArcInner`'s strong and weak counts, then the value.
const EXTRA_ALLOCATION: usize = 2 * size_of::<usize>() + size_of::<CellExtra>();
const _: () = assert!(
    Size::MAX_COLS * (EXTRA_ALLOCATION + MAX_COMBINING_BYTES) <= u16::MAX as usize * EXTRA_UNIT
);

/// Measures the heap bytes of the cell metadata a row references: each
/// extra's allocation and combining text. Pen metadata and composed-cell cache
/// entries are shared by runs of cells (and wide glyphs alternate with their
/// spacers' extras), so an extra equal to one of the last two distinct ones is
/// not counted again. A pure function of the row's extras, in column order.
#[derive(Default)]
struct ExtraMeter {
    last: usize,
    before: usize,
    bytes: usize,
}

impl ExtraMeter {
    #[inline]
    fn add(&mut self, extra: &Arc<CellExtra>) {
        let address = Arc::as_ptr(extra).addr();
        if address != self.last && address != self.before {
            (self.before, self.last) = (self.last, address);
            self.bytes += EXTRA_ALLOCATION + extra.combining.capacity();
        }
    }
}

fn extra_units(bytes: usize) -> u16 {
    u16::try_from(bytes.div_ceil(EXTRA_UNIT)).unwrap_or(u16::MAX)
}

impl Row {
    /// Heap bytes of the stored form (dense cells or packed runs) plus the
    /// metadata charged by [`Self::bound_extras`] or [`Self::measure_extras`],
    /// excluding a transient decoded read cache. Interned hyperlinks are
    /// bounded by their own table. History rows don't change, so a row keeps
    /// its charge until it leaves history or a compaction step measures it.
    pub(super) fn heap_bytes(&self) -> usize {
        let extras = usize::from(self.extra_units) * EXTRA_UNIT;
        self.cells.capacity() * size_of::<Cell>()
            + extras
            + self.packed.as_ref().map_or(0, |packed| {
                size_of::<PackedCells>()
                    + packed.scalars.capacity() * size_of::<Scalar>()
                    + packed.styles.capacity() * size_of::<(u32, Style)>()
                    + packed.extras.capacity() * size_of::<(usize, Arc<CellExtra>)>()
            })
    }

    /// Charge a row entering history without scanning its cells: packed rows
    /// were measured while packing, and a dense row with metadata is charged
    /// an upper bound (one maximal extra per occupied cell) until a compaction
    /// step measures it. History packs every queued row before evicting any, so
    /// the bound costs earlier packing, never history.
    #[inline]
    pub(super) fn bound_extras(&mut self) {
        self.extra_units = if self.packed.is_some() {
            self.count_extra_units()
        } else if self.has_extras {
            extra_units(self.occupied * (EXTRA_ALLOCATION + MAX_COMBINING_BYTES))
        } else {
            0
        };
    }

    /// Charge the measured metadata. Packing measures its extras, and dense
    /// rows the writers never flagged hold none, so only dense rows with
    /// metadata are scanned.
    pub(super) fn measure_extras(&mut self) {
        self.extra_units = self.count_extra_units();
    }

    pub(super) fn count_extra_units(&self) -> u16 {
        extra_units(match &self.packed {
            Some(packed) => packed.extra_bytes,
            None if !self.has_extras => 0,
            None => {
                let mut meter = ExtraMeter::default();
                // Cells past `occupied` are erase blanks without metadata.
                for cell in &self.cells[..self.occupied.min(self.cells.len())] {
                    if let Some(extra) = &cell.extra {
                        meter.add(extra);
                    }
                }
                meter.bytes
            }
        })
    }

    #[cfg(test)]
    pub(in crate::terminal_engine) fn is_packed(&self) -> bool {
        self.packed.is_some()
    }

    pub(in crate::terminal_engine) fn with_cells<T>(
        &self,
        scratch: &mut Vec<Cell>,
        read: impl FnOnce(&[Cell]) -> T,
    ) -> T {
        if let Some(packed) = &self.packed {
            if let Some(cells) = packed.decoded.get() {
                return read(cells);
            }
            packed.decode(scratch);
            read(scratch)
        } else {
            read(&self.cells)
        }
    }

    pub(in crate::terminal_engine) fn cells(&self) -> &[Cell] {
        if let Some(packed) = &self.packed {
            packed.decoded.get_or_init(|| {
                let mut cells = Vec::new();
                packed.decode(&mut cells);
                cells
            })
        } else {
            &self.cells
        }
    }

    pub(super) fn make_dense(&mut self) {
        if let Some(mut packed) = self.packed.take() {
            self.has_extras |= !packed.extras.is_empty();
            if let Some(cells) = packed.decoded.take() {
                self.cells = cells;
            } else {
                packed.decode(&mut self.cells);
            }
        }
    }

    /// Drop packed contents without decoding them, keeping a decoded read
    /// cache's allocation (but not its cells) when one exists.
    #[inline]
    pub(super) fn discard_packed(&mut self) -> bool {
        if self.packed.is_none() {
            return false;
        }
        self.discard_packed_contents();
        true
    }

    #[inline(never)]
    fn discard_packed_contents(&mut self) {
        let Some(mut packed) = self.packed.take() else {
            return;
        };
        self.cells = packed.decoded.take().unwrap_or_default();
        self.cells.clear();
    }

    /// Fit a hard-terminated row to `cols` without rewrapping it, keeping
    /// packed history packed. The result matches the logical-line path:
    /// content up to its last non-default cell, then default blanks. Returns
    /// false when the row needs that path (it would wrap, or a cell's width
    /// could be recomputed there).
    pub(super) fn reflow_in_place(&mut self, cols: usize) -> bool {
        let default = Cell::default();
        if self.wrapped || cols < 2 || self.clear_background != default.style.background {
            return false;
        }
        if let Some(packed) = &mut self.packed {
            let recomputed = |col: usize, extra: &CellExtra| {
                packed.scalars[col].flags() & Cell::WIDE == 0 && !extra.combining.is_empty()
            };
            if packed.scalars.len() > cols
                || packed
                    .scalars
                    .iter()
                    .any(|scalar| scalar.flags() & Cell::LEADING_WIDE_SPACER != 0)
                || packed
                    .extras
                    .iter()
                    .any(|(col, extra)| recomputed(*col, extra))
            {
                return false;
            }
            packed.decoded.take();
            packed.cols = cols;
            return true;
        }
        let len = self.content_len();
        if len > cols
            || self.cells[..len].iter().any(|cell| {
                cell.flags & Cell::LEADING_WIDE_SPACER != 0
                    || (cell.flags & Cell::WIDE == 0 && !cell.combining().is_empty())
            })
        {
            return false;
        }
        self.cells.truncate(len);
        self.cells.resize(cols, default);
        self.occupied = len;
        true
    }

    pub(super) fn release_read_cache(&mut self) {
        if let Some(packed) = &mut self.packed {
            packed.decoded.take();
        }
    }

    pub(super) fn compact(&mut self) {
        if self.packed.is_some() {
            return;
        }
        let mut packed = Box::<PackedCells>::default();
        let encoded = packed.encode(
            &self.cells[..self.occupied],
            self.cells.len(),
            self.clear_background,
            self.has_extras,
        );
        // Packed or not, the row is now charged its measured metadata.
        self.extra_units = extra_units(packed.extra_bytes);
        if !encoded {
            return;
        }
        self.cells = Vec::new();
        self.packed = Some(packed);
    }

    pub(super) fn retain(
        mut self,
        recycled: Option<Self>,
        cols: usize,
        blank: &Cell,
    ) -> (Self, Self) {
        if cols < 16 {
            return (self, recycled.unwrap_or_else(|| Self::new(cols, blank)));
        }
        let mut recycled = recycled;
        let mut packed = recycled
            .as_mut()
            .and_then(|row| row.packed.take())
            .unwrap_or_default();
        if !packed.encode(
            &self.cells[..self.occupied],
            cols,
            self.clear_background,
            self.has_extras,
        ) {
            let mut recycled = recycled.unwrap_or_else(|| Self::new(cols, blank));
            if recycled.cells.is_empty() {
                recycled.cells.resize(cols, blank.clone());
                recycled.occupied = 0;
                recycled.clear_background = blank.style.background;
            }
            return (self, recycled);
        }
        let live = Self {
            cells: std::mem::take(&mut self.cells),
            packed: None,
            occupied: self.occupied,
            clear_background: self.clear_background,
            wrapped: false,
            has_extras: self.has_extras,
            extra_units: 0,
        };
        self.packed = Some(packed);
        (self, live)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_engine::Hyperlink;

    #[test]
    fn plain_packed_rows_store_one_tight_style_run() {
        assert_eq!(size_of::<(u32, Style)>(), 16);
        assert_eq!(size_of::<Scalar>(), 4);
        let mut row = Row::new(120, &Cell::default());
        for (cell, character) in row.cells.iter_mut().zip("plain text".chars()) {
            cell.character = character;
        }
        row.occupied = 10;
        row.compact();
        let packed = row.packed.as_ref().unwrap();
        assert_eq!((packed.styles.len(), packed.styles.capacity()), (1, 1));
        assert_eq!(packed.scalars.capacity(), 10);
    }

    #[test]
    fn packed_rows_roundtrip_wide_flags_styles_and_shared_metadata() {
        let blank = Cell {
            style: Style {
                background: Color::indexed(4),
                ..Style::default()
            },
            ..Cell::default()
        };
        let mut row = Row::new(120, &blank);
        row.cells[0].character = '👩';
        row.cells[0].flags = Cell::WIDE;
        row.cells[0].extra = Some(Arc::new(CellExtra {
            combining: "🏽‍💻".into(),
            hyperlink: Some(Arc::new(Hyperlink {
                id: "id".into(),
                uri: "https://example.test".into(),
            })),
            ..CellExtra::default()
        }));
        row.cells[1].flags = Cell::WIDE_SPACER;
        row.cells[2].character = 'x';
        row.cells[2].style.foreground = Color::rgb(1, 2, 3);
        row.occupied = 3;
        row.wrapped = true;
        // As the writers do whenever they store cell metadata.
        row.has_extras = true;
        let expected = row.cells.clone();
        row.compact();
        assert_eq!(row.cells.capacity(), 0);
        assert!(row.packed.is_some());
        let mut scratch = Vec::new();
        row.with_cells(&mut scratch, |cells| assert_eq!(cells, expected));
        assert!(row.packed.as_ref().unwrap().decoded.get().is_none());
        assert_eq!(row.cells(), expected);
        row.release_read_cache();
        assert!(row.packed.as_ref().unwrap().decoded.get().is_none());
        row.make_dense();
        assert_eq!(row.cells, expected);
        assert!(row.wrapped);
        assert!(Arc::ptr_eq(
            row.cells[0].extra.as_ref().unwrap(),
            expected[0].extra.as_ref().unwrap()
        ));
    }

    #[test]
    fn compact_history_reuses_allocations_and_keeps_live_buffers_dense() {
        let mut grid = super::super::Grid::new(super::super::Size { cols: 120, rows: 4 }, 32);
        for _ in 0..100 {
            grid.write_ascii(b"a short log line");
            grid.carriage_return();
            grid.linefeed();
        }
        grid.compact_history(usize::MAX);
        assert!(grid.primary.rows.iter().all(|row| row.packed.is_none()));
        let mut before: Vec<_> = grid
            .history
            .iter()
            .map(|row| {
                assert!(row.cells.capacity() == 0);
                let packed = row.packed.as_ref().unwrap();
                assert!(packed.decoded.get().is_none());
                packed.scalars.as_ptr()
            })
            .collect();
        for _ in 0..100 {
            grid.write_ascii(b"a short log line");
            grid.carriage_return();
            grid.linefeed();
        }
        let mut after: Vec<_> = grid
            .history
            .iter()
            .map(|row| row.packed.as_ref().unwrap().scalars.as_ptr())
            .collect();
        before.sort();
        after.sort();
        assert_eq!(before, after);
    }

    #[test]
    fn heap_bytes_count_cell_metadata_once_per_shared_run() {
        const ALLOCATION: usize = 2 * size_of::<usize>() + size_of::<CellExtra>();
        // The measurement and its flag fit in the row's padding.
        assert_eq!(size_of::<Row>(), 5 * size_of::<usize>() + 8);
        let mut row = Row::new(120, &Cell::default());
        let plain = row.heap_bytes();
        assert_eq!(plain, 120 * size_of::<Cell>());
        // A run sharing the pen's link counts one allocation.
        let link = Arc::new(CellExtra {
            hyperlink: Some(Arc::new(Hyperlink {
                id: String::new(),
                uri: "https://example.test".into(),
            })),
            ..CellExtra::default()
        });
        for cell in &mut row.cells[..10] {
            cell.extra = Some(Arc::clone(&link));
        }
        // Alternating glyph and spacer extras count once each.
        let color = Arc::new(CellExtra {
            underline_color: Color::indexed(1),
            ..CellExtra::default()
        });
        for (col, cell) in row.cells[10..20].iter_mut().enumerate() {
            cell.extra = Some(Arc::clone(if col % 2 == 0 { &link } else { &color }));
        }
        // Distinct composed cells each count their allocation and text.
        let mut combining = 0;
        for (col, cell) in row.cells[20..30].iter_mut().enumerate() {
            cell.push_combining(char::from_u32(0x300 + col as u32).unwrap());
            combining += cell.extra.as_ref().unwrap().combining.capacity();
        }
        row.occupied = 30;
        // Rows the writers never flagged are not scanned.
        row.measure_extras();
        assert_eq!(row.heap_bytes(), plain);
        row.bound_extras();
        assert_eq!(row.heap_bytes(), plain);
        // A flagged dense row entering history is charged a bound, and the
        // charge is kept until it is measured.
        row.has_extras = true;
        assert_eq!(row.heap_bytes(), plain);
        row.bound_extras();
        let bound = (30 * (ALLOCATION + MAX_COMBINING_BYTES)).div_ceil(EXTRA_UNIT) * EXTRA_UNIT;
        assert_eq!(row.heap_bytes(), plain + bound);
        // Measuring charges each distinct run's allocation and text.
        row.measure_extras();
        let extras = (12 * ALLOCATION + combining).div_ceil(EXTRA_UNIT) * EXTRA_UNIT;
        assert!(extras < bound);
        assert_eq!(row.heap_bytes(), plain + extras);
        // The packed form lists the same metadata, without a bound.
        row.compact();
        assert_eq!(row.count_extra_units(), row.extra_units);
        row.bound_extras();
        let packed = row.packed.as_ref().unwrap();
        assert_eq!(
            row.heap_bytes(),
            size_of::<PackedCells>()
                + packed.scalars.capacity() * size_of::<Scalar>()
                + packed.styles.capacity() * size_of::<(u32, Style)>()
                + packed.extras.capacity() * size_of::<(usize, Arc<CellExtra>)>()
                + extras
        );
    }

    #[test]
    fn pathological_style_changes_keep_the_bounded_dense_representation() {
        let mut row = Row::new(120, &Cell::default());
        for (col, cell) in row.cells.iter_mut().enumerate() {
            cell.style.foreground = Color::indexed(col as u8);
        }
        row.occupied = 120;
        row.compact();
        assert!(row.packed.is_none());
        assert_eq!(row.cells.len(), 120);
    }
}
