use super::*;

impl TerminalView {
    pub(super) fn is_command_palette_shortcut(key: &str, modifiers: gpui::Modifiers) -> bool {
        modifiers.secondary()
            && !modifiers.alt
            && !modifiers.function
            && key.eq_ignore_ascii_case("p")
    }

    pub(super) fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = true;
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.command_palette_query_select_all = false;
        self.command_palette_opened_at = Some(Instant::now());

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                smol::Timer::after(Duration::from_millis(500)).await;
                let should_continue = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        if !view.command_palette_open {
                            return false;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false)
                });

                if !should_continue {
                    break;
                }
            }
        })
        .detach();

        cx.notify();
    }

    pub(super) fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        if !self.command_palette_open {
            return;
        }

        self.command_palette_open = false;
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.command_palette_query_select_all = false;
        self.command_palette_opened_at = None;
        cx.notify();
    }

    fn command_palette_cursor_visible(&self) -> bool {
        let Some(opened_at) = self.command_palette_opened_at else {
            return true;
        };
        (opened_at.elapsed().as_millis() / 500).is_multiple_of(2)
    }

    fn command_palette_items(&self) -> Vec<CommandPaletteItem> {
        let mut items = vec![
            CommandPaletteItem {
                title: "Restart App",
                keywords: "relaunch reopen restart",
                action: CommandPaletteAction::RestartApp,
            },
            CommandPaletteItem {
                title: "Open Config",
                keywords: "settings preferences",
                action: CommandPaletteAction::OpenConfig,
            },
            CommandPaletteItem {
                title: "Zoom In",
                keywords: "font increase",
                action: CommandPaletteAction::ZoomIn,
            },
            CommandPaletteItem {
                title: "Zoom Out",
                keywords: "font decrease",
                action: CommandPaletteAction::ZoomOut,
            },
            CommandPaletteItem {
                title: "Reset Zoom",
                keywords: "font default",
                action: CommandPaletteAction::ResetZoom,
            },
        ];

        if self.use_tabs {
            items.insert(
                0,
                CommandPaletteItem {
                    title: "Rename Tab",
                    keywords: "title name",
                    action: CommandPaletteAction::RenameTab,
                },
            );
            items.insert(
                0,
                CommandPaletteItem {
                    title: "Close Tab",
                    keywords: "remove tab",
                    action: CommandPaletteAction::CloseTab,
                },
            );
            items.insert(
                0,
                CommandPaletteItem {
                    title: "New Tab",
                    keywords: "create tab",
                    action: CommandPaletteAction::NewTab,
                },
            );
        }

        #[cfg(target_os = "macos")]
        items.push(CommandPaletteItem {
            title: "Check for Updates",
            keywords: "release version updater",
            action: CommandPaletteAction::CheckForUpdates,
        });

        items
    }

    fn filtered_command_palette_items(&self) -> Vec<CommandPaletteItem> {
        let query = self.command_palette_query.trim().to_ascii_lowercase();
        self.command_palette_items()
            .into_iter()
            .filter(|item| {
                query.is_empty()
                    || item.title.to_ascii_lowercase().contains(&query)
                    || item.keywords.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    fn clamp_command_palette_selection(&mut self, len: usize) {
        if len == 0 {
            self.command_palette_selected = 0;
        } else if self.command_palette_selected >= len {
            self.command_palette_selected = len - 1;
        }
    }

    pub(super) fn handle_command_palette_key_down(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        if modifiers.secondary()
            && !modifiers.alt
            && !modifiers.function
            && key.eq_ignore_ascii_case("a")
        {
            self.command_palette_query_select_all = !self.command_palette_query.is_empty();
            cx.notify();
            return;
        }

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
                    cx.notify();
                }
                return;
            }
            "down" => {
                let len = self.filtered_command_palette_items().len();
                if len > 0 && self.command_palette_selected + 1 < len {
                    self.command_palette_selected += 1;
                    cx.notify();
                }
                return;
            }
            "backspace" => {
                if self.command_palette_query_select_all {
                    self.command_palette_query.clear();
                    self.command_palette_query_select_all = false;
                    let len = self.filtered_command_palette_items().len();
                    self.clamp_command_palette_selection(len);
                    cx.notify();
                    return;
                }
                if self.command_palette_query.pop().is_some() {
                    self.command_palette_query_select_all = false;
                    let len = self.filtered_command_palette_items().len();
                    self.clamp_command_palette_selection(len);
                    cx.notify();
                }
                return;
            }
            "space"
                if !modifiers.control
                    && !modifiers.platform
                    && !modifiers.alt
                    && !modifiers.function =>
            {
                if self.command_palette_query_select_all {
                    self.command_palette_query.clear();
                    self.command_palette_query_select_all = false;
                }
                self.command_palette_query.push(' ');
                let len = self.filtered_command_palette_items().len();
                self.clamp_command_palette_selection(len);
                cx.notify();
                return;
            }
            _ => {}
        }

        if !modifiers.control
            && !modifiers.platform
            && !modifiers.alt
            && !modifiers.function
            && let Some(input) = key_char
            && !input.is_empty()
        {
            if self.command_palette_query_select_all {
                self.command_palette_query.clear();
                self.command_palette_query_select_all = false;
            }
            self.command_palette_query.push_str(input);
            let len = self.filtered_command_palette_items().len();
            self.clamp_command_palette_selection(len);
            cx.notify();
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

    fn execute_command_palette_action(
        &mut self,
        action: CommandPaletteAction,
        cx: &mut Context<Self>,
    ) {
        self.command_palette_open = false;
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.command_palette_query_select_all = false;

        match action {
            CommandPaletteAction::RestartApp => match self.restart_application() {
                Ok(()) => cx.quit(),
                Err(error) => {
                    termy_toast::error(format!("Restart failed: {}", error));
                    cx.notify();
                }
            },
            CommandPaletteAction::NewTab => {
                self.add_tab(cx);
                termy_toast::success("Opened new tab");
            }
            CommandPaletteAction::CloseTab => {
                self.close_active_tab(cx);
                termy_toast::info("Closed active tab");
            }
            CommandPaletteAction::RenameTab => {
                self.renaming_tab = Some(self.active_tab);
                self.rename_buffer = self.tabs[self.active_tab].title.clone();
                termy_toast::info("Rename mode enabled");
                cx.notify();
            }
            CommandPaletteAction::OpenConfig => {
                config::open_config_file();
                termy_toast::info("Opened config file");
                cx.notify();
            }
            CommandPaletteAction::ZoomIn => {
                let current: f32 = self.font_size.into();
                self.update_zoom(current + ZOOM_STEP, cx);
                termy_toast::info("Zoomed in");
            }
            CommandPaletteAction::ZoomOut => {
                let current: f32 = self.font_size.into();
                self.update_zoom(current - ZOOM_STEP, cx);
                termy_toast::info("Zoomed out");
            }
            CommandPaletteAction::ResetZoom => {
                self.update_zoom(self.base_font_size, cx);
                termy_toast::info("Zoom reset");
            }
            #[cfg(target_os = "macos")]
            CommandPaletteAction::CheckForUpdates => {
                if let Some(updater) = self.auto_updater.as_ref() {
                    AutoUpdater::check(updater.downgrade(), cx);
                }
                termy_toast::info("Checking for updates");
                cx.notify();
            }
        }
    }

    pub(super) fn render_command_palette_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let items = self.filtered_command_palette_items();
        let selected = if items.is_empty() {
            0
        } else {
            self.command_palette_selected.min(items.len() - 1)
        };

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

        let mut list = div().flex().flex_col().gap(px(4.0));
        if items.is_empty() {
            list = list.child(
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .text_size(px(12.0))
                    .text_color(muted_text)
                    .child("No matching commands"),
            );
        } else {
            for (index, item) in items
                .into_iter()
                .take(COMMAND_PALETTE_MAX_ITEMS)
                .enumerate()
            {
                let is_selected = index == selected;
                let action = item.action;
                list = list.child(
                    div()
                        .id(("command-palette-item", index))
                        .w_full()
                        .px(px(10.0))
                        .py(px(8.0))
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
                            this.execute_command_palette_action(action, cx);
                            cx.stop_propagation();
                        }))
                        .text_size(px(12.0))
                        .text_color(primary_text)
                        .child(item.title),
                );
            }
        }

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
                                    .w_full()
                                    .px(px(10.0))
                                    .py(px(8.0))
                                    .rounded_sm()
                                    .bg(transparent)
                                    .border_1()
                                    .border_color(panel_border)
                                    .text_size(px(13.0))
                                    .text_color(if self.command_palette_query_select_all {
                                        self.colors.cursor
                                    } else {
                                        primary_text
                                    })
                                    .child(format!(
                                        "{}{}",
                                        self.command_palette_query,
                                        if self.command_palette_cursor_visible() {
                                            "▌"
                                        } else {
                                            " "
                                        }
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
