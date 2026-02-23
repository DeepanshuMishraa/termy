use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TabDropMarkerSide {
    Left,
    Right,
}

impl TerminalView {
    fn clear_tab_drag_preview_state(&mut self) {
        self.tab_drag_pointer_x = None;
        self.tab_drag_viewport_width = 0.0;
        self.tab_drag_autoscroll_animating = false;
    }

    fn ensure_tab_drag_autoscroll_animation(&mut self, cx: &mut Context<Self>) {
        if self.tab_drag_autoscroll_animating {
            return;
        }
        self.tab_drag_autoscroll_animating = true;

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                smol::Timer::after(Duration::from_millis(16)).await;
                let keep_animating = match cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        if !view.tab_drag_autoscroll_animating || view.tab_drag.is_none() {
                            view.tab_drag_autoscroll_animating = false;
                            return false;
                        }

                        let Some(pointer_x) = view.tab_drag_pointer_x else {
                            view.tab_drag_autoscroll_animating = false;
                            return false;
                        };
                        let viewport_width = view.tab_drag_viewport_width;
                        let scrolled =
                            view.auto_scroll_tab_strip_during_drag(pointer_x, viewport_width);
                        let marker_changed = view.update_tab_drag_marker(pointer_x, cx);
                        if scrolled && !marker_changed {
                            cx.notify();
                        }
                        if !scrolled {
                            view.tab_drag_autoscroll_animating = false;
                            return false;
                        }
                        true
                    })
                }) {
                    Ok(keep_animating) => keep_animating,
                    _ => break,
                };

                if !keep_animating {
                    break;
                }
            }
        })
        .detach();
    }

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
            self.clear_tab_drag_preview_state();
            self.tab_drag = Some(TabDragState {
                source_index: index,
                drop_slot: None,
            });
        }
    }

    pub(super) fn finish_tab_drag(&mut self) -> bool {
        let marker_was_visible = self
            .tab_drag
            .as_ref()
            .and_then(|drag| drag.drop_slot)
            .is_some();
        self.tab_drag = None;
        self.clear_tab_drag_preview_state();
        marker_was_visible
    }

    fn tab_drop_slot_from_pointer_x_for_widths(
        tab_widths: impl IntoIterator<Item = f32>,
        pointer_x: f32,
        scroll_offset_x: f32,
    ) -> usize {
        let mut left = TAB_HORIZONTAL_PADDING + scroll_offset_x;
        let mut slot = 0;

        for width in tab_widths {
            let midpoint_x = left + (width * 0.5);
            if pointer_x < midpoint_x {
                return slot;
            }

            left += width + TAB_ITEM_GAP;
            slot += 1;
        }

        slot
    }

    fn tab_drop_slot_from_pointer_x(&self, pointer_x: f32) -> usize {
        let scroll_offset_x: f32 = self.tab_strip_scroll_handle.offset().x.into();
        Self::tab_drop_slot_from_pointer_x_for_widths(
            self.tabs.iter().map(|tab| tab.display_width),
            pointer_x,
            scroll_offset_x,
        )
    }

    fn normalized_drop_slot(source_index: usize, raw_slot: usize) -> Option<usize> {
        if raw_slot == source_index || raw_slot == source_index.saturating_add(1) {
            return None;
        }
        Some(raw_slot)
    }

    fn reorder_target_index_for_drop_slot(source_index: usize, drop_slot: usize) -> usize {
        if drop_slot > source_index {
            drop_slot - 1
        } else {
            drop_slot
        }
    }

    fn tab_drop_marker_side_for_slot(index: usize, drop_slot: usize) -> Option<TabDropMarkerSide> {
        if drop_slot == index {
            Some(TabDropMarkerSide::Left)
        } else if drop_slot == index.saturating_add(1) {
            Some(TabDropMarkerSide::Right)
        } else {
            None
        }
    }

    pub(super) fn tab_drop_marker_side(&self, index: usize) -> Option<TabDropMarkerSide> {
        if index >= self.tabs.len() {
            return None;
        }

        let drop_slot = self.tab_drag.and_then(|drag| drag.drop_slot)?;
        Self::tab_drop_marker_side_for_slot(index, drop_slot)
    }

    fn update_tab_drag_marker(&mut self, pointer_x: f32, cx: &mut Context<Self>) -> bool {
        let Some(source_index) = self.tab_drag.map(|drag| drag.source_index) else {
            return false;
        };

        let raw_drop_slot = self.tab_drop_slot_from_pointer_x(pointer_x);
        let next_drop_slot = Self::normalized_drop_slot(source_index, raw_drop_slot);

        let Some(drag) = self.tab_drag.as_mut() else {
            return false;
        };
        if drag.drop_slot == next_drop_slot {
            return false;
        }

        drag.drop_slot = next_drop_slot;
        cx.notify();
        true
    }

    fn auto_scroll_tab_strip_during_drag(&mut self, pointer_x: f32, viewport_width: f32) -> bool {
        if self.tab_drag.is_none() || viewport_width <= f32::EPSILON {
            return false;
        }

        let max_scroll: f32 = self.tab_strip_scroll_handle.max_offset().width.into();
        if max_scroll <= f32::EPSILON {
            return false;
        }

        let edge = TAB_DRAG_AUTOSCROLL_EDGE_WIDTH
            .min(viewport_width * 0.5)
            .max(f32::EPSILON);
        let left_strength = ((edge - pointer_x) / edge).clamp(0.0, 1.0);
        let right_start = (viewport_width - edge).max(0.0);
        let right_strength = ((pointer_x - right_start) / edge).clamp(0.0, 1.0);
        let delta = (right_strength - left_strength) * TAB_DRAG_AUTOSCROLL_MAX_STEP;
        if delta.abs() <= f32::EPSILON {
            return false;
        }

        let offset = self.tab_strip_scroll_handle.offset();
        let current_scroll = -Into::<f32>::into(offset.x);
        let next_scroll = (current_scroll + delta).clamp(0.0, max_scroll);
        if (next_scroll - current_scroll).abs() <= f32::EPSILON {
            return false;
        }

        self.tab_strip_scroll_handle
            .set_offset(point(px(-next_scroll), offset.y));
        true
    }

    pub(super) fn update_tab_drag_preview(
        &mut self,
        pointer_x: f32,
        viewport_width: f32,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.tab_drag.is_none() {
            return false;
        }
        self.tab_drag_pointer_x = Some(pointer_x);
        self.tab_drag_viewport_width = viewport_width.max(0.0);

        let scrolled = self.auto_scroll_tab_strip_during_drag(pointer_x, viewport_width);
        let marker_changed = self.update_tab_drag_marker(pointer_x, cx);
        if scrolled && !marker_changed {
            cx.notify();
        }
        if scrolled {
            self.ensure_tab_drag_autoscroll_animation(cx);
        } else {
            self.tab_drag_autoscroll_animating = false;
        }
        scrolled || marker_changed
    }

    pub(super) fn commit_tab_drag(&mut self, cx: &mut Context<Self>) {
        let drag = self.tab_drag.take();
        self.clear_tab_drag_preview_state();
        let Some(TabDragState {
            source_index,
            drop_slot,
        }) = drag
        else {
            return;
        };

        let Some(drop_slot) = drop_slot else {
            return;
        };

        let target_index = Self::reorder_target_index_for_drop_slot(source_index, drop_slot);
        if source_index == target_index {
            cx.notify();
            return;
        }

        if !self.reorder_tab(source_index, target_index, cx) {
            cx.notify();
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
        self.finish_tab_drag();
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
        self.finish_tab_drag();

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
    fn normalized_drop_slot_filters_noop_boundaries() {
        assert_eq!(TerminalView::normalized_drop_slot(2, 2), None);
        assert_eq!(TerminalView::normalized_drop_slot(2, 3), None);
        assert_eq!(TerminalView::normalized_drop_slot(2, 1), Some(1));
        assert_eq!(TerminalView::normalized_drop_slot(2, 4), Some(4));
    }

    #[test]
    fn reorder_target_index_for_drop_slot_moves_right_correctly() {
        assert_eq!(TerminalView::reorder_target_index_for_drop_slot(1, 3), 2);
        assert_eq!(TerminalView::reorder_target_index_for_drop_slot(0, 3), 2);
    }

    #[test]
    fn reorder_target_index_for_drop_slot_moves_left_correctly() {
        assert_eq!(TerminalView::reorder_target_index_for_drop_slot(3, 1), 1);
        assert_eq!(TerminalView::reorder_target_index_for_drop_slot(2, 0), 0);
    }

    #[test]
    fn tab_drop_slot_from_pointer_x_respects_midpoints() {
        let widths = [100.0, 100.0, 100.0];
        assert_eq!(
            TerminalView::tab_drop_slot_from_pointer_x_for_widths(widths, 40.0, 0.0),
            0
        );
        assert_eq!(
            TerminalView::tab_drop_slot_from_pointer_x_for_widths(widths, 70.0, 0.0),
            1
        );
        assert_eq!(
            TerminalView::tab_drop_slot_from_pointer_x_for_widths(widths, 170.0, 0.0),
            2
        );
        assert_eq!(
            TerminalView::tab_drop_slot_from_pointer_x_for_widths(widths, 270.0, 0.0),
            3
        );
    }

    #[test]
    fn tab_drop_slot_from_pointer_x_respects_scroll_offset() {
        let widths = [100.0, 100.0];
        assert_eq!(
            TerminalView::tab_drop_slot_from_pointer_x_for_widths(widths, 40.0, 0.0),
            0
        );
        assert_eq!(
            TerminalView::tab_drop_slot_from_pointer_x_for_widths(widths, 40.0, -30.0),
            1
        );
    }

    #[test]
    fn tab_drop_marker_side_maps_slot_to_left_and_right_edges() {
        assert_eq!(
            TerminalView::tab_drop_marker_side_for_slot(2, 2),
            Some(TabDropMarkerSide::Left)
        );
        assert_eq!(
            TerminalView::tab_drop_marker_side_for_slot(2, 3),
            Some(TabDropMarkerSide::Right)
        );
        assert_eq!(TerminalView::tab_drop_marker_side_for_slot(2, 1), None);
    }
}
