use super::*;

impl TerminalView {
    pub(super) fn truncate_tab_title(title: &str) -> String {
        // Keep titles single-line so shell-provided newlines do not break tab layout.
        let normalized = title.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.chars().count() > MAX_TAB_TITLE_CHARS {
            return normalized.chars().take(MAX_TAB_TITLE_CHARS).collect();
        }
        normalized
    }

    pub(super) fn tab_title_char_budget(display_width: f32, close_slot_width: f32) -> usize {
        let hidden_close_guard = if close_slot_width <= f32::EPSILON {
            TAB_TITLE_BUDGET_HIDDEN_CLOSE_GUARD_PX
        } else {
            0.0
        };
        let text_area = (display_width
            - (TAB_TEXT_PADDING_X * 2.0)
            - close_slot_width
            - TAB_TITLE_BUDGET_CLOSE_GUARD_PX
            - hidden_close_guard)
            .max(0.0);
        (text_area / TAB_TITLE_CHAR_WIDTH).floor() as usize
    }

    pub(super) fn format_tab_label_for_render(title: &str, max_chars: usize) -> String {
        let char_count = title.chars().count();
        if char_count <= max_chars {
            return title.to_string();
        }

        if max_chars <= 3 {
            return ".".repeat(max_chars);
        }

        let is_path_like = title.contains('/') || title.contains('\\');
        if !is_path_like {
            // Non-path titles keep end-truncation behavior through render-level text ellipsis.
            return title.to_string();
        }

        let chars: Vec<char> = title.chars().collect();
        let basename_len = chars
            .iter()
            .rposition(|ch| *ch == '/' || *ch == '\\')
            .map_or(chars.len(), |index| chars.len().saturating_sub(index + 1));
        let min_tail = (max_chars - 3) / 2;
        let preferred_tail = (basename_len + 1).min(max_chars - 4);
        let tail_chars = min_tail.max(preferred_tail);
        let head_chars = max_chars - 3 - tail_chars;

        let mut formatted = String::with_capacity(max_chars);
        for ch in chars.iter().take(head_chars) {
            formatted.push(*ch);
        }
        formatted.push_str("...");
        for ch in chars
            .iter()
            .skip(chars.len().saturating_sub(tail_chars))
            .take(tail_chars)
        {
            formatted.push(*ch);
        }

        formatted
    }

    pub(super) fn fallback_title(&self) -> &str {
        let fallback = self.tab_title.fallback.trim();
        if fallback.is_empty() {
            DEFAULT_TAB_TITLE
        } else {
            fallback
        }
    }

    pub(super) fn resolve_template(
        template: &str,
        cwd: Option<&str>,
        command: Option<&str>,
    ) -> String {
        template
            .replace("{cwd}", cwd.unwrap_or(""))
            .replace("{command}", command.unwrap_or(""))
    }

    pub(super) fn should_seed_predicted_prompt_title(tab_title: &TabTitleConfig) -> bool {
        tab_title
            .priority
            .iter()
            .any(|source| *source == TabTitleSource::Explicit)
    }

    pub(super) fn predicted_prompt_seed_title(
        tab_title: &TabTitleConfig,
        cwd: Option<&str>,
    ) -> Option<String> {
        if !Self::should_seed_predicted_prompt_title(tab_title) {
            return None;
        }

        let resolved = Self::resolve_template(&tab_title.prompt_format, cwd, None);
        let resolved = resolved.trim();
        if resolved.is_empty() {
            return None;
        }

        Some(Self::truncate_tab_title(resolved))
    }

    pub(super) fn parse_explicit_title(&self, title: &str) -> Option<ExplicitTitlePayload> {
        let prefix = self.tab_title.explicit_prefix.trim();
        if prefix.is_empty() {
            return None;
        }

        let payload = title.strip_prefix(prefix)?.trim();
        if payload.is_empty() {
            return None;
        }

        if let Some(prompt) = payload.strip_prefix("prompt:") {
            let prompt = prompt.trim();
            if prompt.is_empty() {
                return None;
            }
            return Some(ExplicitTitlePayload::Prompt(Self::resolve_template(
                &self.tab_title.prompt_format,
                Some(prompt),
                None,
            )));
        }

        if let Some(command) = payload.strip_prefix("command:") {
            let command = command.trim();
            if command.is_empty() {
                return None;
            }
            return Some(ExplicitTitlePayload::Command(Self::resolve_template(
                &self.tab_title.command_format,
                None,
                Some(command),
            )));
        }

        let explicit = payload.strip_prefix("title:").unwrap_or(payload).trim();
        if explicit.is_empty() {
            return None;
        }

        Some(ExplicitTitlePayload::Title(explicit.to_string()))
    }

    pub(super) fn resolved_tab_title(&self, index: usize) -> String {
        let tab = &self.tabs[index];

        for source in &self.tab_title.priority {
            let candidate = match source {
                TabTitleSource::Manual => tab.manual_title.as_deref(),
                TabTitleSource::Explicit => tab.explicit_title.as_deref(),
                TabTitleSource::Shell => tab.shell_title.as_deref(),
                TabTitleSource::Fallback => Some(self.fallback_title()),
            };

            if let Some(candidate) = candidate.map(str::trim).filter(|value| !value.is_empty()) {
                return Self::truncate_tab_title(candidate);
            }
        }

        Self::truncate_tab_title(self.fallback_title())
    }

    pub(super) fn refresh_tab_title(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        let next = self.resolved_tab_title(index);
        if self.tabs[index].title == next {
            return false;
        }

        self.tabs[index].title = next;
        self.tabs[index].display_width = Self::tab_display_width_for_title(&self.tabs[index].title);
        true
    }

    pub(super) fn cancel_pending_command_title(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }

        let tab = &mut self.tabs[index];
        tab.pending_command_token = tab.pending_command_token.wrapping_add(1);
        tab.pending_command_title = None;
    }

    pub(super) fn set_explicit_title(&mut self, index: usize, explicit_title: String) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        let explicit_title = Self::truncate_tab_title(&explicit_title);
        if self.tabs[index].explicit_title.as_deref() == Some(explicit_title.as_str()) {
            return false;
        }

        self.tabs[index].explicit_title = Some(explicit_title);
        self.refresh_tab_title(index)
    }

    pub(super) fn schedule_delayed_command_title(
        &mut self,
        index: usize,
        command_title: String,
        delay_ms: u64,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }

        let tab = &mut self.tabs[index];
        tab.pending_command_token = tab.pending_command_token.wrapping_add(1);
        tab.pending_command_title = Some(Self::truncate_tab_title(&command_title));
        let token = tab.pending_command_token;

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            smol::Timer::after(Duration::from_millis(delay_ms)).await;
            let _ = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    if view.activate_pending_command_title(index, token) {
                        cx.notify();
                    }
                })
            });
        })
        .detach();
    }

    pub(super) fn activate_pending_command_title(&mut self, index: usize, token: u64) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        let tab = &mut self.tabs[index];
        if tab.pending_command_token != token {
            return false;
        }

        let Some(command_title) = tab.pending_command_title.take() else {
            return false;
        };

        if tab.explicit_title.as_deref() == Some(command_title.as_str()) {
            return false;
        }

        tab.explicit_title = Some(command_title);
        self.refresh_tab_title(index)
    }

    pub(super) fn apply_terminal_title(
        &mut self,
        index: usize,
        title: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let title = title.trim();
        if title.is_empty() || index >= self.tabs.len() {
            return false;
        }

        if let Some(explicit_payload) = self.parse_explicit_title(title) {
            return match explicit_payload {
                ExplicitTitlePayload::Prompt(prompt_title) => {
                    self.tabs[index].running_process = false;
                    self.cancel_pending_command_title(index);
                    self.set_explicit_title(index, prompt_title)
                }
                ExplicitTitlePayload::Title(prompt_title) => {
                    self.cancel_pending_command_title(index);
                    self.set_explicit_title(index, prompt_title)
                }
                ExplicitTitlePayload::Command(command_title) => {
                    self.tabs[index].running_process = true;
                    self.schedule_delayed_command_title(
                        index,
                        command_title,
                        COMMAND_TITLE_DELAY_MS,
                        cx,
                    );
                    false
                }
            };
        }

        let shell_title = Self::truncate_tab_title(title);
        if self.tabs[index].shell_title.as_deref() == Some(shell_title.as_str()) {
            return false;
        }

        self.tabs[index].shell_title = Some(shell_title);
        self.refresh_tab_title(index)
    }

    pub(super) fn clear_terminal_titles(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        self.cancel_pending_command_title(index);
        let tab = &mut self.tabs[index];
        tab.running_process = false;
        let had_shell = tab.shell_title.take().is_some();
        let had_explicit = tab.explicit_title.take().is_some();
        if !had_shell && !had_explicit {
            return false;
        }

        self.refresh_tab_title(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{TabTitleConfig, TabTitleSource};

    #[test]
    fn predicted_prompt_seed_title_uses_cwd_template_when_explicit_is_enabled() {
        let config = TabTitleConfig::default();
        let title = TerminalView::predicted_prompt_seed_title(&config, Some("~/projects/termy"));
        assert_eq!(title.as_deref(), Some("~/projects/termy"));
    }

    #[test]
    fn predicted_prompt_seed_title_skips_static_only_priority() {
        let mut config = TabTitleConfig::default();
        config.priority = vec![TabTitleSource::Manual, TabTitleSource::Fallback];

        let title = TerminalView::predicted_prompt_seed_title(&config, Some("~/projects/termy"));
        assert!(title.is_none());
    }

    #[test]
    fn predicted_prompt_seed_title_ignores_empty_resolved_output() {
        let mut config = TabTitleConfig::default();
        config.prompt_format = "{cwd}".to_string();

        let title = TerminalView::predicted_prompt_seed_title(&config, None);
        assert!(title.is_none());
    }

    #[test]
    fn tab_title_char_budget_accounts_for_padding_and_close_slot() {
        let budget = TerminalView::tab_title_char_budget(TAB_MIN_WIDTH, TAB_CLOSE_SLOT_WIDTH);
        assert_eq!(budget, 5);
    }

    #[test]
    fn tab_title_char_budget_reclaims_space_when_close_slot_is_hidden() {
        let budget_hidden = TerminalView::tab_title_char_budget(TAB_MIN_WIDTH, 0.0);
        let budget_with_close =
            TerminalView::tab_title_char_budget(TAB_MIN_WIDTH, TAB_CLOSE_SLOT_WIDTH);
        assert!(budget_hidden > budget_with_close);
    }

    #[test]
    fn tab_title_char_budget_applies_edge_guard_without_close_slot() {
        let budget_hidden = TerminalView::tab_title_char_budget(TAB_MIN_WIDTH, 0.0);
        assert_eq!(budget_hidden, 8);
    }

    #[test]
    fn format_tab_label_for_render_middle_squeezes_path_titles() {
        let title = "~/Desktop/claudeCode/claude-code-provider-proxy/docs";
        let formatted = TerminalView::format_tab_label_for_render(title, 24);

        assert_eq!(formatted.chars().count(), 24);
        assert!(formatted.contains("..."));
        assert!(formatted.starts_with("~/"));
        assert!(formatted.ends_with("/docs"));
    }

    #[test]
    fn format_tab_label_for_render_returns_dots_for_tiny_budgets() {
        let title = "~/Desktop/claudeCode/claude-code-provider-proxy/docs";
        assert_eq!(TerminalView::format_tab_label_for_render(title, 3), "...");
        assert_eq!(TerminalView::format_tab_label_for_render(title, 2), "..");
        assert_eq!(TerminalView::format_tab_label_for_render(title, 0), "");
    }

    #[test]
    fn format_tab_label_for_render_leaves_non_path_titles_for_end_truncation() {
        let title = "cargo test --workspace --all-features";
        assert_eq!(
            TerminalView::format_tab_label_for_render(title, 8),
            "cargo test --workspace --all-features"
        );
    }
}
