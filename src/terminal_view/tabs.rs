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

    pub(super) fn tab_strip_drag_viewport_width(&self, window: &Window) -> f32 {
        let viewport_width: f32 = window.viewport_size().width.into();
        if !self.show_tab_bar() {
            return viewport_width.max(0.0);
        }

        (viewport_width - TABBAR_ACTION_RAIL_WIDTH).max(0.0)
    }

    pub(super) fn effective_tab_max_width_for_viewport(
        viewport_width: f32,
        tab_count: usize,
    ) -> f32 {
        let content_width = (viewport_width - (TAB_HORIZONTAL_PADDING * 2.0)).max(TAB_MAX_WIDTH);
        let share = content_width / tab_count.max(1) as f32;
        let elastic_growth = (share - TAB_MAX_WIDTH).max(0.0) * TAB_ADAPTIVE_GROWTH_FACTOR;
        let elastic = TAB_MAX_WIDTH + elastic_growth;
        let hard_cap = (content_width * TAB_ADAPTIVE_HARD_CAP_RATIO).max(TAB_MAX_WIDTH);

        elastic.min(hard_cap)
    }

    pub(super) fn tab_display_width_for_title_with_max(title: &str, max_width: f32) -> f32 {
        let title_chars = title.trim().chars().count() as f32;
        let text_width = title_chars * TAB_TITLE_CHAR_WIDTH;
        let base_width = (TAB_TEXT_PADDING_X * 2.0) + text_width + TAB_CLOSE_SLOT_WIDTH;
        let slack_start = TAB_MIN_WIDTH - TAB_TITLE_LAYOUT_SLACK_PX;
        let slack_end = TAB_MIN_WIDTH + TAB_TITLE_LAYOUT_SLACK_PX;
        let slack_span = (slack_end - slack_start).max(f32::EPSILON);
        let slack_factor = ((slack_end - base_width) / slack_span).clamp(0.0, 1.0);
        let effective_slack = TAB_TITLE_LAYOUT_SLACK_PX * slack_factor;
        let width = base_width + effective_slack;
        width.clamp(TAB_MIN_WIDTH, max_width.max(TAB_MIN_WIDTH))
    }

    pub(super) fn tab_display_width_for_title(title: &str) -> f32 {
        Self::tab_display_width_for_title_with_max(title, TAB_MAX_WIDTH)
    }

    pub(super) fn sync_tab_display_widths_for_viewport(&mut self, viewport_width: f32) -> bool {
        let effective_max =
            Self::effective_tab_max_width_for_viewport(viewport_width, self.tabs.len());
        let mut changed = false;

        for tab in &mut self.tabs {
            let next_width = Self::tab_display_width_for_title_with_max(&tab.title, effective_max);
            if (tab.display_width - next_width).abs() <= f32::EPSILON {
                continue;
            }

            tab.display_width = next_width;
            changed = true;
        }

        changed
    }

    pub(super) fn tab_shows_close(
        is_active: bool,
        hovered_tab: Option<usize>,
        hovered_tab_close: Option<usize>,
        index: usize,
    ) -> bool {
        is_active || hovered_tab == Some(index) || hovered_tab_close == Some(index)
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
        let widths_changed =
            self.sync_tab_display_widths_for_viewport(self.tab_drag_viewport_width);

        let scrolled = self.auto_scroll_tab_strip_during_drag(pointer_x, viewport_width);
        let marker_changed = self.update_tab_drag_marker(pointer_x, cx);
        if scrolled && !marker_changed {
            cx.notify();
        }
        if widths_changed && !scrolled && !marker_changed {
            cx.notify();
        }
        if scrolled {
            self.ensure_tab_drag_autoscroll_animation(cx);
        } else {
            self.tab_drag_autoscroll_animating = false;
        }
        scrolled || marker_changed || widths_changed
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
        self.hovered_tab_close = self
            .hovered_tab_close
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
        self.hovered_tab_close = None;
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
        self.hovered_tab_close = match self.hovered_tab_close {
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

    fn assert_float_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.0001,
            "expected {expected}, got {actual}"
        );
    }

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
    fn tab_display_width_for_title_tapers_slack_for_short_titles() {
        let long_title = "x".repeat(15);
        let long_width =
            TerminalView::tab_display_width_for_title_with_max(&long_title, TAB_MAX_WIDTH * 2.0);
        let expected_long = (TAB_TEXT_PADDING_X * 2.0)
            + (long_title.chars().count() as f32 * TAB_TITLE_CHAR_WIDTH)
            + TAB_CLOSE_SLOT_WIDTH;
        assert_eq!(long_width, expected_long);

        let short_title = "x".repeat(7);
        let short_width =
            TerminalView::tab_display_width_for_title_with_max(&short_title, TAB_MAX_WIDTH * 2.0);
        let short_base = (TAB_TEXT_PADDING_X * 2.0)
            + (short_title.chars().count() as f32 * TAB_TITLE_CHAR_WIDTH)
            + TAB_CLOSE_SLOT_WIDTH;
        assert!(short_width > short_base);
        assert!(short_width < short_base + TAB_TITLE_LAYOUT_SLACK_PX);
    }

    #[test]
    fn tab_display_width_for_title_is_monotonic_near_slack_transition() {
        let width_7 = TerminalView::tab_display_width_for_title_with_max("xxxxxxx", 512.0);
        let width_8 = TerminalView::tab_display_width_for_title_with_max("xxxxxxxx", 512.0);
        let width_9 = TerminalView::tab_display_width_for_title_with_max("xxxxxxxxx", 512.0);

        assert!(width_7 < width_8);
        assert!(width_8 < width_9);
    }

    #[test]
    fn tab_display_width_for_title_with_max_uses_provided_cap() {
        let very_long_title = "x".repeat(200);
        let width = TerminalView::tab_display_width_for_title_with_max(&very_long_title, 512.0);
        assert_eq!(width, 512.0);
    }

    #[test]
    fn effective_tab_max_width_grows_for_sparse_tabs() {
        let effective = TerminalView::effective_tab_max_width_for_viewport(1600.0, 1);
        assert!(effective > TAB_MAX_WIDTH);
    }

    #[test]
    fn effective_tab_max_width_stays_baseline_for_crowded_tabs() {
        let effective = TerminalView::effective_tab_max_width_for_viewport(1600.0, 8);
        assert_float_eq(effective, TAB_MAX_WIDTH);
    }

    #[test]
    fn effective_tab_max_width_respects_hard_cap_ratio() {
        let viewport_width = 4000.0;
        let content_width = (viewport_width - (TAB_HORIZONTAL_PADDING * 2.0)).max(TAB_MAX_WIDTH);
        let expected_hard_cap = (content_width * TAB_ADAPTIVE_HARD_CAP_RATIO).max(TAB_MAX_WIDTH);
        let effective = TerminalView::effective_tab_max_width_for_viewport(viewport_width, 1);
        assert_float_eq(effective, expected_hard_cap);
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
        assert!(TerminalView::tab_shows_close(true, None, None, 1));
        assert!(TerminalView::tab_shows_close(false, Some(1), None, 1));
        assert!(TerminalView::tab_shows_close(false, None, Some(1), 1));
        assert!(!TerminalView::tab_shows_close(false, Some(2), None, 1));
        assert!(!TerminalView::tab_shows_close(false, None, Some(2), 1));
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

    #[test]
    fn tab_drop_slot_mapping_is_stable_with_adaptive_widths() {
        let effective_max = TerminalView::effective_tab_max_width_for_viewport(1500.0, 3);
        let widths = [
            TerminalView::tab_display_width_for_title_with_max(
                "~/Desktop/claudeCode/claude-code-provider-proxy/docs",
                effective_max,
            ),
            TerminalView::tab_display_width_for_title_with_max("~", effective_max),
            TerminalView::tab_display_width_for_title_with_max("~/projects/termy", effective_max),
        ];

        let first_midpoint = TAB_HORIZONTAL_PADDING + (widths[0] * 0.5);
        assert_eq!(
            TerminalView::tab_drop_slot_from_pointer_x_for_widths(
                widths,
                first_midpoint - 1.0,
                0.0,
            ),
            0
        );
        assert_eq!(
            TerminalView::tab_drop_slot_from_pointer_x_for_widths(
                widths,
                first_midpoint + 1.0,
                0.0,
            ),
            1
        );
    }
}
