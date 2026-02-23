use super::*;

impl TerminalView {
    pub(super) fn tab_display_width_for_title(title: &str) -> f32 {
        let title_chars = title.trim().chars().count() as f32;
        let text_width = title_chars * TAB_TITLE_CHAR_WIDTH;
        let width = (TAB_TEXT_PADDING_X * 2.0) + text_width + TAB_CLOSE_SLOT_WIDTH;
        width.clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH)
    }

    pub(super) fn tab_shows_close(
        is_active: bool,
        hovered_tab: Option<usize>,
        index: usize,
    ) -> bool {
        is_active || hovered_tab == Some(index)
    }

    fn remap_index_after_move(index: usize, from: usize, to: usize) -> usize {
        if index == from {
            return to;
        }

        if from < to {
            if (from + 1..=to).contains(&index) {
                return index - 1;
            }
            index
        } else if (to..from).contains(&index) {
            index + 1
        } else {
            index
        }
    }

    pub(super) fn begin_tab_drag(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.tab_drag = Some(TabDragState {
                dragged_index: index,
            });
        }
    }

    pub(super) fn finish_tab_drag(&mut self) {
        self.tab_drag = None;
    }

    fn drag_reorder_crosses_threshold(
        dragged_index: usize,
        target_index: usize,
        pointer_x: f32,
        target_midpoint_x: f32,
    ) -> bool {
        if target_index > dragged_index {
            pointer_x >= target_midpoint_x
        } else if target_index < dragged_index {
            pointer_x <= target_midpoint_x
        } else {
            false
        }
    }

    fn tab_midpoint_x(&self, index: usize) -> Option<f32> {
        if index >= self.tabs.len() {
            return None;
        }

        let scroll_offset_x: f32 = self.tab_strip_scroll_handle.offset().x.into();
        let mut left = TAB_HORIZONTAL_PADDING + scroll_offset_x;
        for tab in self.tabs.iter().take(index) {
            left += tab.display_width + TAB_ITEM_GAP;
        }

        Some(left + (self.tabs[index].display_width * 0.5))
    }

    pub(super) fn drag_tab_to(
        &mut self,
        target_index: usize,
        pointer_x: f32,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.tab_drag else {
            return;
        };

        if target_index >= self.tabs.len() || drag.dragged_index == target_index {
            return;
        }

        let Some(target_midpoint_x) = self.tab_midpoint_x(target_index) else {
            return;
        };

        if !Self::drag_reorder_crosses_threshold(
            drag.dragged_index,
            target_index,
            pointer_x,
            target_midpoint_x,
        ) {
            return;
        }

        if self.reorder_tab(drag.dragged_index, target_index, cx) {
            self.tab_drag = Some(TabDragState {
                dragged_index: target_index,
            });
        }
    }

    pub(super) fn reorder_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) -> bool {
        if from >= self.tabs.len() || to >= self.tabs.len() || from == to {
            return false;
        }

        let moved_tab = self.tabs.remove(from);
        self.tabs.insert(to, moved_tab);

        self.active_tab = Self::remap_index_after_move(self.active_tab, from, to);
        self.renaming_tab = self
            .renaming_tab
            .map(|index| Self::remap_index_after_move(index, from, to));
        self.hovered_tab = self
            .hovered_tab
            .map(|index| Self::remap_index_after_move(index, from, to));

        if let Some(drag) = &mut self.tab_drag {
            drag.dragged_index = Self::remap_index_after_move(drag.dragged_index, from, to);
        }

        self.scroll_active_tab_into_view();
        cx.notify();
        true
    }

    pub(super) fn add_tab(&mut self, cx: &mut Context<Self>) {
        if !self.use_tabs {
            return;
        }

        let terminal = Terminal::new(
            TerminalSize::default(),
            self.configured_working_dir.as_deref(),
            Some(self.event_wakeup_tx.clone()),
            Some(&self.tab_shell_integration),
            Some(&self.terminal_runtime),
        )
        .expect("Failed to create terminal tab");

        let predicted_prompt_cwd = Self::predicted_prompt_cwd(
            self.configured_working_dir.as_deref(),
            self.terminal_runtime.working_dir_fallback,
        );
        let predicted_title =
            Self::predicted_prompt_seed_title(&self.tab_title, predicted_prompt_cwd.as_deref());

        self.tabs.push(TerminalTab::new(terminal, predicted_title));
        self.active_tab = self.tabs.len() - 1;
        self.refresh_tab_title(self.active_tab);
        self.renaming_tab = None;
        self.rename_input.clear();
        self.inline_input_selecting = false;
        self.hovered_tab = None;
        self.tab_drag = None;
        self.clear_selection();
        self.scroll_active_tab_into_view();
        cx.notify();
    }

    pub(super) fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return;
        }

        self.tabs.remove(index);

        if self.active_tab > index {
            self.active_tab -= 1;
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }

        match self.renaming_tab {
            Some(editing) if editing == index => {
                self.renaming_tab = None;
                self.rename_input.clear();
                self.inline_input_selecting = false;
            }
            Some(editing) if editing > index => {
                self.renaming_tab = Some(editing - 1);
            }
            _ => {}
        }

        self.hovered_tab = match self.hovered_tab {
            Some(hovered) if hovered == index => None,
            Some(hovered) if hovered > index => Some(hovered - 1),
            value => value,
        };
        self.tab_drag = match self.tab_drag {
            Some(TabDragState { dragged_index }) if dragged_index == index => None,
            Some(TabDragState { dragged_index }) if dragged_index > index => Some(TabDragState {
                dragged_index: dragged_index - 1,
            }),
            value => value,
        };

        self.clear_selection();
        self.scroll_active_tab_into_view();
        cx.notify();
    }

    pub(super) fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        self.close_tab(self.active_tab, cx);
    }

    pub(super) fn begin_rename_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.use_tabs || index >= self.tabs.len() {
            return;
        }

        if self.command_palette_open {
            self.close_command_palette(cx);
        }
        if self.search_open {
            self.close_search(cx);
        }

        if self.active_tab != index {
            self.switch_tab(index, cx);
        }

        self.finish_tab_drag();
        self.renaming_tab = Some(index);
        self.rename_input.set_text(self.tabs[index].title.clone());
        self.reset_cursor_blink_phase();
        self.inline_input_selecting = false;
        cx.notify();
    }

    pub(super) fn switch_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() || index == self.active_tab {
            return;
        }

        let old_active = self.active_tab;
        self.active_tab = index;

        // Apply inactive_tab_scrollback optimization if configured
        if let Some(inactive_scrollback) = self.inactive_tab_scrollback {
            // Shrink the previously active tab's scrollback to save memory
            self.tabs[old_active]
                .terminal
                .set_scrollback_history(inactive_scrollback);

            // Restore full scrollback for the newly active tab
            self.tabs[index]
                .terminal
                .set_scrollback_history(self.terminal_runtime.scrollback_history);
        }

        self.renaming_tab = None;
        self.rename_input.clear();
        self.inline_input_selecting = false;
        self.finish_tab_drag();
        self.clear_selection();
        self.scroll_active_tab_into_view();
        cx.notify();
    }

    pub(super) fn commit_rename_tab(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.renaming_tab else {
            return;
        };

        let trimmed = self.rename_input.text().trim();
        self.tabs[index].manual_title = (!trimmed.is_empty())
            .then(|| Self::truncate_tab_title(trimmed))
            .filter(|title| !title.is_empty());
        self.refresh_tab_title(index);

        self.renaming_tab = None;
        self.rename_input.clear();
        self.inline_input_selecting = false;
        self.finish_tab_drag();
        cx.notify();
    }

    pub(super) fn cancel_rename_tab(&mut self, cx: &mut Context<Self>) {
        if self.renaming_tab.is_none() {
            return;
        }

        self.renaming_tab = None;
        self.rename_input.clear();
        self.inline_input_selecting = false;
        self.finish_tab_drag();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_display_width_for_title_clamps_to_min() {
        let width = TerminalView::tab_display_width_for_title("a");
        assert_eq!(width, TAB_MIN_WIDTH);
    }

    #[test]
    fn tab_display_width_for_title_clamps_to_max() {
        let very_long_title = "x".repeat(200);
        let width = TerminalView::tab_display_width_for_title(&very_long_title);
        assert_eq!(width, TAB_MAX_WIDTH);
    }

    #[test]
    fn remap_index_after_move_handles_move_to_right() {
        assert_eq!(TerminalView::remap_index_after_move(1, 1, 3), 3);
        assert_eq!(TerminalView::remap_index_after_move(2, 1, 3), 1);
        assert_eq!(TerminalView::remap_index_after_move(3, 1, 3), 2);
        assert_eq!(TerminalView::remap_index_after_move(0, 1, 3), 0);
    }

    #[test]
    fn remap_index_after_move_handles_move_to_left() {
        assert_eq!(TerminalView::remap_index_after_move(3, 3, 1), 1);
        assert_eq!(TerminalView::remap_index_after_move(1, 3, 1), 2);
        assert_eq!(TerminalView::remap_index_after_move(2, 3, 1), 3);
        assert_eq!(TerminalView::remap_index_after_move(4, 3, 1), 4);
    }

    #[test]
    fn tab_shows_close_for_active_or_hovered() {
        assert!(TerminalView::tab_shows_close(true, None, 1));
        assert!(TerminalView::tab_shows_close(false, Some(1), 1));
        assert!(!TerminalView::tab_shows_close(false, Some(2), 1));
    }

    #[test]
    fn drag_reorder_crosses_threshold_respects_direction() {
        assert!(!TerminalView::drag_reorder_crosses_threshold(
            0, 1, 49.0, 50.0
        ));
        assert!(TerminalView::drag_reorder_crosses_threshold(
            0, 1, 50.0, 50.0
        ));
        assert!(!TerminalView::drag_reorder_crosses_threshold(
            2, 1, 51.0, 50.0
        ));
        assert!(TerminalView::drag_reorder_crosses_threshold(
            2, 1, 50.0, 50.0
        ));
    }
}
