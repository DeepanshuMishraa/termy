//! Cold rows retain scalar/flag arrays and style runs; their sparse shared
//! metadata stays in the row's table either way. The live screen keeps dense
//! cells. Borrowed public reads materialize a row until the next mutation;
//! streaming reads use caller-owned scratch instead.
use super::super::types::{CellExtra, Line, MAX_COMBINING_BYTES};
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
}

impl PackedCells {
    /// Pack `cells`, returning false when their packed form wouldn't be
    /// smaller enough to keep.
    fn encode(&mut self, cells: &[Cell], cols: usize, background: Color) -> bool {
        self.decoded.take();
        self.cols = cols;
        self.background = background;
        self.scalars.clear();
        self.styles.clear();
        // Buffers recycled from a longer row must not keep its capacity.
        if self.scalars.capacity() > 2 * cells.len() {
            self.scalars.shrink_to(cells.len());
        }
        if self.scalars.capacity() < cells.len() {
            self.scalars.reserve_exact(cells.len() - self.scalars.len());
        }
        self.scalars.extend(cells.iter().map(Scalar::new));
        let fixed_bytes = self.scalars.capacity() * size_of::<Scalar>();
        let budget = cols * size_of::<Cell>() / 2;
        if fixed_bytes >= budget {
            return false;
        }
        // Style runs, found by scanning for each run's end instead of
        // probing per cell.
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
            if fixed_bytes + self.styles.capacity() * size_of::<(u32, Style)>() >= budget {
                return false;
            }
            start = end;
        }
        // Bounded slack keeps reuse cheap without retaining a wide row's runs.
        if self.styles.capacity() > 2 * self.styles.len() + 4 {
            self.styles.shrink_to(self.styles.len());
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
/// not counted again.
///
/// Extras are compared by value, never by allocation: whether two equal cells
/// share one depends on caches (and pointer-keyed buckets) that differ between
/// an engine and one decoded from its snapshot, and history's eviction must
/// not. [`Row::share_equal_extras`] makes the sharing match the measurement.
#[derive(Default)]
struct ExtraMeter<'a> {
    last: Option<&'a Arc<CellExtra>>,
    before: Option<&'a Arc<CellExtra>>,
    bytes: usize,
}

fn same_extra(left: &Arc<CellExtra>, right: &Arc<CellExtra>) -> bool {
    Arc::ptr_eq(left, right)
        || (left.underline_color == right.underline_color
            && left.combining == right.combining
            && match (&left.hyperlink, &right.hyperlink) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right) || left == right,
                _ => false,
            })
}

impl<'a> ExtraMeter<'a> {
    /// The recent extra equal to `extra`, if any; otherwise counts it.
    #[inline]
    fn add(&mut self, extra: &'a Arc<CellExtra>) -> Option<&'a Arc<CellExtra>> {
        for recent in [self.last, self.before].into_iter().flatten() {
            if same_extra(recent, extra) {
                return Some(recent);
            }
        }
        (self.before, self.last) = (self.last, Some(extra));
        self.bytes += EXTRA_ALLOCATION + extra.combining.len();
        None
    }
}

fn extra_units(bytes: usize) -> u16 {
    u16::try_from(bytes.div_ceil(EXTRA_UNIT)).unwrap_or(u16::MAX)
}

impl Row {
    /// Heap bytes of the stored form (dense cells or packed runs, and the
    /// metadata table) plus the metadata charged by [`Self::measure_extras`],
    /// excluding a transient decoded read cache. Interned hyperlinks are
    /// bounded by their own table. History rows don't change, so a row keeps
    /// its charge until it leaves history or is measured again.
    pub(super) fn heap_bytes(&self) -> usize {
        let extras = usize::from(self.extra_units) * EXTRA_UNIT;
        self.cells.capacity() * size_of::<Cell>()
            + self.extras.capacity() * size_of::<(usize, Arc<CellExtra>)>()
            + extras
            + self.packed.as_ref().map_or(0, |packed| {
                size_of::<PackedCells>()
                    + packed.scalars.capacity() * size_of::<Scalar>()
                    + packed.styles.capacity() * size_of::<(u32, Style)>()
            })
    }

    /// Charge a row entering history its metadata. Plain rows have none;
    /// others are measured from their table, never by scanning cells.
    #[inline]
    pub(super) fn bound_extras(&mut self) {
        if self.has_extras() {
            self.measure_extras();
        } else {
            self.extra_units = 0;
        }
    }

    /// Charge the measured metadata.
    pub(super) fn measure_extras(&mut self) {
        self.share_equal_extras();
        self.extra_units = self.count_extra_units();
    }

    pub(super) fn count_extra_units(&self) -> u16 {
        let mut meter = ExtraMeter::default();
        for (_, extra) in &self.extras {
            meter.add(extra);
        }
        extra_units(meter.bytes)
    }

    /// Point each extra the meter would not count at the equal extra it
    /// matched, so one it charges once is also stored once.
    fn share_equal_extras(&mut self) {
        let mut replace = Vec::new();
        let mut meter = ExtraMeter::default();
        for (index, (_, extra)) in self.extras.iter().enumerate() {
            if let Some(recent) = meter.add(extra)
                && !Arc::ptr_eq(recent, extra)
            {
                replace.push((index, Arc::clone(recent)));
            }
        }
        for (index, extra) in replace {
            self.extras[index].1 = extra;
        }
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

    /// [`Self::with_cells`], with the cells' metadata.
    pub(in crate::terminal_engine) fn with_line<T>(
        &self,
        scratch: &mut Vec<Cell>,
        read: impl FnOnce(Line<'_>) -> T,
    ) -> T {
        self.with_cells(scratch, |cells| read(Line::new(cells, &self.extras)))
    }

    /// Where this row's erase blanks start, and their background: every
    /// cell from that column on is a blank on that background. Packed rows
    /// report none.
    pub(in crate::terminal_engine) fn blank_suffix(&self) -> (usize, Color) {
        if self.packed.is_some() {
            return (usize::MAX, self.clear_background);
        }
        let start = self.occupied.min(self.cells.len());
        debug_assert!(
            self.cells[start..]
                .iter()
                .all(|cell| *cell == super::Grid::blank_on(self.clear_background))
                && self.extras.last().is_none_or(|(col, _)| *col < start),
            "cells past `occupied` are not erase blanks"
        );
        (start, self.clear_background)
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
                || self
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
            || self.cells[..len]
                .iter()
                .any(|cell| cell.flags & Cell::LEADING_WIDE_SPACER != 0)
            || self.extras.iter().any(|(col, extra)| {
                self.cells[*col].flags & Cell::WIDE == 0 && !extra.combining.is_empty()
            })
        {
            return false;
        }
        self.cells.truncate(len);
        self.cells.resize(cols, default);
        self.occupied = len;
        true
    }

    /// Capacities of the stored form: dense cells or packed scalars, packed
    /// style runs, and the metadata table. [`Self::heap_bytes`] and packing
    /// reuse depend on them, so snapshots restore them exactly.
    pub(super) fn storage_capacities(&self) -> [usize; 3] {
        let (cells, styles) = self
            .packed
            .as_ref()
            .map_or((self.cells.capacity(), 0), |packed| {
                (packed.scalars.capacity(), packed.styles.capacity())
            });
        [cells, styles, self.extras.capacity()]
    }

    pub(super) fn set_storage_capacities(&mut self, [cells, styles, extras]: [usize; 3]) {
        fn fit<T>(vec: &mut Vec<T>, capacity: usize) {
            let capacity = capacity.max(vec.len());
            if vec.capacity() < capacity {
                vec.reserve_exact(capacity - vec.len());
            } else if vec.capacity() > capacity {
                vec.shrink_to(capacity);
            }
        }
        if let Some(packed) = &mut self.packed {
            fit(&mut packed.scalars, cells);
            fit(&mut packed.styles, styles);
        } else {
            fit(&mut self.cells, cells);
        }
        fit(&mut self.extras, extras);
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
        );
        // Packed or not, the row is now charged its measured metadata.
        self.measure_extras();
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
        if !packed.encode(&self.cells[..self.occupied], cols, self.clear_background) {
            let mut recycled = recycled.unwrap_or_else(|| Self::new(cols, blank));
            if recycled.cells.is_empty() {
                recycled.cells.resize(cols, *blank);
                recycled.extras.clear();
                recycled.occupied = 0;
                recycled.clear_background = blank.style.background;
            }
            return (self, recycled);
        }
        // The packed row keeps its metadata; the live buffer holds its old
        // cells, which the caller clears.
        let live = Self {
            cells: std::mem::take(&mut self.cells),
            extras: Vec::new(),
            packed: None,
            occupied: self.occupied,
            clear_background: self.clear_background,
            wrapped: false,
            extra_units: 0,
        };
        self.packed = Some(packed);
        (self, live)
    }
}
#[cfg(test)]
mod tests {
    use super::super::super::types::ExtraSlot;
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
        row.set_extra(
            0,
            Some(Arc::new(CellExtra {
                combining: "🏽‍💻".into(),
                hyperlink: Some(Arc::new(Hyperlink {
                    id: "id".into(),
                    uri: "https://example.test".into(),
                })),
                ..CellExtra::default()
            })),
        );
        row.cells[1].flags = Cell::WIDE_SPACER;
        row.cells[2].character = 'x';
        row.cells[2].style.foreground = Color::rgb(1, 2, 3);
        row.occupied = 3;
        row.wrapped = true;
        let expected = row.cells.clone();
        let expected_extra = Arc::clone(row.extra(0).unwrap());
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
        // The metadata never left the row's table.
        assert_eq!(row.extras.len(), 1);
        assert!(Arc::ptr_eq(row.extra(0).unwrap(), &expected_extra));
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
        // The measurement fits in the row's padding.
        assert_eq!(size_of::<Row>(), size_of::<[usize; 8]>() + 8);
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
        for col in 0..10 {
            row.set_extra(col, Some(Arc::clone(&link)));
        }
        // Alternating glyph and spacer extras count once each.
        let color = Arc::new(CellExtra {
            underline_color: Color::indexed(1),
            ..CellExtra::default()
        });
        for col in 10..20 {
            row.set_extra(
                col,
                Some(Arc::clone(if col % 2 == 0 { &link } else { &color })),
            );
        }
        // Distinct composed cells each count their allocation and text.
        let mut combining = 0;
        for col in 20..30 {
            let mut extra = None;
            extra.push_combining(char::from_u32(0x300 + col as u32).unwrap());
            combining += extra.as_ref().unwrap().combining.capacity();
            row.set_extra(col, extra);
        }
        row.occupied = 30;
        // Measuring charges each distinct run's allocation and text, beside
        // the table itself; a row entering history is charged the same.
        let table = row.extras.capacity() * size_of::<(usize, Arc<CellExtra>)>();
        let extras = (12 * ALLOCATION + combining).div_ceil(EXTRA_UNIT) * EXTRA_UNIT;
        assert_eq!(row.heap_bytes(), plain + table);
        row.measure_extras();
        assert_eq!(row.heap_bytes(), plain + table + extras);
        row.extra_units = 0;
        row.bound_extras();
        assert_eq!(row.heap_bytes(), plain + table + extras);
        // The packed row keeps the same table and charge.
        row.compact();
        assert_eq!(row.count_extra_units(), row.extra_units);
        let packed = row.packed.as_ref().unwrap();
        assert_eq!(
            row.heap_bytes(),
            size_of::<PackedCells>()
                + packed.scalars.capacity() * size_of::<Scalar>()
                + packed.styles.capacity() * size_of::<(u32, Style)>()
                + table
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
