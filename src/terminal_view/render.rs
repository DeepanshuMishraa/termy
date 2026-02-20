use super::*;

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.toast_manager.ingest_pending();
        self.toast_manager.tick();

        // Compute update banner state
        #[cfg(target_os = "macos")]
        let banner_state = self.auto_updater.as_ref().map(|e| e.read(cx).state.clone());
        #[cfg(target_os = "macos")]
        {
            self.sync_update_toasts(banner_state.as_ref());
            self.show_update_banner = matches!(
                &banner_state,
                Some(
                    UpdateState::Available { .. }
                        | UpdateState::Downloading { .. }
                        | UpdateState::Downloaded { .. }
                        | UpdateState::Installing { .. }
                        | UpdateState::Installed { .. }
                        | UpdateState::Error(_)
                )
            );
        }

        let cell_size = self.calculate_cell_size(window, cx);
        let colors = self.colors.clone();
        let font_family = self.font_family.clone();
        let font_size = self.font_size;
        let background_opacity = self.transparent_background_opacity;
        #[cfg(target_os = "windows")]
        let effective_background_opacity = if background_opacity < 1.0 { 1.0 } else { background_opacity };
        #[cfg(not(target_os = "windows"))]
        let effective_background_opacity = background_opacity;

        self.sync_terminal_size(window, cell_size);

        // Collect cells to render
        let mut cells_to_render: Vec<CellRenderInfo> = Vec::new();
        let (cursor_col, cursor_row) = self.active_terminal().cursor_position();

        self.active_terminal().with_term(|term| {
            let content = term.renderable_content();
            for cell in content.display_iter {
                let point = cell.point;
                let cell_content = &cell.cell;
                let row = point.line.0;
                if row < 0 {
                    continue;
                }
                let row = row as usize;
                let col = point.column.0;

                // Get foreground and background colors
                let mut fg = colors.convert(cell_content.fg);
                let mut bg = colors.convert(cell_content.bg);
                if cell_content.flags.contains(Flags::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if cell_content.flags.contains(Flags::DIM) {
                    fg.r *= DIM_TEXT_FACTOR;
                    fg.g *= DIM_TEXT_FACTOR;
                    fg.b *= DIM_TEXT_FACTOR;
                }

                let c = cell_content.c;
                let is_cursor = col == cursor_col && row == cursor_row;
                let selected = self.cell_is_selected(col, row);

                cells_to_render.push(CellRenderInfo {
                    col,
                    row,
                    char: c,
                    fg: fg.into(),
                    bg: bg.into(),
                    bold: cell_content.flags.contains(Flags::BOLD),
                    render_text: !cell_content.flags.intersects(
                        Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER | Flags::HIDDEN,
                    ),
                    is_cursor,
                    selected,
                });
            }
        });

        let terminal_size = self.active_terminal().size();
        let focus_handle = self.focus_handle.clone();
        let show_tab_bar = self.show_tab_bar();
        let show_windows_controls = cfg!(target_os = "windows");
        let show_titlebar_plus = self.use_tabs && !show_windows_controls;
        let titlebar_side_slot_width = if show_windows_controls {
            WINDOWS_TITLEBAR_CONTROLS_WIDTH
        } else {
            TITLEBAR_PLUS_SIZE
        };
        let viewport = window.viewport_size();
        let tab_layout = self.tab_bar_layout(viewport.width.into());
        let titlebar_height = self.titlebar_height();
        let mut titlebar_bg = colors.background;
        titlebar_bg.a = 0.96;
        let mut titlebar_border = colors.cursor;
        titlebar_border.a = 0.18;
        let mut titlebar_text = colors.foreground;
        titlebar_text.a = 0.82;
        let mut titlebar_plus_bg = colors.cursor;
        titlebar_plus_bg.a = if show_titlebar_plus { 0.2 } else { 0.0 };
        let mut titlebar_plus_text = colors.foreground;
        titlebar_plus_text.a = if show_titlebar_plus { 0.92 } else { 0.0 };
        let mut tabbar_bg = colors.background;
        tabbar_bg.a = if show_tab_bar { 0.92 } else { 0.0 };
        let mut tabbar_border = colors.cursor;
        tabbar_border.a = if show_tab_bar { 0.14 } else { 0.0 };
        let mut active_tab_bg = colors.cursor;
        active_tab_bg.a = 0.2;
        let mut active_tab_border = colors.cursor;
        active_tab_border.a = 0.32;
        let mut active_tab_text = colors.foreground;
        active_tab_text.a = 0.95;
        let mut inactive_tab_bg = colors.background;
        inactive_tab_bg.a = 0.56;
        let mut inactive_tab_border = colors.cursor;
        inactive_tab_border.a = 0.12;
        let mut inactive_tab_text = colors.foreground;
        inactive_tab_text.a = 0.68;
        let mut selection_bg = colors.cursor;
        selection_bg.a = SELECTION_BG_ALPHA;
        let selection_fg = colors.background;
        let hovered_link_range = self
            .hovered_link
            .as_ref()
            .map(|link| (link.row, link.start_col, link.end_col));

        let mut tabs_row = div()
            .w_full()
            .h(px(if show_tab_bar { TABBAR_HEIGHT } else { 0.0 }))
            .flex()
            .items_center()
            .px(px(TAB_HORIZONTAL_PADDING));

        if show_tab_bar {
            for (index, tab) in self.tabs.iter().enumerate() {
                let is_active = index == self.active_tab;
                let show_tab_close = Self::tab_shows_close(
                    tab_layout.tab_pill_width,
                    is_active,
                    tab_layout.tab_padding_x,
                );
                let close_slot_width = if show_tab_close {
                    TAB_CLOSE_HITBOX
                } else {
                    0.0
                };
                let label = if self.renaming_tab == Some(index) {
                    format!("{}|", self.rename_buffer)
                } else {
                    tab.title.clone()
                };

                tabs_row = tabs_row.child(
                    div()
                        .bg(if is_active {
                            active_tab_bg
                        } else {
                            inactive_tab_bg
                        })
                        .border_1()
                        .border_color(if is_active {
                            active_tab_border
                        } else {
                            inactive_tab_border
                        })
                        .w(px(tab_layout.tab_pill_width))
                        .h(px(TAB_PILL_HEIGHT))
                        .px(px(tab_layout.tab_padding_x))
                        .flex()
                        .items_center()
                        .child(div().w(px(close_slot_width)).h(px(TAB_CLOSE_HITBOX)))
                        .child(
                            div()
                                .flex_1()
                                .truncate()
                                .text_center()
                                .text_color(if is_active {
                                    active_tab_text
                                } else {
                                    inactive_tab_text
                                })
                                .text_size(px(12.0))
                                .child(label),
                        )
                        .children(show_tab_close.then(|| {
                            div()
                                .w(px(close_slot_width))
                                .h(px(TAB_CLOSE_HITBOX))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(if is_active {
                                    active_tab_text
                                } else {
                                    inactive_tab_text
                                })
                                .text_size(px(13.0))
                                .child("×")
                        })),
                );

                if index + 1 < self.tabs.len() {
                    tabs_row = tabs_row.child(div().w(px(TAB_PILL_GAP)).h(px(1.0)));
                }
            }
        }

        // Build update banner element (macOS only)
        #[cfg(target_os = "macos")]
        let banner_element: Option<AnyElement> = if self.show_update_banner {
            let mut banner_bg = colors.cursor;
            banner_bg.a = 0.15;
            let banner_text_color = colors.foreground;

            match &banner_state {
                Some(UpdateState::Available { version, .. }) => {
                    let version = version.clone();
                    let updater_weak = self.auto_updater.as_ref().map(|e| e.downgrade());
                    let updater_weak2 = updater_weak.clone();
                    Some(
                        div()
                            .id("update-banner")
                            .w_full()
                            .h(px(UPDATE_BANNER_HEIGHT))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(12.0))
                            .bg(banner_bg)
                            .text_color(banner_text_color)
                            .text_size(px(12.0))
                            .child(format!("Update v{} available", version))
                            .child(
                                div()
                                    .id("update-install-btn")
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .bg(colors.cursor)
                                    .text_color(colors.background)
                                    .text_size(px(11.0))
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |_this, _event, _window, cx| {
                                            if let Some(ref weak) = updater_weak {
                                                if let Some(entity) = weak.upgrade() {
                                                    AutoUpdater::install(entity.downgrade(), cx);
                                                    termy_toast::info("Downloading update...");
                                                }
                                            }
                                        }),
                                    )
                                    .child("Install"),
                            )
                            .child(
                                div()
                                    .id("update-dismiss-btn")
                                    .px(px(6.0))
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .text_size(px(13.0))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |_this, _event, _window, cx| {
                                            if let Some(ref weak) = updater_weak2 {
                                                if let Some(entity) = weak.upgrade() {
                                                    entity.update(cx, |u, cx| u.dismiss(cx));
                                                }
                                            }
                                        }),
                                    )
                                    .child("\u{00d7}"),
                            )
                            .into_any(),
                    )
                }
                Some(UpdateState::Downloading {
                    version,
                    downloaded,
                    total,
                }) => {
                    let progress_text = if *total > 0 {
                        format!(
                            "Downloading v{}... {}%",
                            version,
                            (*downloaded as f64 / *total as f64 * 100.0) as u32
                        )
                    } else {
                        format!("Downloading v{}... {} KB", version, *downloaded / 1024)
                    };
                    Some(
                        div()
                            .id("update-banner")
                            .w_full()
                            .h(px(UPDATE_BANNER_HEIGHT))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(banner_bg)
                            .text_color(banner_text_color)
                            .text_size(px(12.0))
                            .child(progress_text)
                            .into_any(),
                    )
                }
                Some(UpdateState::Downloaded { version, .. }) => {
                    let version = version.clone();
                    let updater_weak = self.auto_updater.as_ref().map(|e| e.downgrade());
                    Some(
                        div()
                            .id("update-banner")
                            .w_full()
                            .h(px(UPDATE_BANNER_HEIGHT))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(12.0))
                            .bg(banner_bg)
                            .text_color(banner_text_color)
                            .text_size(px(12.0))
                            .child(format!("v{} downloaded", version))
                            .child(
                                div()
                                    .id("update-install-btn")
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .bg(colors.cursor)
                                    .text_color(colors.background)
                                    .text_size(px(11.0))
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |_this, _event, _window, cx| {
                                            if let Some(ref weak) = updater_weak {
                                                if let Some(entity) = weak.upgrade() {
                                                    AutoUpdater::complete_install(
                                                        entity.downgrade(),
                                                        cx,
                                                    );
                                                    termy_toast::info("Starting installation...");
                                                }
                                            }
                                        }),
                                    )
                                    .child("Install Now"),
                            )
                            .into_any(),
                    )
                }
                Some(UpdateState::Installing { version }) => Some(
                    div()
                        .id("update-banner")
                        .w_full()
                        .h(px(UPDATE_BANNER_HEIGHT))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(banner_bg)
                        .text_color(banner_text_color)
                        .text_size(px(12.0))
                        .child(format!("Installing v{}...", version))
                        .into_any(),
                ),
                Some(UpdateState::Installed { version }) => {
                    let updater_weak = self.auto_updater.as_ref().map(|e| e.downgrade());
                    Some(
                        div()
                            .id("update-banner")
                            .w_full()
                            .h(px(UPDATE_BANNER_HEIGHT))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(12.0))
                            .bg(banner_bg)
                            .text_color(banner_text_color)
                            .text_size(px(12.0))
                            .child(format!("v{} installed — restart to complete", version))
                            .child(
                                div()
                                    .id("update-restart-btn")
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .bg(colors.cursor)
                                    .text_color(colors.background)
                                    .text_size(px(11.0))
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _event, _window, cx| {
                                            match this.restart_application() {
                                                Ok(()) => cx.quit(),
                                                Err(error) => {
                                                    termy_toast::error(format!(
                                                        "Restart failed: {}",
                                                        error
                                                    ));
                                                }
                                            }
                                        }),
                                    )
                                    .child("Restart"),
                            )
                            .child(
                                div()
                                    .id("update-dismiss-btn")
                                    .px(px(6.0))
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .text_size(px(13.0))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |_this, _event, _window, cx| {
                                            if let Some(ref weak) = updater_weak {
                                                if let Some(entity) = weak.upgrade() {
                                                    entity.update(cx, |u, cx| u.dismiss(cx));
                                                }
                                            }
                                        }),
                                    )
                                    .child("\u{00d7}"),
                            )
                            .into_any(),
                    )
                }
                Some(UpdateState::Error(msg)) => {
                    let updater_weak = self.auto_updater.as_ref().map(|e| e.downgrade());
                    Some(
                        div()
                            .id("update-banner")
                            .w_full()
                            .h(px(UPDATE_BANNER_HEIGHT))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(12.0))
                            .bg(banner_bg)
                            .text_color(banner_text_color)
                            .text_size(px(12.0))
                            .child(format!("Update error: {}", msg))
                            .child(
                                div()
                                    .id("update-dismiss-btn")
                                    .px(px(6.0))
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .text_size(px(13.0))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |_this, _event, _window, cx| {
                                            if let Some(ref weak) = updater_weak {
                                                if let Some(entity) = weak.upgrade() {
                                                    entity.update(cx, |u, cx| u.dismiss(cx));
                                                }
                                            }
                                        }),
                                    )
                                    .child("\u{00d7}"),
                            )
                            .into_any(),
                    )
                }
                _ => None,
            }
        } else {
            None
        };
        #[cfg(not(target_os = "macos"))]
        let banner_element: Option<AnyElement> = None;
        let mut terminal_surface_bg = colors.background;
        terminal_surface_bg.a *= effective_background_opacity;

        let terminal_grid = TerminalGrid {
            cells: cells_to_render,
            cell_size,
            cols: terminal_size.cols as usize,
            rows: terminal_size.rows as usize,
            default_bg: terminal_surface_bg.into(),
            cursor_color: colors.cursor.into(),
            selection_bg: selection_bg.into(),
            selection_fg: selection_fg.into(),
            hovered_link_range,
            font_family: font_family.clone(),
            font_size,
        };
        let command_palette_overlay = self
            .command_palette_open
            .then(|| self.render_command_palette_modal(cx));
        let titlebar_element: Option<AnyElement> = (titlebar_height > 0.0).then(|| {
            div()
                .id("titlebar")
                .w_full()
                .h(px(titlebar_height))
                .flex_none()
                .flex()
                .items_center()
                .window_control_area(WindowControlArea::Drag)
                .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_titlebar_mouse_down))
                .bg(titlebar_bg)
                .border_b(px(1.0))
                .border_color(titlebar_border)
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .px(px(TITLEBAR_SIDE_PADDING))
                        .child(
                            div()
                                .w(px(titlebar_side_slot_width))
                                .h(px(TITLEBAR_PLUS_SIZE)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_center()
                                .text_color(titlebar_text)
                                .text_size(px(12.0))
                                .child("Termy"),
                        )
                        .child(if show_windows_controls {
                            div()
                                .w(px(WINDOWS_TITLEBAR_CONTROLS_WIDTH))
                                .h(px(TITLEBAR_HEIGHT))
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .w(px(WINDOWS_TITLEBAR_BUTTON_WIDTH))
                                        .h(px(TITLEBAR_HEIGHT))
                                        .window_control_area(WindowControlArea::Min)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(titlebar_text)
                                        .text_size(px(12.0))
                                        .child("-"),
                                )
                                .child(
                                    div()
                                        .w(px(WINDOWS_TITLEBAR_BUTTON_WIDTH))
                                        .h(px(TITLEBAR_HEIGHT))
                                        .window_control_area(WindowControlArea::Max)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(titlebar_text)
                                        .text_size(px(12.0))
                                        .child("+"),
                                )
                                .child(
                                    div()
                                        .w(px(WINDOWS_TITLEBAR_BUTTON_WIDTH))
                                        .h(px(TITLEBAR_HEIGHT))
                                        .window_control_area(WindowControlArea::Close)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(titlebar_text)
                                        .text_size(px(12.0))
                                        .child("x"),
                                )
                        } else {
                            div()
                                .w(px(TITLEBAR_PLUS_SIZE))
                                .h(px(TITLEBAR_PLUS_SIZE))
                                .rounded_sm()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(titlebar_plus_bg)
                                .text_color(titlebar_plus_text)
                                .text_size(px(16.0))
                                .child(if show_titlebar_plus { "+" } else { "" })
                        }),
                )
                .into_any()
        });
        let toast_overlay = if self.toast_manager.active().is_empty() {
            None
        } else {
            let mut container = div().flex().flex_col().gap(px(8.0));
            for toast in self.toast_manager.active().iter() {
                let (symbol, accent) = match toast.kind {
                    termy_toast::ToastKind::Info => (
                        "i",
                        gpui::Rgba {
                            r: 0.45,
                            g: 0.67,
                            b: 0.98,
                            a: 1.0,
                        },
                    ),
                    termy_toast::ToastKind::Success => (
                        "+",
                        gpui::Rgba {
                            r: 0.44,
                            g: 0.85,
                            b: 0.55,
                            a: 1.0,
                        },
                    ),
                    termy_toast::ToastKind::Warning => (
                        "!",
                        gpui::Rgba {
                            r: 0.96,
                            g: 0.78,
                            b: 0.32,
                            a: 1.0,
                        },
                    ),
                    termy_toast::ToastKind::Error => (
                        "x",
                        gpui::Rgba {
                            r: 0.98,
                            g: 0.48,
                            b: 0.48,
                            a: 1.0,
                        },
                    ),
                };

                let mut bg = colors.background;
                bg.a = 0.94;
                let mut border = colors.cursor;
                border.a = 0.2;
                let mut text = colors.foreground;
                text.a = 0.96;

                container = container.child(
                    div()
                        .id(("toast", toast.id))
                        .max_w(px(440.0))
                        .rounded_md()
                        .bg(bg)
                        .border_1()
                        .border_color(border)
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .items_center()
                                .child(div().w(px(3.0)).h_full().bg(accent))
                                .child(
                                    div()
                                        .px(px(12.0))
                                        .py(px(10.0))
                                        .flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(
                                            div()
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .bg(accent)
                                                .text_size(px(11.0))
                                                .text_color(colors.background)
                                                .child(symbol),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_size(px(12.0))
                                                .text_color(text)
                                                .child(toast.message.clone()),
                                        ),
                                ),
                        ),
                );
            }

            Some(
                div()
                    .id("toast-overlay")
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .items_end()
                            .justify_end()
                            .pr(px(16.0))
                            .pb(px(16.0))
                            .child(container),
                    )
                    .into_any(),
            )
        };
        let mut root_bg = colors.background;
        root_bg.a *= effective_background_opacity;

        div()
            .id("termy-root")
            .flex()
            .flex_col()
            .size_full()
            .bg(root_bg)
            .children(titlebar_element)
            .child(
                div()
                    .id("tabbar")
                    .w_full()
                    .h(px(self.tab_bar_height()))
                    .flex_none()
                    .overflow_hidden()
                    .bg(tabbar_bg)
                    .border_b(px(if show_tab_bar { 1.0 } else { 0.0 }))
                    .border_color(tabbar_border)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(Self::handle_tabbar_mouse_down),
                    )
                    .child(tabs_row),
            )
            .children(banner_element)
            .child(
                div()
                    .id("terminal")
                    .track_focus(&focus_handle)
                    .key_context("Terminal")
                    .on_key_down(cx.listener(Self::handle_key_down))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
                    .on_mouse_move(cx.listener(Self::handle_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
                    .flex_1()
                    .w_full()
                    .px(px(self.padding_x))
                    .py(px(self.padding_y))
                    .overflow_hidden()
                    .bg(terminal_surface_bg)
                    .font_family(font_family.clone())
                    .text_size(font_size)
                    .child(terminal_grid)
                    .children(command_palette_overlay),
            )
            .children(toast_overlay)
    }
}


