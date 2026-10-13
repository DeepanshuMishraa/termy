//! Output floods: a line feed scrolls one full-screen row at a time into
//! history at its row limit, and the row it scrolls in is blanked lazily, so
//! the next line's text is not stored on top of a fresh blank.
use super::{Cell, Color, Grid, GridEffect, Row, Style};

/// Leading cells of a live row that must still become plain blanks.
#[derive(Clone, Copy)]
pub(super) struct PendingClear {
    pub(super) row: usize,
    pub(super) end: usize,
    pub(super) style: Style,
}

impl Grid {
    /// Fill a row a line feed left partly unblanked. Every grid operation
    /// other than the text writers that complete it requires this first, and
    /// the engine flushes before anything outside parsing can read the grid.
    #[inline]
    pub(in crate::terminal_engine) fn flush_pending_clear(&mut self) {
        if let Some(pending) = self.pending_clear.take() {
            self.fill_pending_clear(pending, 0, 0);
        }
    }

    /// Blank a pending row's leading cells except `skip_start..skip_end`,
    /// which the caller just wrote.
    #[inline(never)]
    pub(super) fn fill_pending_clear(
        &mut self,
        pending: PendingClear,
        skip_start: usize,
        skip_end: usize,
    ) {
        let cells = &mut self.screen_mut().rows[pending.row].cells;
        let end = pending.end.min(cells.len());
        if skip_start >= skip_end {
            Cell::fill_plain(&mut cells[..end], ' ', pending.style);
            return;
        }
        Cell::fill_plain(&mut cells[..skip_start.min(end)], ' ', pending.style);
        Cell::fill_plain(&mut cells[skip_end.min(end)..end], ' ', pending.style);
    }

    /// The pending clear of `row`, for a text write about to overwrite part
    /// of it; any other pending clear is applied now.
    #[inline]
    pub(super) fn take_pending_clear(&mut self, row: usize) -> Option<PendingClear> {
        let pending = self.pending_clear.take()?;
        if pending.row == row {
            return Some(pending);
        }
        self.fill_pending_clear(pending, 0, 0);
        None
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
    #[inline]
    pub(super) fn scroll_line_into_full_history(&mut self) {
        self.flush_pending_clear();
        let cols = self.size.cols;
        let rows = self.size.rows;
        let history_before = self.history.len();
        let removed = self.primary.rows.pop_front().expect("screen row");
        let mut recycled = self.pop_history_front().expect("history at its limit");
        self.push_history(removed);
        self.history_activity = true;
        self.pending_compaction = (self.pending_compaction + 1).min(self.history.len());
        let blank = Cell::default();
        let deferred = if self.defer_scroll_clear {
            recycled.clear_deferring_fill(cols, &blank)
        } else {
            recycled.clear(cols, &blank);
            None
        };
        self.primary.rows.push_back(recycled);
        self.pending_clear = deferred.map(|end| PendingClear {
            row: rows - 1,
            end,
            style: blank.style,
        });
        self.mark_scroll_damage(0, rows, 1);
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
                self.pending_compaction = (self.pending_compaction + 1).min(self.history.len());
                if self.display_offset != 0 {
                    self.display_offset = (self.display_offset + 1).min(self.history.len());
                }
                recycled
            } else {
                removed
            };
            let deferred = if defer_clear {
                recycled.clear_deferring_fill(cols, &blank)
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
            self.pending_clear = deferred.map(|end| PendingClear {
                row: bottom - 1,
                end,
                style: blank.style,
            });
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
