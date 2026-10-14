//! Output floods: a line feed scrolls one full-screen row at a time into
//! history at its row limit, and the row it scrolls in is blanked lazily, so
//! the next line's text is not stored on top of a fresh blank.
use super::{Cell, Color, Grid, GridEffect, Row, Style};

/// Cells `start..end` of a live row that must still become plain blanks.
/// An `end` of zero leaves nothing to fill, so it also means none is
/// pending. The grid stores this by value and reads it a field at a time:
/// it is written while the line's cell stores drain, and a wider read would
/// wait for all of them.
#[derive(Clone, Copy)]
pub(super) struct PendingClear {
    pub(super) row: usize,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) background: Color,
}

impl PendingClear {
    pub(super) const NONE: Self = Self {
        row: 0,
        start: 0,
        end: 0,
        background: Color::DEFAULT,
    };
}

/// The blank a full-screen scroll clears with while the pen has the default
/// background. A constant, so reading it never waits on recent stores.
const DEFAULT_BLANK: Cell = Cell {
    style: Style {
        foreground: Color::DEFAULT,
        background: Color::DEFAULT,
        attributes: 0,
        underline: super::super::types::UnderlineStyle::None,
    },
    flags: 0,
    character: ' ',
};

impl Grid {
    /// Fill a row a line feed left partly unblanked. Every grid operation
    /// other than the text writers that complete it requires this first, and
    /// the engine flushes before anything outside parsing can read the grid.
    #[inline]
    pub(in crate::terminal_engine) fn flush_pending_clear(&mut self) {
        if self.pending_clear.end != 0 {
            let pending = self.pending_clear;
            self.pending_clear.end = 0;
            self.fill_pending_clear(pending.row, pending.start, pending.end, pending.background);
        }
    }

    #[inline]
    pub(super) fn has_pending_clear(&self) -> bool {
        self.pending_clear.end != 0
    }

    #[inline(never)]
    fn fill_pending_clear(&mut self, row: usize, start: usize, end: usize, background: Color) {
        let cells = &mut self.screen_mut().rows[row].cells;
        let end = end.min(cells.len());
        if start < end {
            let style = Style {
                background,
                ..Style::default()
            };
            Cell::fill_plain(&mut cells[start..end], ' ', style);
        }
    }

    /// Account for text a writer just stored at `start..end` of `row`, the
    /// cursor row. A pending clear of another row is applied; on this row
    /// the stale cells left of the text are blanked, and those right of it
    /// stay pending, since the line's next run usually covers them.
    #[inline]
    pub(super) fn write_over_pending_clear(&mut self, row: usize, start: usize, end: usize) {
        if self.pending_clear.end == 0 {
            return;
        }
        if self.pending_clear.row != row {
            self.flush_pending_clear();
            return;
        }
        let pending = self.pending_clear;
        if pending.start < start {
            let left = start.min(pending.end);
            self.fill_pending_clear(row, pending.start, left, pending.background);
        }
        let rest = pending.start.max(end);
        if rest < pending.end {
            self.pending_clear.start = rest;
        } else {
            self.pending_clear.end = 0;
        }
    }

    /// LF from output: a row it scrolls in is blanked lazily, so the line
    /// written next isn't stored twice. See `flush_pending_clear`.
    pub(in crate::terminal_engine) fn linefeed_deferring_clear(&mut self) {
        self.flush_pending_clear();
        self.defer_scroll_clear = true;
        self.linefeed();
        self.defer_scroll_clear = false;
    }

    /// `scroll_region_up` of one full-screen line, retaining it in history
    /// at its row limit: the oldest history row is recycled as the new line.
    /// The viewport shows the live screen.
    #[inline]
    pub(super) fn scroll_line_into_full_history(&mut self) {
        self.flush_pending_clear();
        let cols = self.size.cols;
        let rows = self.size.rows;
        let history_before = self.history.len();
        // The top row joins history and the oldest history row becomes the
        // new bottom row: the two trade places, then each ring's first slot
        // becomes its last, which moves no row while the ring is exactly
        // full. Rows moved through the stack, or edited before a move, are
        // read back across narrower stores, which waits for every cell store
        // the line queued; the trade reads two rows no recent line wrote.
        if self.history.capacity() != self.history.len() {
            self.history.shrink_to_fit();
        }
        if self.primary.rows.capacity() != self.primary.rows.len() {
            self.primary.rows.shrink_to_fit();
        }
        let newest = self.history.front_mut().expect("history at its limit");
        let recycled = self.primary.rows.front_mut().expect("screen row");
        // Copied whole, as `swap`'s word-sized steps take twice the stores.
        // SAFETY: `newest` is read out and written back with `recycled`'s
        // row before anything else reads it; nothing in between can panic.
        unsafe {
            let row = std::ptr::read(newest);
            std::ptr::write(newest, std::mem::replace(recycled, row));
        }
        // Charged where they now stay.
        self.history_bytes -= recycled.heap_bytes();
        newest.bound_extras();
        self.history_bytes += newest.heap_bytes();
        if self.defer_scroll_clear {
            let end = recycled.clear_deferring_fill(cols, &DEFAULT_BLANK);
            self.pending_clear = PendingClear {
                row: rows - 1,
                start: 0,
                end,
                background: Color::DEFAULT,
            };
        } else {
            recycled.clear(cols, &DEFAULT_BLANK);
        }
        // Rotating moves neither row from the slot it was edited in above.
        self.history.rotate_front_to_back();
        self.primary.rows.rotate_front_to_back();
        self.history_activity = true;
        self.scrolled_into_history = self.scrolled_into_history.wrapping_add(1);
        self.pending_compaction = (self.pending_compaction + 1).min(history_before);
        // The caller checked that the viewport shows the live screen.
        self.mark_live_scroll_damage();
        if self.track_effects {
            self.effect(GridEffect::Scroll {
                alternate: false,
                top: 0,
                bottom: rows,
                lines: 1,
                retains_history: true,
                history_before,
                history_after: self.history.len(),
            });
        }
        self.enforce_history_bytes();
    }

    pub(super) fn scroll_region_up(
        &mut self,
        top: usize,
        bottom: usize,
        count: usize,
        retain_history: bool,
    ) {
        // Output floods scroll one full-screen line at a time into history
        // that is already at its row limit.
        if count == 1
            && top == 0
            && bottom == self.size.rows
            && retain_history
            && self.history_limit != 0
            && self.history.len() == self.history_limit
            && !self.compact_on_scroll
            && self.display_offset == 0
            && self.pen.style.background == Color::DEFAULT
            && self.screen().rows.len() == self.size.rows
        {
            self.scroll_line_into_full_history();
            return;
        }
        self.scroll_region_up_general(top, bottom, count, retain_history);
    }

    #[inline(never)]
    fn scroll_region_up_general(
        &mut self,
        top: usize,
        bottom: usize,
        count: usize,
        retain_history: bool,
    ) {
        let blank = self.blank();
        let cols = self.size.cols;
        let count = count.min(bottom - top);
        let history_before = self.history_size();
        self.flush_pending_clear();
        // A full-screen region rotates the ring instead of shifting rows.
        let whole_screen = top == 0 && bottom == self.screen().rows.len();
        let defer_clear = self.defer_scroll_clear && count == 1 && whole_screen;
        for _ in 0..count {
            let rows = &mut self.screen_mut().rows;
            let removed = if whole_screen {
                rows.pop_front()
            } else {
                rows.remove(top)
            }
            .expect("scroll region row");
            let mut recycled = if retain_history && self.history_limit != 0 {
                let recycled = if self.history.len() == self.history_limit {
                    self.pop_history_front()
                } else {
                    None
                };
                let (retained, recycled) = if self.compact_on_scroll {
                    removed.retain(recycled, cols, &blank)
                } else {
                    (removed, recycled.unwrap_or_else(|| Row::new(cols, &blank)))
                };
                self.push_history(retained);
                self.history_activity = true;
                self.scrolled_into_history = self.scrolled_into_history.wrapping_add(1);
                self.pending_compaction = (self.pending_compaction + 1).min(self.history.len());
                if self.display_offset != 0 {
                    self.display_offset = (self.display_offset + 1).min(self.history.len());
                }
                recycled
            } else {
                removed
            };
            let deferred = if defer_clear {
                Some(recycled.clear_deferring_fill(cols, &blank))
            } else {
                recycled.clear(cols, &blank);
                None
            };
            let rows = &mut self.screen_mut().rows;
            if whole_screen {
                rows.push_back(recycled);
            } else {
                rows.insert(bottom - 1, recycled);
            }
            if let Some(end) = deferred {
                self.pending_clear = PendingClear {
                    row: bottom - 1,
                    start: 0,
                    end,
                    background: blank.style.background,
                };
            }
        }
        if count != 0 {
            self.mark_scroll_damage(top, bottom, count as i32);
            self.effect(GridEffect::Scroll {
                alternate: self.alternate_active,
                top,
                bottom,
                lines: count as i64,
                retains_history: retain_history && self.history_limit != 0,
                history_before,
                history_after: self.history_size(),
            });
            self.enforce_history_bytes();
        }
    }
}
