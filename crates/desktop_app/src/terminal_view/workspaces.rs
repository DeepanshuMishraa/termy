use super::*;

/// A workspace groups tabs under one sidebar entry. The active workspace's
/// tabs live in `TerminalView::tabs`; only inactive workspaces keep their
/// tabs stashed here. Switching workspaces swaps the whole tab vec so the
/// index-based tab machinery (strip, drag, persistence) never has to filter.
pub(crate) struct WorkspaceEntry {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) pinned: bool,
    pub(crate) tabs: Vec<TerminalTab>,
    pub(crate) active_tab: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WorkspaceSidebarResizeDragState {
    start_window_x: f32,
    start_width: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceDragState {
    source_index: usize,
    drop_slot: Option<usize>,
}

impl WorkspaceEntry {
    pub(crate) fn new(id: u64) -> Self {
        Self {
            id,
            name: format!("Workspace {id}"),
            pinned: false,
            tabs: Vec::new(),
            active_tab: 0,
        }
    }
}

impl TerminalView {
    pub(crate) fn clamp_workspace_sidebar_width(width: f32) -> f32 {
        if width.is_finite() {
            width.clamp(
                termy_config_core::MIN_SIDEBAR_WIDTH,
                termy_config_core::MAX_SIDEBAR_WIDTH,
            )
        } else {
            termy_config_core::DEFAULT_SIDEBAR_WIDTH
        }
    }

    pub(crate) fn workspace_sidebar_visible(&self) -> bool {
        self.workspace_sidebar_enabled
            && !self.workspace_sidebar_collapsed
            && !self.simple_mode
            && self.effective_tab_bar_visibility() != TabBarVisibility::ForceHidden
    }

    pub(crate) fn workspace_sidebar_overlay_visible(&self) -> bool {
        self.workspace_sidebar_enabled
            && self.workspace_sidebar_collapsed
            && self.workspace_sidebar_peek_visible
            && !self.simple_mode
            && self.effective_tab_bar_visibility() != TabBarVisibility::ForceHidden
    }

    pub(crate) fn workspace_sidebar_edge_peek_enabled(&self) -> bool {
        self.workspace_sidebar_enabled
            && self.workspace_sidebar_collapsed
            && !self.simple_mode
            && self.effective_tab_bar_visibility() != TabBarVisibility::ForceHidden
    }

    /// Width reserved on the left for the workspace sidebar. Feeds the
    /// terminal grid sizer and window-to-surface mouse coordinate mapping.
    pub(crate) fn workspace_sidebar_width(&self) -> f32 {
        if self.workspace_sidebar_visible() {
            self.workspace_sidebar_width
        } else {
            0.0
        }
    }

    pub(crate) fn set_workspace_sidebar_peek_visible(
        &mut self,
        visible: bool,
        cx: &mut Context<Self>,
    ) {
        if self.workspace_sidebar_peek_visible == visible {
            return;
        }
        self.workspace_sidebar_peek_visible = visible;
        cx.notify();
    }

    pub(crate) fn toggle_workspace_sidebar_collapsed(&mut self, cx: &mut Context<Self>) {
        if !self.workspace_sidebar_enabled {
            self.workspace_sidebar_enabled = true;
        }
        self.workspace_sidebar_collapsed = !self.workspace_sidebar_collapsed;
        if !self.workspace_sidebar_collapsed {
            self.workspace_sidebar_peek_visible = false;
        }
        self.mark_tab_strip_layout_dirty();
        cx.notify();
    }

    pub(crate) fn begin_workspace_sidebar_resize_drag(&mut self, window_x: f32) {
        self.workspace_sidebar_resize_drag = Some(WorkspaceSidebarResizeDragState {
            start_window_x: window_x,
            start_width: self.workspace_sidebar_width,
        });
    }

    pub(crate) fn workspace_sidebar_resize_drag_active(&self) -> bool {
        self.workspace_sidebar_resize_drag.is_some()
    }

    pub(crate) fn update_workspace_sidebar_resize_drag(&mut self, window_x: f32) -> bool {
        let Some(drag) = self.workspace_sidebar_resize_drag else {
            return false;
        };
        let next_width = Self::clamp_workspace_sidebar_width(
            drag.start_width + (window_x - drag.start_window_x),
        );
        if (next_width - self.workspace_sidebar_width).abs() < f32::EPSILON {
            return false;
        }
        self.workspace_sidebar_width = next_width;
        self.mark_tab_strip_layout_dirty();
        true
    }

    pub(crate) fn finish_workspace_sidebar_resize_drag(&mut self) -> bool {
        let Some(drag) = self.workspace_sidebar_resize_drag.take() else {
            return false;
        };
        if (self.workspace_sidebar_width - drag.start_width).abs() >= 1.0
            && let Err(error) = crate::config::set_root_setting(
                termy_config_core::RootSettingId::SidebarWidth,
                &format!("{:.0}", self.workspace_sidebar_width),
            )
        {
            log::error!("Failed to persist workspace sidebar width: {error}");
        }
        self.mark_tab_strip_layout_dirty();
        true
    }

    pub(crate) fn sync_workspace_sidebar_width_from_config(
        &mut self,
        configured_width: f32,
    ) -> bool {
        if self.workspace_sidebar_resize_drag.is_some() {
            return false;
        }
        let next_width = Self::clamp_workspace_sidebar_width(configured_width);
        if (next_width - self.workspace_sidebar_width).abs() < f32::EPSILON {
            return false;
        }
        self.workspace_sidebar_width = next_width;
        true
    }

    pub(crate) fn has_other_workspaces(&self) -> bool {
        self.workspaces.len() > 1
    }

    pub(crate) fn begin_workspace_drag(&mut self, index: usize) {
        if index >= self.workspaces.len() || self.renaming_workspace.is_some() {
            return;
        }
        self.workspace_drag = Some(WorkspaceDragState {
            source_index: index,
            drop_slot: None,
        });
    }

    pub(crate) fn update_workspace_drag_over(
        &mut self,
        hover_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.workspace_drag else {
            return;
        };
        if hover_index >= self.workspaces.len() || drag.source_index >= self.workspaces.len() {
            return;
        }

        let raw_slot = if hover_index > drag.source_index {
            hover_index.saturating_add(1)
        } else {
            hover_index
        };
        let clamped_slot = self.clamp_workspace_drop_slot_to_pin_group(drag.source_index, raw_slot);
        let next_drop_slot = Self::normalized_workspace_drop_slot(drag.source_index, clamped_slot);
        let Some(drag) = self.workspace_drag.as_mut() else {
            return;
        };
        if drag.drop_slot == next_drop_slot {
            return;
        }

        drag.drop_slot = next_drop_slot;
        cx.notify();
    }

    pub(crate) fn finish_workspace_drag(&mut self) -> bool {
        self.workspace_drag.take().is_some()
    }

    pub(crate) fn workspace_drop_marker_side(
        &self,
        index: usize,
    ) -> Option<crate::terminal_view::tab_strip::state::TabDropMarkerSide> {
        if index >= self.workspaces.len() {
            return None;
        }
        let drop_slot = self.workspace_drag.and_then(|drag| drag.drop_slot)?;
        if drop_slot == index {
            Some(crate::terminal_view::tab_strip::state::TabDropMarkerSide::Leading)
        } else if drop_slot == index.saturating_add(1) {
            Some(crate::terminal_view::tab_strip::state::TabDropMarkerSide::Trailing)
        } else {
            None
        }
    }

    pub(crate) fn commit_workspace_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(WorkspaceDragState {
            source_index,
            drop_slot: Some(drop_slot),
        }) = self.workspace_drag.take()
        else {
            return false;
        };
        if source_index >= self.workspaces.len() {
            return false;
        }
        let target_index =
            Self::workspace_reorder_target_index_for_drop_slot(source_index, drop_slot);
        if target_index >= self.workspaces.len() || source_index == target_index {
            return false;
        }

        let active_id = self
            .workspaces
            .get(self.active_workspace)
            .map(|entry| entry.id);
        let entry = self.workspaces.remove(source_index);
        self.workspaces.insert(target_index, entry);
        if let Some(active_id) = active_id
            && let Some(next_active) = self
                .workspaces
                .iter()
                .position(|entry| entry.id == active_id)
        {
            self.active_workspace = next_active;
        }
        self.schedule_persist_native_workspace();
        cx.notify();
        true
    }

    fn normalized_workspace_drop_slot(source_index: usize, raw_slot: usize) -> Option<usize> {
        if raw_slot == source_index || raw_slot == source_index.saturating_add(1) {
            return None;
        }
        Some(raw_slot)
    }

    fn workspace_reorder_target_index_for_drop_slot(
        source_index: usize,
        drop_slot: usize,
    ) -> usize {
        if drop_slot > source_index {
            drop_slot - 1
        } else {
            drop_slot
        }
    }

    fn workspace_pin_group_bounds(&self, source_index: usize) -> (usize, usize) {
        let Some(source) = self.workspaces.get(source_index) else {
            return (0, self.workspaces.len());
        };
        let mut start = source_index;
        while start > 0 && self.workspaces[start - 1].pinned == source.pinned {
            start -= 1;
        }
        let mut end = source_index + 1;
        while end < self.workspaces.len() && self.workspaces[end].pinned == source.pinned {
            end += 1;
        }
        (start, end)
    }

    fn clamp_workspace_drop_slot_to_pin_group(
        &self,
        source_index: usize,
        raw_slot: usize,
    ) -> usize {
        let (start, end) = self.workspace_pin_group_bounds(source_index);
        raw_slot.clamp(start, end)
    }

    fn reorder_workspaces_for_pins(&mut self) {
        if self.workspaces.len() <= 1 {
            return;
        }
        let active_id = self
            .workspaces
            .get(self.active_workspace)
            .map(|entry| entry.id);
        self.workspaces
            .sort_by_key(|entry| (!entry.pinned, entry.id));
        if let Some(active_id) = active_id
            && let Some(next_active) = self
                .workspaces
                .iter()
                .position(|entry| entry.id == active_id)
        {
            self.active_workspace = next_active;
        }
    }

    fn stash_active_workspace_tabs(&mut self) {
        let active_tab = self.active_tab;
        if let Some(entry) = self.workspaces.get_mut(self.active_workspace) {
            entry.tabs = std::mem::take(&mut self.tabs);
            entry.active_tab = active_tab;
            for tab in &mut entry.tabs {
                for pane in &mut tab.panes {
                    if let Some(state) = pane.browser_state_mut() {
                        state.editing_url = false;
                    }
                }
            }
        }
    }

    fn restore_workspace_tabs(&mut self, index: usize) {
        if let Some(entry) = self.workspaces.get_mut(index) {
            self.tabs = std::mem::take(&mut entry.tabs);
            self.active_tab = entry.active_tab.min(self.tabs.len().saturating_sub(1));
        }
        self.active_workspace = index;
    }

    fn set_tab_scrollback_options(&self, tab_index: usize, active: bool) {
        let Some(inactive_scrollback) = self.inactive_tab_scrollback else {
            return;
        };
        let active_options = self.terminal_runtime.term_options();
        let options = if active {
            active_options
        } else {
            active_options.with_scrollback_history(inactive_scrollback)
        };
        if let Some(tab) = self.tabs.get(tab_index) {
            for pane in &tab.panes {
                if let Some(terminal) = pane.maybe_terminal() {
                    terminal.set_term_options(options);
                }
            }
        }
    }

    fn reset_view_state_after_workspace_change(&mut self, cx: &mut Context<Self>) {
        self.reset_tab_rename_state();
        self.reset_workspace_rename_state();
        self.finish_workspace_drag();
        self.reset_tab_drag_state();
        self.clear_selection();
        self.clear_hovered_link();
        self.clear_terminal_scrollbar_marker_cache();
        self.mark_tab_strip_layout_dirty();
        self.sync_tab_strip_for_active_tab();
        self.schedule_persist_native_workspace();
        cx.notify();
    }

    pub(crate) fn switch_workspace(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.workspaces.len() || index == self.active_workspace {
            return;
        }

        self.set_tab_scrollback_options(self.active_tab, false);
        self.stash_active_workspace_tabs();
        self.restore_workspace_tabs(index);
        self.set_tab_scrollback_options(self.active_tab, true);
        if self.tabs.is_empty() {
            // Defensive: a workspace should never be empty, but if tab
            // restoration ever leaves one bare, give it a fresh tab instead
            // of rendering a dead surface.
            self.add_tab(cx);
        }
        self.reset_view_state_after_workspace_change(cx);
    }

    pub(crate) fn set_workspace_pinned(
        &mut self,
        index: usize,
        pinned: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(entry) = self.workspaces.get_mut(index) else {
            return false;
        };
        if entry.pinned == pinned {
            return false;
        }

        entry.pinned = pinned;
        self.finish_workspace_drag();
        self.reorder_workspaces_for_pins();
        self.mark_tab_strip_layout_dirty();
        self.schedule_persist_native_workspace();
        cx.notify();
        true
    }

    pub(crate) fn toggle_workspace_pinned(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        let Some(pinned) = self.workspaces.get(index).map(|entry| entry.pinned) else {
            return false;
        };
        self.set_workspace_pinned(index, !pinned, cx)
    }

    pub(crate) fn reset_workspace_rename_state(&mut self) -> bool {
        let was_renaming = self.renaming_workspace.take().is_some();
        let had_text = !self.workspace_rename_input.text().is_empty();
        let was_selecting = self.inline_input_selecting;
        self.workspace_rename_input.clear();
        self.inline_input_selecting = false;
        let changed = was_renaming || had_text || was_selecting;
        if changed {
            self.reset_cursor_blink_phase();
        }
        changed
    }

    pub(crate) fn begin_rename_workspace(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.workspaces.len() {
            return;
        }

        if self.is_command_palette_open() {
            self.close_command_palette(cx);
        }
        if self.search_open {
            self.close_search(cx);
        }

        self.reset_tab_rename_state();
        self.finish_workspace_drag();
        self.renaming_workspace = Some(index);
        self.workspace_rename_input
            .set_text(self.workspaces[index].name.clone());
        self.reset_cursor_blink_phase();
        self.inline_input_selecting = false;
        cx.notify();
    }

    pub(crate) fn commit_rename_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.renaming_workspace else {
            return;
        };
        let trimmed = self.workspace_rename_input.text().trim();
        if let Some(entry) = self.workspaces.get_mut(index)
            && !trimmed.is_empty()
        {
            entry.name = Self::truncate_tab_title(trimmed);
            self.schedule_persist_native_workspace();
        }

        self.reset_workspace_rename_state();
        cx.notify();
    }

    pub(crate) fn cancel_rename_workspace(&mut self, cx: &mut Context<Self>) {
        if self.renaming_workspace.is_none() {
            return;
        }

        self.reset_workspace_rename_state();
        cx.notify();
    }

    pub(crate) fn add_workspace(&mut self, cx: &mut Context<Self>) {
        if self.runtime_kind() != RuntimeKind::Native {
            termy_toast::info("Workspaces are not available with the tmux runtime");
            self.notify_overlay(cx);
            return;
        }

        let previous_workspace = self.active_workspace;
        let id = self.next_workspace_id;
        self.stash_active_workspace_tabs();
        self.workspaces.push(WorkspaceEntry::new(id));
        self.active_workspace = self.workspaces.len() - 1;
        self.active_tab = 0;
        self.add_tab(cx);
        if self.tabs.is_empty() {
            // Tab creation failed; roll back so no empty workspace survives.
            self.workspaces.pop();
            self.restore_workspace_tabs(previous_workspace);
            return;
        }
        self.next_workspace_id += 1;
        self.reset_view_state_after_workspace_change(cx);
    }

    /// Close the active workspace (dropping its tabs) and activate a
    /// neighbor. Used when the last tab of a workspace is closed or exits.
    pub(crate) fn close_active_workspace(&mut self, cx: &mut Context<Self>) {
        if !self.has_other_workspaces() {
            return;
        }

        let removed_pane_ids = self
            .tabs
            .iter()
            .flat_map(|tab| tab.panes.iter().map(|pane| pane.id.clone()))
            .collect::<Vec<_>>();
        let _ = self.release_forwarded_mouse_presses_for_panes(&removed_pane_ids);
        let removed_tab_ids = self.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>();
        for tab_id in removed_tab_ids {
            self.native_pane_zoom_snapshots.remove(&tab_id);
            self.native_pane_layout_trees.remove(&tab_id);
        }

        let removed_index = self.active_workspace;
        self.tabs.clear();
        self.workspaces.remove(removed_index);
        let target = removed_index.min(self.workspaces.len() - 1);
        self.restore_workspace_tabs(target);
        self.set_tab_scrollback_options(self.active_tab, true);
        if self.tabs.is_empty() {
            self.add_tab(cx);
        }
        self.reset_view_state_after_workspace_change(cx);
    }

    /// Merge every stashed workspace's tabs into the visible strip and keep a
    /// single workspace. Runs when the sidebar setting is turned off so no
    /// tab becomes unreachable.
    pub(crate) fn collapse_workspaces_into_active(&mut self) {
        if !self.has_other_workspaces() {
            return;
        }

        let mut merged: Vec<TerminalTab> = Vec::new();
        let mut active_tab = self.active_tab;
        let mut kept_entry: Option<WorkspaceEntry> = None;
        for (index, mut entry) in std::mem::take(&mut self.workspaces).into_iter().enumerate() {
            if index == self.active_workspace {
                active_tab = merged.len() + self.active_tab;
                merged.append(&mut self.tabs);
                kept_entry = Some(entry);
            } else {
                merged.append(&mut entry.tabs);
            }
        }

        self.active_tab = active_tab.min(merged.len().saturating_sub(1));
        self.tabs = merged;
        let mut entry = kept_entry.unwrap_or_else(|| WorkspaceEntry::new(1));
        entry.tabs = Vec::new();
        self.workspaces = vec![entry];
        self.active_workspace = 0;
        self.mark_tab_strip_layout_dirty();
        self.sync_tab_strip_for_active_tab();
        self.schedule_persist_native_workspace();
    }

    /// Drain PTY events for tabs stashed in inactive workspaces so their
    /// processes never stall behind a full event channel. Only a minimal
    /// subset is applied (progress, cwd, exit); titles refresh on the next
    /// prompt after the workspace is reactivated. Returns whether more
    /// events remain queued.
    pub(crate) fn drain_stashed_workspace_terminal_events(
        &mut self,
        reply_host: &mut impl TerminalReplyHost,
    ) -> bool {
        let mut events_remain = false;
        let mut exited_panes: Vec<(u64, TabId, String)> = Vec::new();

        for entry in &mut self.workspaces {
            for tab in &mut entry.tabs {
                for pane in &mut tab.panes {
                    let Some(terminal) = pane.maybe_terminal() else {
                        continue;
                    };
                    let (events, has_more) = terminal.drain_events(reply_host);
                    if has_more {
                        events_remain = true;
                    }
                    for event in events {
                        match event {
                            TerminalEvent::Exit => {
                                exited_panes.push((entry.id, tab.id, pane.id.clone()));
                            }
                            TerminalEvent::Progress(state) => {
                                pane.progress_state = state;
                            }
                            TerminalEvent::WorkingDirectory(path) => {
                                tab.last_prompt_cwd = Some(path);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        for (workspace_id, tab_id, pane_id) in exited_panes {
            self.remove_stashed_pane(workspace_id, tab_id, pane_id.as_str());
        }

        events_remain
    }

    fn remove_stashed_pane(&mut self, workspace_id: u64, tab_id: TabId, pane_id: &str) {
        let Some(workspace_index) = self
            .workspaces
            .iter()
            .position(|entry| entry.id == workspace_id)
        else {
            return;
        };
        if workspace_index == self.active_workspace {
            return;
        }

        let entry = &mut self.workspaces[workspace_index];
        let Some(tab_index) = entry.tabs.iter().position(|tab| tab.id == tab_id) else {
            return;
        };
        let tab = &mut entry.tabs[tab_index];
        let Some(pane_index) = tab.panes.iter().position(|pane| pane.id == pane_id) else {
            return;
        };

        if tab.panes.len() > 1 {
            let removed = tab.panes.remove(pane_index);
            Self::native_close_expand_neighbors(&mut tab.panes, &removed);
            if tab.active_pane_id == removed.id || !tab.has_active_pane() {
                let next_index = pane_index.min(tab.panes.len().saturating_sub(1));
                if let Some(next) = tab.panes.get(next_index) {
                    tab.active_pane_id = next.id.clone();
                }
            }
            // The stored layout tree still references the removed leaf; drop
            // it so it is rebuilt from pane geometry on next use.
            self.native_pane_layout_trees.remove(&tab_id);
            self.native_pane_zoom_snapshots.remove(&tab_id);
            return;
        }

        entry.tabs.remove(tab_index);
        if entry.active_tab >= entry.tabs.len() {
            entry.active_tab = entry.tabs.len().saturating_sub(1);
        }
        let entry_is_empty = entry.tabs.is_empty();
        self.native_pane_layout_trees.remove(&tab_id);
        self.native_pane_zoom_snapshots.remove(&tab_id);
        if entry_is_empty {
            self.workspaces.remove(workspace_index);
            if workspace_index < self.active_workspace {
                self.active_workspace -= 1;
            }
        }
        self.schedule_persist_native_workspace();
    }

    /// All tabs held by inactive workspaces, in sidebar order.
    pub(super) fn stashed_workspace_tabs(&self) -> impl Iterator<Item = &TerminalTab> {
        self.workspaces.iter().flat_map(|entry| entry.tabs.iter())
    }

    /// Stashed tabs that are busy (running process or fullscreen app), for
    /// quit warnings: the visible strip is checked separately.
    pub(crate) fn stashed_busy_workspace_tab_titles(&self) -> Vec<String> {
        let fallback_title = self.fallback_title();
        self.stashed_workspace_tabs()
            .filter(|tab| Self::tab_is_busy(tab))
            .map(|tab| {
                let title = tab.title.trim();
                if title.is_empty() {
                    fallback_title.to_string()
                } else {
                    title.to_string()
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_sidebar_width_clamps_to_configured_bounds() {
        assert_eq!(
            TerminalView::clamp_workspace_sidebar_width(1.0),
            termy_config_core::MIN_SIDEBAR_WIDTH
        );
        assert_eq!(TerminalView::clamp_workspace_sidebar_width(240.0), 240.0);
        assert_eq!(
            TerminalView::clamp_workspace_sidebar_width(10_000.0),
            termy_config_core::MAX_SIDEBAR_WIDTH
        );
        assert_eq!(
            TerminalView::clamp_workspace_sidebar_width(f32::NAN),
            termy_config_core::DEFAULT_SIDEBAR_WIDTH
        );
    }
}
