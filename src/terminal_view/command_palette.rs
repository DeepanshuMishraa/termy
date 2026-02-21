use super::*;
use gpui::{ScrollStrategy, uniform_list};
use std::ops::Range;

impl TerminalView {
    fn reset_command_palette_state(&mut self) {
        self.command_palette_input.clear();
        self.command_palette_selected = 0;
        self.command_palette_scroll_handle = UniformListScrollHandle::new();
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
        self.command_palette_items()
            .into_iter()
            .filter(|item| {
                query.is_empty()
                    || item.title.to_ascii_lowercase().contains(&query)
                    || item.keywords.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    pub(super) fn clamp_command_palette_selection(&mut self, len: usize) {
        if len == 0 {
            self.command_palette_selected = 0;
        } else if self.command_palette_selected >= len {
            self.command_palette_selected = len - 1;
        }
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
                if self.command_palette_selected > 0 {
                    self.command_palette_selected -= 1;
                    self.command_palette_scroll_handle
                        .scroll_to_item(self.command_palette_selected, ScrollStrategy::Nearest);
                    cx.notify();
                }
                return;
            }
            "down" => {
                let len = self.filtered_command_palette_items().len();
                if len > 0 && self.command_palette_selected + 1 < len {
                    self.command_palette_selected += 1;
                    self.command_palette_scroll_handle
                        .scroll_to_item(self.command_palette_selected, ScrollStrategy::Nearest);
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
        let mut hover_bg = self.colors.cursor;
        hover_bg.a = 0.12;
        let mut hover_border = self.colors.cursor;
        hover_border.a = 0.24;
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
                    .h(px(30.0))
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
                    .hover(|style| style.bg(hover_bg).border_color(hover_border))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.command_palette_selected = index;
                        this.execute_command_palette_action(action, cx);
                        cx.stop_propagation();
                    }))
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

    pub(super) fn render_command_palette_modal(
        &mut self,
        _window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = self.filtered_command_palette_items();
        let list_height = (COMMAND_PALETTE_MAX_ITEMS as f32 * 30.0)
            + (COMMAND_PALETTE_MAX_ITEMS.saturating_sub(1) as f32 * 4.0);

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
            uniform_list(
                "command-palette-list",
                items.len(),
                cx.processor(Self::render_command_palette_rows),
            )
            .w_full()
            .h(px(list_height))
            .track_scroll(&self.command_palette_scroll_handle)
            .into_any_element()
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
                                    .text_size(px(13.0))
                                    .text_color(primary_text)
                                    .child(
                                        div()
                                            .w_full()
                                            .h_full()
                                            .relative()
                                            .child(self.command_palette_input.text_with_cursor())
                                            .child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .left_0()
                                                    .right_0()
                                                    .bottom_0()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            Self::handle_inline_input_mouse_down,
                                                        ),
                                                    )
                                                    .on_mouse_move(cx.listener(
                                                        Self::handle_inline_input_mouse_move,
                                                    ))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            Self::handle_inline_input_mouse_up,
                                                        ),
                                                    )
                                                    .on_mouse_up_out(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            Self::handle_inline_input_mouse_up,
                                                        ),
                                                    )
                                                    .child(InlineInputElement::new(
                                                        cx.entity(),
                                                        self.focus_handle.clone(),
                                                        px(13.0),
                                                    )),
                                            ),
                                    ),
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
