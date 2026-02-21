use super::*;
use gpui::{point, uniform_list};
use std::ops::Range;

impl TerminalView {
    fn reset_command_palette_state(&mut self) {
        self.command_palette_input.clear();
        self.command_palette_selected = 0;
        self.command_palette_scroll_handle = UniformListScrollHandle::new();
        self.command_palette_scroll_target_y = None;
        self.command_palette_scroll_max_y = 0.0;
        self.command_palette_scroll_animating = false;
        self.inline_input_selecting = false;
    }

    fn format_keybinding_label(binding: &gpui::KeyBinding) -> String {
        binding
            .keystrokes()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn command_palette_binding_badge<A: gpui::Action>(
        &self,
        action: &A,
        window: &Window,
    ) -> Option<String> {
        if let Some(binding) =
            window.highest_precedence_binding_for_action_in(action, &self.focus_handle)
        {
            Some(Self::format_keybinding_label(&binding))
        } else {
            None
        }
    }

    fn command_palette_command_shortcut(
        &self,
        action: CommandAction,
        window: &Window,
    ) -> Option<String> {
        match action {
            CommandAction::Quit => self.command_palette_binding_badge(&commands::Quit, window),
            CommandAction::OpenConfig => {
                self.command_palette_binding_badge(&commands::OpenConfig, window)
            }
            CommandAction::AppInfo => {
                self.command_palette_binding_badge(&commands::AppInfo, window)
            }
            CommandAction::RestartApp => {
                self.command_palette_binding_badge(&commands::RestartApp, window)
            }
            CommandAction::RenameTab => {
                self.command_palette_binding_badge(&commands::RenameTab, window)
            }
            CommandAction::CheckForUpdates => {
                self.command_palette_binding_badge(&commands::CheckForUpdates, window)
            }
            CommandAction::ToggleCommandPalette => {
                self.command_palette_binding_badge(&commands::ToggleCommandPalette, window)
            }
            CommandAction::NewTab => self.command_palette_binding_badge(&commands::NewTab, window),
            CommandAction::CloseTab => {
                self.command_palette_binding_badge(&commands::CloseTab, window)
            }
            CommandAction::Copy => self.command_palette_binding_badge(&commands::Copy, window),
            CommandAction::Paste => self.command_palette_binding_badge(&commands::Paste, window),
            CommandAction::ZoomIn => self.command_palette_binding_badge(&commands::ZoomIn, window),
            CommandAction::ZoomOut => {
                self.command_palette_binding_badge(&commands::ZoomOut, window)
            }
            CommandAction::ZoomReset => {
                self.command_palette_binding_badge(&commands::ZoomReset, window)
            }
        }
    }

    fn command_palette_shortcut(&self, action: CommandAction, window: &Window) -> Option<String> {
        if !self.command_palette_show_keybinds {
            return None;
        }

        self.command_palette_command_shortcut(action, window)
    }

    pub(super) fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = true;
        self.reset_command_palette_state();
        self.reset_cursor_blink_phase();

        cx.notify();
    }

    pub(super) fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        if !self.command_palette_open {
            return;
        }

        self.command_palette_open = false;
        self.reset_command_palette_state();
        cx.notify();
    }

    fn command_palette_items(&self) -> Vec<CommandPaletteItem> {
        let mut items = vec![
            CommandPaletteItem {
                title: "App Info",
                keywords: "information version about build",
                action: CommandAction::AppInfo,
            },
            CommandPaletteItem {
                title: "Restart App",
                keywords: "relaunch reopen restart",
                action: CommandAction::RestartApp,
            },
            CommandPaletteItem {
                title: "Open Config",
                keywords: "settings preferences",
                action: CommandAction::OpenConfig,
            },
            CommandPaletteItem {
                title: "Zoom In",
                keywords: "font increase",
                action: CommandAction::ZoomIn,
            },
            CommandPaletteItem {
                title: "Zoom Out",
                keywords: "font decrease",
                action: CommandAction::ZoomOut,
            },
            CommandPaletteItem {
                title: "Reset Zoom",
                keywords: "font default",
                action: CommandAction::ZoomReset,
            },
        ];

        if self.use_tabs {
            items.insert(
                0,
                CommandPaletteItem {
                    title: "Rename Tab",
                    keywords: "title name",
                    action: CommandAction::RenameTab,
                },
            );
            items.insert(
                0,
                CommandPaletteItem {
                    title: "Close Tab",
                    keywords: "remove tab",
                    action: CommandAction::CloseTab,
                },
            );
            items.insert(
                0,
                CommandPaletteItem {
                    title: "New Tab",
                    keywords: "create tab",
                    action: CommandAction::NewTab,
                },
            );
        }

        #[cfg(target_os = "macos")]
        items.push(CommandPaletteItem {
            title: "Check for Updates",
            keywords: "release version updater",
            action: CommandAction::CheckForUpdates,
        });

        items
    }

    pub(super) fn filtered_command_palette_items(&self) -> Vec<CommandPaletteItem> {
        let query = self.command_palette_query().trim().to_ascii_lowercase();
        let query_terms: Vec<&str> = query
            .split_whitespace()
            .filter(|term| !term.is_empty())
            .collect();

        self.command_palette_items()
            .into_iter()
            .filter(|item| Self::command_palette_item_matches_terms(item, &query_terms))
            .collect()
    }

    fn command_palette_item_matches_terms(item: &CommandPaletteItem, query_terms: &[&str]) -> bool {
        if query_terms.is_empty() {
            return true;
        }

        let searchable = format!("{} {}", item.title, item.keywords).to_ascii_lowercase();
        let words: Vec<&str> = searchable
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect();

        query_terms
            .iter()
            .all(|term| words.iter().any(|word| word.starts_with(term)))
    }

    pub(super) fn clamp_command_palette_selection(&mut self, len: usize) {
        if len == 0 {
            self.command_palette_selected = 0;
        } else if self.command_palette_selected >= len {
            self.command_palette_selected = len - 1;
        }
    }

    fn command_palette_viewport_height() -> f32 {
        COMMAND_PALETTE_MAX_ITEMS as f32 * COMMAND_PALETTE_ROW_HEIGHT
    }

    fn command_palette_max_scroll_for_count(item_count: usize) -> f32 {
        (item_count as f32 * COMMAND_PALETTE_ROW_HEIGHT - Self::command_palette_viewport_height())
            .max(0.0)
    }

    pub(super) fn animate_command_palette_to_selected(
        &mut self,
        item_count: usize,
        cx: &mut Context<Self>,
    ) {
        if item_count == 0 {
            self.command_palette_scroll_target_y = None;
            self.command_palette_scroll_max_y = 0.0;
            self.command_palette_scroll_animating = false;
            return;
        }

        let viewport_height = Self::command_palette_viewport_height();
        let max_scroll = Self::command_palette_max_scroll_for_count(item_count);
        self.command_palette_scroll_max_y = max_scroll;

        let scroll_handle = self
            .command_palette_scroll_handle
            .0
            .borrow()
            .base_handle
            .clone();
        let offset = scroll_handle.offset();
        let current_y = -Into::<f32>::into(offset.y);

        let row_top = self.command_palette_selected as f32 * COMMAND_PALETTE_ROW_HEIGHT;
        let row_bottom = row_top + COMMAND_PALETTE_ROW_HEIGHT;
        let mut target_y = current_y;
        if row_top < current_y {
            target_y = row_top;
        } else if row_bottom > current_y + viewport_height {
            target_y = row_bottom - viewport_height;
        }
        target_y = target_y.clamp(0.0, max_scroll);

        if (target_y - current_y).abs() <= f32::EPSILON {
            self.command_palette_scroll_target_y = None;
            self.command_palette_scroll_animating = false;
            return;
        }

        self.command_palette_scroll_target_y = Some(target_y);
        self.start_command_palette_scroll_animation(cx);
    }

    fn start_command_palette_scroll_animation(&mut self, cx: &mut Context<Self>) {
        if self.command_palette_scroll_animating {
            return;
        }
        self.command_palette_scroll_animating = true;

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                smol::Timer::after(Duration::from_millis(16)).await;
                let keep_animating = match cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        let changed = view.tick_command_palette_scroll_animation();
                        if changed {
                            cx.notify();
                        }
                        view.command_palette_scroll_animating
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

    fn tick_command_palette_scroll_animation(&mut self) -> bool {
        if !self.command_palette_open {
            self.command_palette_scroll_target_y = None;
            self.command_palette_scroll_animating = false;
            return false;
        }

        let Some(target_y) = self.command_palette_scroll_target_y else {
            self.command_palette_scroll_animating = false;
            return false;
        };

        let scroll_handle = self
            .command_palette_scroll_handle
            .0
            .borrow()
            .base_handle
            .clone();
        let offset = scroll_handle.offset();
        let current_y = -Into::<f32>::into(offset.y);
        let max_offset_from_handle: f32 = scroll_handle.max_offset().height.into();
        let max_scroll = max_offset_from_handle
            .max(self.command_palette_scroll_max_y)
            .max(0.0);
        let target_y = target_y.clamp(0.0, max_scroll);
        let delta = target_y - current_y;

        if delta.abs() <= 0.5 {
            if delta.abs() > f32::EPSILON {
                scroll_handle.set_offset(point(offset.x, px(-target_y)));
            }
            self.command_palette_scroll_target_y = None;
            self.command_palette_scroll_animating = false;
            return true;
        }

        let eased_step = (delta * 0.32).clamp(-18.0, 18.0);
        let min_step = if delta.is_sign_positive() { 0.6 } else { -0.6 };
        let next_y = if eased_step.abs() < 0.6 {
            current_y + min_step
        } else {
            current_y + eased_step
        }
        .clamp(0.0, max_scroll);

        scroll_handle.set_offset(point(offset.x, px(-next_y)));
        true
    }

    pub(super) fn handle_command_palette_key_down(&mut self, key: &str, cx: &mut Context<Self>) {
        match key {
            "escape" => {
                self.close_command_palette(cx);
                return;
            }
            "enter" => {
                self.execute_command_palette_selection(cx);
                return;
            }
            "up" => {
                let len = self.filtered_command_palette_items().len();
                if len > 0 && self.command_palette_selected > 0 {
                    self.command_palette_selected -= 1;
                    self.animate_command_palette_to_selected(len, cx);
                    cx.notify();
                }
                return;
            }
            "down" => {
                let len = self.filtered_command_palette_items().len();
                if len > 0 && self.command_palette_selected + 1 < len {
                    self.command_palette_selected += 1;
                    self.animate_command_palette_to_selected(len, cx);
                    cx.notify();
                }
                return;
            }
            _ => {}
        }
    }

    fn execute_command_palette_selection(&mut self, cx: &mut Context<Self>) {
        let items = self.filtered_command_palette_items();
        if items.is_empty() {
            return;
        }

        let index = self.command_palette_selected.min(items.len() - 1);
        let action = items[index].action;

        self.execute_command_palette_action(action, cx);
    }

    fn execute_command_palette_action(&mut self, action: CommandAction, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.reset_command_palette_state();

        self.execute_command_action(action, false, cx);

        match action {
            CommandAction::OpenConfig => {
                termy_toast::info("Opened config file");
                cx.notify();
            }
            CommandAction::NewTab => termy_toast::success("Opened new tab"),
            CommandAction::CloseTab => termy_toast::info("Closed active tab"),
            CommandAction::ZoomIn => termy_toast::info("Zoomed in"),
            CommandAction::ZoomOut => termy_toast::info("Zoomed out"),
            CommandAction::ZoomReset => termy_toast::info("Zoom reset"),
            CommandAction::Quit
            | CommandAction::AppInfo
            | CommandAction::RestartApp
            | CommandAction::RenameTab
            | CommandAction::CheckForUpdates
            | CommandAction::ToggleCommandPalette
            | CommandAction::Copy
            | CommandAction::Paste => {}
        }
    }

    fn command_palette_scrollbar_metrics(
        &self,
        viewport_height: f32,
        item_count: usize,
    ) -> Option<(f32, f32)> {
        let scroll_handle = self
            .command_palette_scroll_handle
            .0
            .borrow()
            .base_handle
            .clone();
        let max_offset_from_handle: f32 = scroll_handle.max_offset().height.into();
        let estimated_content_height = item_count as f32 * COMMAND_PALETTE_ROW_HEIGHT;
        let estimated_max_offset = (estimated_content_height - viewport_height).max(0.0);
        let max_offset = max_offset_from_handle.max(estimated_max_offset);
        if max_offset <= f32::EPSILON {
            return None;
        }

        let offset: f32 = scroll_handle.offset().y.into();
        let progress = (-offset / max_offset).clamp(0.0, 1.0);
        let content_height = viewport_height + max_offset;
        let thumb_height = ((viewport_height / content_height) * viewport_height)
            .clamp(COMMAND_PALETTE_SCROLLBAR_MIN_THUMB_HEIGHT, viewport_height);
        let travel = (viewport_height - thumb_height).max(0.0);

        Some((travel * progress, thumb_height))
    }

    fn render_command_palette_rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let items = self.filtered_command_palette_items();
        let selected = if items.is_empty() {
            0
        } else {
            self.command_palette_selected.min(items.len() - 1)
        };

        let mut selected_bg = self.colors.cursor;
        selected_bg.a = 0.2;
        let mut selected_border = self.colors.cursor;
        selected_border.a = 0.35;
        let mut transparent = self.colors.background;
        transparent.a = 0.0;

        let mut primary_text = self.colors.foreground;
        primary_text.a = 0.95;
        let mut shortcut_bg = self.colors.cursor;
        shortcut_bg.a = 0.1;
        let mut shortcut_border = self.colors.cursor;
        shortcut_border.a = 0.22;
        let mut shortcut_text = self.colors.foreground;
        shortcut_text.a = 0.8;

        let mut rows = Vec::with_capacity(range.len());
        for index in range {
            let Some(item) = items.get(index).copied() else {
                continue;
            };

            let is_selected = index == selected;
            let action = item.action;
            let shortcut = self.command_palette_shortcut(action, window);

            rows.push(
                div()
                    .id(("command-palette-item", index))
                    .w_full()
                    .h(px(COMMAND_PALETTE_ROW_HEIGHT))
                    .px(px(10.0))
                    .rounded_sm()
                    .bg(if is_selected {
                        selected_bg
                    } else {
                        transparent
                    })
                    .border_1()
                    .border_color(if is_selected {
                        selected_border
                    } else {
                        transparent
                    })
                    .cursor_pointer()
                    .on_mouse_move(cx.listener(move |this, _event, _window, cx| {
                        if this.command_palette_selected != index {
                            this.command_palette_selected = index;
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event, _window, cx| {
                            this.command_palette_selected = index;
                            this.execute_command_palette_action(action, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .text_size(px(12.0))
                    .text_color(primary_text)
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.0))
                            .child(div().flex_1().truncate().child(item.title))
                            .children(shortcut.map(|label| {
                                div()
                                    .flex_none()
                                    .h(px(20.0))
                                    .px(px(6.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .bg(shortcut_bg)
                                    .border_1()
                                    .border_color(shortcut_border)
                                    .text_size(px(10.0))
                                    .text_color(shortcut_text)
                                    .child(label)
                            })),
                    )
                    .into_any_element(),
            );
        }
        rows
    }

    pub(super) fn render_command_palette_modal(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let items = self.filtered_command_palette_items();
        let list_height = COMMAND_PALETTE_MAX_ITEMS as f32 * COMMAND_PALETTE_ROW_HEIGHT;

        let mut overlay_bg = self.colors.background;
        overlay_bg.a = 0.78;
        let mut panel_bg = self.colors.background;
        panel_bg.a = 0.98;
        let mut panel_border = self.colors.cursor;
        panel_border.a = 0.24;
        let mut primary_text = self.colors.foreground;
        primary_text.a = 0.95;
        let mut muted_text = self.colors.foreground;
        muted_text.a = 0.62;
        let mut transparent = self.colors.background;
        transparent.a = 0.0;
        let input_font = Font {
            family: self.font_family.clone(),
            ..Font::default()
        };
        let mut input_selection = self.colors.cursor;
        input_selection.a = 0.28;
        let mut scrollbar_track = self.colors.cursor;
        scrollbar_track.a = 0.1;
        let mut scrollbar_thumb = self.colors.cursor;
        scrollbar_thumb.a = 0.42;

        let list = if items.is_empty() {
            div()
                .w_full()
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(8.0))
                        .text_size(px(12.0))
                        .text_color(muted_text)
                        .child("No matching commands"),
                )
                .into_any_element()
        } else {
            let list = uniform_list(
                "command-palette-list",
                items.len(),
                cx.processor(Self::render_command_palette_rows),
            )
            .flex_1()
            .h(px(list_height))
            .track_scroll(&self.command_palette_scroll_handle)
            .into_any_element();
            let mut list_container = div()
                .w_full()
                .h(px(list_height))
                .flex()
                .items_start()
                .child(list);

            if let Some((thumb_top, thumb_height)) =
                self.command_palette_scrollbar_metrics(list_height, items.len())
            {
                list_container = list_container.child(
                    div()
                        .w(px(COMMAND_PALETTE_SCROLLBAR_WIDTH + 4.0))
                        .h_full()
                        .pl(px(2.0))
                        .pr(px(2.0))
                        .child(
                            div()
                                .relative()
                                .w(px(COMMAND_PALETTE_SCROLLBAR_WIDTH))
                                .h_full()
                                .rounded_full()
                                .bg(scrollbar_track)
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(thumb_top))
                                        .left_0()
                                        .right_0()
                                        .h(px(thumb_height))
                                        .rounded_full()
                                        .bg(scrollbar_thumb),
                                ),
                        ),
                );
            }

            list_container.into_any_element()
        };

        div()
            .id("command-palette-modal")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .occlude()
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.close_command_palette(cx);
            }))
            .child(div().size_full().bg(overlay_bg).absolute().top_0().left_0())
            .child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .pt(px(36.0))
                    .child(
                        div()
                            .id("command-palette-panel")
                            .w(px(COMMAND_PALETTE_WIDTH))
                            .px(px(10.0))
                            .py(px(10.0))
                            .rounded_md()
                            .bg(panel_bg)
                            .border_1()
                            .border_color(panel_border)
                            .on_click(cx.listener(|_this, _event, _window, cx| {
                                cx.stop_propagation();
                            }))
                            .child(
                                div()
                                    .id("command-palette-input")
                                    .w_full()
                                    .h(px(34.0))
                                    .px(px(10.0))
                                    .py(px(8.0))
                                    .relative()
                                    .rounded_sm()
                                    .bg(transparent)
                                    .border_1()
                                    .border_color(panel_border)
                                    .child(div().w_full().h_full().relative().child(
                                        self.render_inline_input_layer(
                                            input_font.clone(),
                                            px(13.0),
                                            primary_text.into(),
                                            input_selection.into(),
                                            InlineInputAlignment::Left,
                                            cx,
                                        ),
                                    )),
                            )
                            .child(div().h(px(8.0)))
                            .child(list)
                            .child(
                                div()
                                    .pt(px(8.0))
                                    .text_size(px(11.0))
                                    .text_color(muted_text)
                                    .child("Enter: Run  Esc: Close  Up/Down: Navigate"),
                            ),
                    ),
            )
            .into_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_re_uses_word_prefix_matching() {
        let new_tab = CommandPaletteItem {
            title: "New Tab",
            keywords: "create tab",
            action: CommandAction::NewTab,
        };
        let rename_tab = CommandPaletteItem {
            title: "Rename Tab",
            keywords: "title name",
            action: CommandAction::RenameTab,
        };
        let restart_app = CommandPaletteItem {
            title: "Restart App",
            keywords: "relaunch reopen restart",
            action: CommandAction::RestartApp,
        };
        let reset_zoom = CommandPaletteItem {
            title: "Reset Zoom",
            keywords: "font default",
            action: CommandAction::ZoomReset,
        };

        assert!(!TerminalView::command_palette_item_matches_terms(
            &new_tab,
            &["re"]
        ));
        assert!(TerminalView::command_palette_item_matches_terms(
            &rename_tab,
            &["re"]
        ));
        assert!(TerminalView::command_palette_item_matches_terms(
            &restart_app,
            &["re"]
        ));
        assert!(TerminalView::command_palette_item_matches_terms(
            &reset_zoom,
            &["re"]
        ));
    }
}
