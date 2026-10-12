//! Scrollback storage. History is bounded by a row limit and by a byte budget
//! that counts packed rows at their real size, so how much history survives
//! no longer depends on the current width (and a transient widen loses none).
use super::{Cell, Grid, Row, Size};

/// Heap bytes history may hold: what one million dense cells take.
pub(super) const HISTORY_BYTE_BUDGET: usize = Size::MAX_CELLS * size_of::<Cell>();
/// Queued rows packed per step when history crosses its byte budget.
const BUDGET_COMPACTION_CHUNK: usize = 256;

impl Grid {
    pub(super) fn push_history(&mut self, mut row: Row) {
        row.bound_extras();
        self.history_bytes += row.heap_bytes();
        self.history.push_back(row);
    }

    pub(super) fn pop_history_front(&mut self) -> Option<Row> {
        let row = self.history.pop_front()?;
        self.history_bytes -= row.heap_bytes();
        Some(row)
    }

    pub(super) fn pop_history_back(&mut self) -> Option<Row> {
        let row = self.history.pop_back()?;
        self.history_bytes -= row.heap_bytes();
        Some(row)
    }

    pub(super) fn clear_history_rows(&mut self) {
        self.history.clear();
        self.history.shrink_to_fit();
        self.history_bytes = 0;
    }

    /// Remeasure every row, for paths that edit history rows in place.
    pub(super) fn recount_history_bytes(&mut self) {
        self.history_bytes = self
            .history
            .iter_mut()
            .map(|row| {
                row.measure_extras();
                row.heap_bytes()
            })
            .sum();
    }

    /// Pack the oldest queued rows, then evict the oldest rows, until history
    /// fits its byte budget. Cheap when it already fits.
    #[inline]
    pub(super) fn enforce_history_bytes(&mut self) {
        if self.history_bytes > HISTORY_BYTE_BUDGET {
            self.fit_history_bytes();
        }
    }

    /// Packing stops at the first chunk that brings history under budget, so
    /// crossing it costs one chunk rather than packing every queued row.
    #[inline(never)]
    fn fit_history_bytes(&mut self) {
        while self.history_bytes > HISTORY_BYTE_BUDGET && self.needs_compaction() {
            self.compact_pending_history(BUDGET_COMPACTION_CHUNK);
        }
        let mut removed = 0;
        while self.history_bytes > HISTORY_BYTE_BUDGET && self.pop_history_front().is_some() {
            removed += 1;
        }
        if removed != 0 {
            self.pending_compaction = self.pending_compaction.min(self.history.len());
            self.display_offset = self.display_offset.min(self.history.len());
            self.effect(super::GridEffect::ClearHistory { removed });
        }
    }

    /// Pack queued history without switching scrolling rows to immediate
    /// packing, for steps forced while output is still arriving.
    pub(in crate::terminal_engine) fn compact_pending_history(&mut self, limit: usize) {
        self.pending_compaction = self.pending_compaction.min(self.history.len());
        let start = self.history.len() - self.pending_compaction;
        let count = limit.min(self.pending_compaction);
        for row in self.history.iter_mut().skip(start).take(count) {
            let before = row.heap_bytes();
            // Also replaces a bound charged on entry with the measured bytes.
            row.compact();
            self.history_bytes = self.history_bytes - before + row.heap_bytes();
        }
        self.pending_compaction -= count;
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Color, tests::grid};
    use super::*;
    use std::{collections::HashSet, sync::Arc};

    fn line(grid: &mut Grid, text: &[u8]) {
        grid.write_ascii(text);
        grid.carriage_return();
        grid.linefeed();
    }

    #[test]
    fn crossing_the_byte_budget_packs_one_chunk_of_the_oldest_queued_rows() {
        // Pen metadata makes dense rows enter history charged a bound, which
        // may cost earlier packing but no history.
        for colored in [false, true] {
            let mut grid = grid(120, 4, 20_000);
            if colored {
                grid.pen.set_underline_color(Color::indexed(1));
            }
            let mut lines = 0;
            // The oldest queued rows are packed first.
            while !grid.history.front().is_some_and(Row::is_packed) {
                line(&mut grid, b"a short log line");
                lines += 1;
            }
            // The crossing packed one chunk and evicted nothing; newer queued
            // rows stay dense for the idle steps.
            let packed = grid.history.iter().filter(|row| row.is_packed()).count();
            assert!(packed <= BUDGET_COMPACTION_CHUNK, "{packed}");
            assert!(grid.history.iter().take(packed).all(Row::is_packed));
            assert_eq!(grid.pending_compaction, grid.history.len() - packed);
            assert_eq!(grid.history_size(), lines - 3);
            assert!(grid.history_bytes <= HISTORY_BYTE_BUDGET);

            // Later crossings keep packing chunks instead of evicting rows.
            for _ in 0..4000 {
                line(&mut grid, b"a short log line");
                lines += 1;
            }
            assert_eq!(grid.history_size(), lines - 3);
            assert!(grid.history_bytes <= HISTORY_BYTE_BUDGET);
            let bytes: usize = grid.history.iter().map(Row::heap_bytes).sum();
            assert_eq!(grid.history_bytes, bytes);
        }
    }

    #[test]
    fn combining_heavy_history_counts_its_metadata_against_the_budget() {
        let mut grid = grid(120, 4, 20_000);
        let mut mark = 0;
        for _ in 0..3000 {
            for _ in 0..119 {
                grid.put_char('e');
                // Distinct two-mark suffixes defeat the composed-cell cache.
                for shift in [0, 7] {
                    grid.put_char(char::from_u32(0x300 + (mark >> shift) % 112).unwrap());
                }
                mark += 1;
            }
            grid.carriage_return();
            grid.linefeed();
        }
        assert!(grid.history_size() < 3000, "{}", grid.history_size());
        assert!(grid.history_bytes <= HISTORY_BYTE_BUDGET);
        let bytes: usize = grid.history.iter().map(Row::heap_bytes).sum();
        assert_eq!(grid.history_bytes, bytes);
        // The accounted bytes cover what history really holds: its cells and
        // every distinct extra (allocation and text) they reference.
        let mut seen = HashSet::new();
        let mut real = 0;
        for row in &grid.history {
            real += row.cells.capacity() * size_of::<Cell>();
            for extra in row.cells().iter().filter_map(|cell| cell.extra.as_ref()) {
                if seen.insert(Arc::as_ptr(extra)) {
                    real += 2 * size_of::<usize>()
                        + size_of_val(extra.as_ref())
                        + extra.combining.capacity();
                }
            }
        }
        assert!(
            real <= grid.history_bytes,
            "{real} > {}",
            grid.history_bytes
        );
    }
}
