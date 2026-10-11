//! Scrollback storage. History is bounded by a row limit and by a byte budget
//! that counts packed rows at their real size, so how much history survives
//! no longer depends on the current width (and a transient widen loses none).
use super::{Cell, Grid, Row, Size};

/// Heap bytes history may hold: what one million dense cells take.
pub(super) const HISTORY_BYTE_BUDGET: usize = Size::MAX_CELLS * size_of::<Cell>();

impl Grid {
    pub(super) fn push_history(&mut self, row: Row) {
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

    pub(super) fn recount_history_bytes(&mut self) {
        self.history_bytes = self.history.iter().map(Row::heap_bytes).sum();
    }

    /// Pack queued rows, then evict the oldest, until history fits its byte
    /// budget. Cheap when it already fits.
    pub(super) fn enforce_history_bytes(&mut self) {
        if self.history_bytes <= HISTORY_BYTE_BUDGET {
            return;
        }
        self.compact_pending_history(usize::MAX);
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
            row.compact();
            self.history_bytes = self.history_bytes - before + row.heap_bytes();
        }
        self.pending_compaction -= count;
    }
}
