use super::*;

impl TerminalView {
    pub(super) fn tab_bar_layout(&self, viewport_width: f32) -> TabBarLayout {
        let tab_count = self.tabs.len().max(1) as f32;
        let total_gap = (self.tabs.len().saturating_sub(1) as f32) * TAB_PILL_GAP;
        let available =
            (viewport_width - (TAB_HORIZONTAL_PADDING * 2.0) - total_gap).max(tab_count);
        let tab_pill_width = (available / tab_count).max(1.0);

        TabBarLayout {
            tab_pill_width,
            tab_padding_x: Self::tab_pill_padding_x(tab_pill_width),
            slot_width: tab_pill_width + TAB_PILL_GAP,
        }
    }

    pub(super) fn tab_pill_padding_x(tab_pill_width: f32) -> f32 {
        if tab_pill_width >= TAB_PILL_COMPACT_THRESHOLD {
            TAB_PILL_NORMAL_PADDING
        } else {
            TAB_PILL_COMPACT_PADDING
        }
    }

    pub(super) fn tab_shows_close(
        tab_pill_width: f32,
        is_active: bool,
        tab_padding_x: f32,
    ) -> bool {
        // Keep close affordance visible on the active tab whenever there is
        // enough room for the hit target and side padding.
        let min_hit_target_width = TAB_CLOSE_HITBOX + (tab_padding_x * 2.0);
        if tab_pill_width < min_hit_target_width {
            return false;
        }

        if is_active {
            return true;
        }

        tab_pill_width >= TAB_INACTIVE_CLOSE_MIN_WIDTH
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

        self.tabs.push(TerminalTab::new(terminal));
        self.active_tab = self.tabs.len() - 1;
        self.refresh_tab_title(self.active_tab);
        self.renaming_tab = None;
        self.rename_buffer.clear();
        self.clear_selection();
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
                self.rename_buffer.clear();
            }
            Some(editing) if editing > index => {
                self.renaming_tab = Some(editing - 1);
            }
            _ => {}
        }

        self.clear_selection();
        cx.notify();
    }

    pub(super) fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        self.close_tab(self.active_tab, cx);
    }

    pub(super) fn switch_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() || index == self.active_tab {
            return;
        }

        self.active_tab = index;
        self.renaming_tab = None;
        self.rename_buffer.clear();
        self.clear_selection();
        cx.notify();
    }

    pub(super) fn commit_rename_tab(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.renaming_tab else {
            return;
        };

        let trimmed = self.rename_buffer.trim();
        self.tabs[index].manual_title = (!trimmed.is_empty())
            .then(|| Self::truncate_tab_title(trimmed))
            .filter(|title| !title.is_empty());
        self.refresh_tab_title(index);

        self.renaming_tab = None;
        self.rename_buffer.clear();
        cx.notify();
    }

    pub(super) fn cancel_rename_tab(&mut self, cx: &mut Context<Self>) {
        if self.renaming_tab.is_none() {
            return;
        }

        self.renaming_tab = None;
        self.rename_buffer.clear();
        cx.notify();
    }
}
