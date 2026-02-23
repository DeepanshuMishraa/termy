use super::scrollbar as terminal_scrollbar;
use super::*;
use crate::ui::scrollbar::{self as ui_scrollbar, ScrollbarPaintStyle};

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TerminalView {
    fn refresh_terminal_scrollbar_marker_cache(
        &mut self,
        layout: terminal_scrollbar::TerminalScrollbarLayout,
        marker_height: f32,
    ) -> Option<f32> {
        if !self.search_open {
            self.clear_terminal_scrollbar_marker_cache();
            return None;
        }

        let marker_height = marker_height.max(0.0);
        let marker_top_limit =
            terminal_scrollbar::marker_top_limit(layout.metrics.track_height, marker_height);
        let cache_key = TerminalScrollbarMarkerCacheKey {
            results_revision: self.search_state.results_revision(),
            history_size: layout.history_size,
            viewport_rows: layout.viewport_rows,
            marker_top_limit_bucket: terminal_scrollbar::marker_top_limit_bucket(marker_top_limit),
        };
        let rebuild_markers = self.terminal_scrollbar_marker_cache.key.as_ref() != Some(&cache_key);

        let (is_empty, current_line, new_marker_tops) = {
            let results = self.search_state.results();
            if results.is_empty() {
                (true, None, None)
            } else {
                let current_line = results.current().map(|current| current.line);
                let new_marker_tops = rebuild_markers.then(|| {
                    terminal_scrollbar::deduped_marker_tops(
                        results
                            .matches()
                            .iter()
                            .map(|search_match| search_match.line),
                        layout.history_size,
                        layout.viewport_rows,
                        marker_height,
                        marker_top_limit,
                    )
                });
                (false, current_line, new_marker_tops)
            }
        };

        if is_empty {
            self.clear_terminal_scrollbar_marker_cache();
            return None;
        }

        if let Some(marker_tops) = new_marker_tops {
            self.terminal_scrollbar_marker_cache.marker_tops = marker_tops;
            self.terminal_scrollbar_marker_cache.key = Some(cache_key);
        }

        current_line.map(|line| {
            terminal_scrollbar::marker_top_for_line(
                line,
                layout.history_size,
                layout.viewport_rows,
                marker_top_limit,
            )
        })
    }

    fn render_terminal_scrollbar_overlay(
        &mut self,
        layout: terminal_scrollbar::TerminalScrollbarLayout,
        force_visible: bool,
    ) -> Option<AnyElement> {
        let now = Instant::now();
        let force_visible = force_visible
            && self.terminal_scrollbar_mode() != ui_scrollbar::ScrollbarVisibilityMode::AlwaysOff;
        let alpha = if force_visible {
            1.0
        } else {
            self.terminal_scrollbar_alpha(now)
        };
        if alpha <= f32::EPSILON && !self.terminal_scrollbar_visibility_controller.is_dragging() {
            return None;
        }
        let overlay_style = self.overlay_style();
        let gutter_bg = overlay_style.panel_background(TERMINAL_SCROLLBAR_GUTTER_ALPHA);
        let style = ScrollbarPaintStyle {
            width: TERMINAL_SCROLLBAR_TRACK_WIDTH,
            track_radius: TERMINAL_SCROLLBAR_TRACK_RADIUS,
            thumb_radius: TERMINAL_SCROLLBAR_THUMB_RADIUS,
            thumb_inset: TERMINAL_SCROLLBAR_THUMB_INSET,
            marker_inset: TERMINAL_SCROLLBAR_THUMB_INSET,
            marker_radius: TERMINAL_SCROLLBAR_THUMB_RADIUS,
            track_color: self.scrollbar_color(overlay_style, TERMINAL_SCROLLBAR_TRACK_ALPHA),
            thumb_color: self.scrollbar_color(overlay_style, TERMINAL_SCROLLBAR_THUMB_ALPHA),
            active_thumb_color: self
                .scrollbar_color(overlay_style, TERMINAL_SCROLLBAR_THUMB_ACTIVE_ALPHA),
            marker_color: Some(
                self.scrollbar_color(overlay_style, TERMINAL_SCROLLBAR_MATCH_MARKER_ALPHA),
            ),
            current_marker_color: Some(
                overlay_style.panel_cursor(TERMINAL_SCROLLBAR_CURRENT_MARKER_ALPHA),
            ),
        }
        .scale_alpha(alpha);

        let current_marker_top =
            self.refresh_terminal_scrollbar_marker_cache(layout, TERMINAL_SCROLLBAR_MARKER_HEIGHT);
        let marker_tops = &self.terminal_scrollbar_marker_cache.marker_tops;

        Some(
            div()
                .id("terminal-scrollbar-overlay")
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(TERMINAL_SCROLLBAR_GUTTER_WIDTH))
                .bg(gutter_bg)
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_0()
                        .w(px(TERMINAL_SCROLLBAR_TRACK_WIDTH))
                        .child(ui_scrollbar::render_vertical(
                            "terminal-scrollbar",
                            layout.metrics,
                            style,
                            self.terminal_scrollbar_visibility_controller.is_dragging(),
                            marker_tops,
                            current_marker_top,
                            TERMINAL_SCROLLBAR_MARKER_HEIGHT,
                        )),
                )
                .into_any_element(),
        )
    }

    #[cfg(target_os = "macos")]
    fn render_update_banner(
        &mut self,
        state: &UpdateState,
        colors: &TerminalColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let model = termy_auto_update_ui::UpdateBannerModel::from_state(state)?;
        let updater_weak = self.auto_updater.as_ref().map(|e| e.downgrade());

        let mut banner_bg = colors.background;
        banner_bg.a = 0.88;
        let mut border_color = colors.foreground;
        border_color.a = 0.16;
        let mut muted_text = colors.foreground;
        muted_text.a = 0.72;

        let tone = match model.tone {
            termy_auto_update_ui::UpdateBannerTone::Info => {
                let mut color = colors.cursor;
                color.a = 0.22;
                color
            }
            termy_auto_update_ui::UpdateBannerTone::Success => gpui::Rgba {
                r: 0.25,
                g: 0.66,
                b: 0.36,
                a: 0.24,
            },
            termy_auto_update_ui::UpdateBannerTone::Error => gpui::Rgba {
                r: 0.85,
                g: 0.31,
                b: 0.31,
                a: 0.24,
            },
        };

        let mut actions = div().flex().items_center().gap(px(6.0));
        for button in model.buttons {
            let action = button.action;
            let updater_weak = updater_weak.clone();
            let (button_bg, button_text, button_border) = match button.style {
                termy_auto_update_ui::UpdateButtonStyle::Primary => {
                    let mut bg = colors.cursor;
                    bg.a = 0.96;
                    (
                        bg,
                        colors.background,
                        gpui::Rgba {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        },
                    )
                }
                termy_auto_update_ui::UpdateButtonStyle::Secondary => {
                    let mut bg = colors.foreground;
                    bg.a = 0.08;
                    let mut border = colors.foreground;
                    border.a = 0.2;
                    (bg, colors.foreground, border)
                }
            };

            actions = actions.child(
                div()
                    .px(px(9.0))
                    .py(px(3.0))
                    .rounded_md()
                    .bg(button_bg)
                    .border_1()
                    .border_color(button_border)
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(button_text)
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event, _window, cx| match action {
                            termy_auto_update_ui::UpdateBannerAction::Install => {
                                if let Some(ref weak) = updater_weak
                                    && let Some(entity) = weak.upgrade()
                                {
                                    AutoUpdater::install(entity.downgrade(), cx);
                                    termy_toast::info("Downloading update...");
                                }
                            }
                            termy_auto_update_ui::UpdateBannerAction::CompleteInstall => {
                                if let Some(ref weak) = updater_weak
                                    && let Some(entity) = weak.upgrade()
                                {
                                    AutoUpdater::complete_install(entity.downgrade(), cx);
                                    termy_toast::info("Starting installation...");
                                }
                            }
                            termy_auto_update_ui::UpdateBannerAction::Restart => {
                                match this.restart_application() {
                                    Ok(()) => cx.quit(),
                                    Err(error) => {
                                        termy_toast::error(format!("Restart failed: {}", error));
                                    }
                                }
                            }
                            termy_auto_update_ui::UpdateBannerAction::Dismiss => {
                                if let Some(ref weak) = updater_weak
                                    && let Some(entity) = weak.upgrade()
                                {
                                    entity.update(cx, |updater, cx| updater.dismiss(cx));
                                }
                            }
                        }),
                    )
                    .child(button.label),
            );
        }

        let progress_element = model.progress_percent.map(|progress| {
            let mut progress_track = colors.foreground;
            progress_track.a = 0.14;
            let progress_width = 130.0;
            let fill_width = (f32::from(progress) / 100.0) * progress_width;

            div()
                .mt(px(2.0))
                .w(px(progress_width))
                .h(px(4.0))
                .rounded_full()
                .bg(progress_track)
                .child(
                    div()
                        .h_full()
                        .w(px(fill_width.max(0.0)))
                        .rounded_full()
                        .bg(colors.cursor),
                )
                .into_any()
        });

        Some(
            div()
                .id("update-banner")
                .w_full()
                .h(px(UPDATE_BANNER_HEIGHT))
                .flex_none()
                .bg(banner_bg)
                .border_b_1()
                .border_color(border_color)
                .child(
                    div()
                        .size_full()
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(3.0))
                                        .rounded_full()
                                        .bg(tone)
                                        .text_size(px(10.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(colors.foreground)
                                        .child(model.badge),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(colors.foreground)
                                                .child(model.message),
                                        )
                                        .children(model.detail.map(|detail| {
                                            div()
                                                .text_size(px(10.0))
                                                .text_color(muted_text)
                                                .child(detail)
                                                .into_any()
                                        }))
                                        .children(progress_element),
                                ),
                        )
                        .child(actions),
                )
                .into_any(),
        )
    }

    fn render_chrome_icon_button(
        &self,
        id: &'static str,
        label: &'static str,
        icon_size: f32,
        baseline_nudge_y: f32,
        bg: gpui::Rgba,
        text: gpui::Rgba,
        on_click: impl Fn(&mut Self, &MouseDownEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(id)
            .w(px(TITLEBAR_PLUS_SIZE))
            .h(px(TITLEBAR_PLUS_SIZE))
            .rounded_sm()
            .bg(bg)
            .text_color(text)
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    on_click(this, event, window, cx);
                }),
            )
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(icon_size))
                    .mt(px(baseline_nudge_y))
                    .child(label),
            )
            .into_any_element()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Process pending OSC 52 clipboard writes
        if let Some(text) = self.pending_clipboard.take() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }

        self.toast_manager.ingest_pending();
        self.toast_manager.tick_with_hovered(self.hovered_toast);
        if let Some((_, copied_at)) = self.copied_toast_feedback
            && copied_at.elapsed() >= Duration::from_millis(TOAST_COPY_FEEDBACK_MS)
        {
            self.copied_toast_feedback = None;
        }

        // Request re-render during toast animations for smooth fade in/out
        // Only schedule one timer at a time to avoid spawning 60 tasks/sec
        if self.toast_manager.is_animating() && !self.toast_animation_scheduled {
            self.toast_animation_scheduled = true;
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                smol::Timer::after(Duration::from_millis(16)).await;
                let _ = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        view.toast_animation_scheduled = false;
                        cx.notify();
                    })
                });
            })
            .detach();
        }

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
        self.sync_window_background_appearance(window);
        let effective_background_opacity = self.background_opacity_factor();
        let (effective_padding_x, effective_padding_y) = self.effective_terminal_padding();

        self.sync_terminal_size(window, cell_size);

        // Collect cells to render - pre-allocate based on terminal size to avoid reallocations
        let terminal_size = self.active_terminal().size();
        let estimated_cells = (terminal_size.cols as usize) * (terminal_size.rows as usize);
        let mut cells_to_render: Vec<CellRenderInfo> = Vec::with_capacity(estimated_cells);
        let (cursor_col, cursor_row) = self.active_terminal().cursor_position();
        let terminal_cursor_active =
            !self.command_palette_open && self.renaming_tab.is_none() && !self.search_open;
        let cursor_visible = terminal_cursor_active
            && self.cursor_visible_for_focus(self.focus_handle.is_focused(window));

        // Pre-compute search match info
        let search_active = self.search_open;
        let search_results = if search_active {
            Some(self.search_state.results())
        } else {
            None
        };
        let mut terminal_display_offset = 0usize;

        self.active_terminal().with_term(|term| {
            let content = term.renderable_content();
            terminal_display_offset = content.display_offset;
            let show_cursor = content.display_offset == 0 && cursor_visible;
            for cell in content.display_iter {
                let point = cell.point;
                let cell_content = &cell.cell;
                let term_line = point.line.0;
                let Some(row) =
                    Self::viewport_row_from_term_line(term_line, content.display_offset)
                else {
                    continue;
                };
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
                bg.a *= effective_background_opacity;

                let c = cell_content.c;
                let is_cursor = show_cursor && col == cursor_col && row == cursor_row;
                let selected = self.cell_is_selected(col, row);

                // Check search matches
                let (search_current, search_match) = if let Some(results) = &search_results {
                    let is_current = results.is_current_match(term_line, col);
                    let is_any = results.is_any_match(term_line, col);
                    (is_current, is_any && !is_current)
                } else {
                    (false, false)
                };

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
                    search_current,
                    search_match,
                });
            }
        });

        let focus_handle = self.focus_handle.clone();
        let show_tab_bar = self.show_tab_bar();
        let show_custom_titlebar_buttons = !self.hide_titlebar_buttons;
        let show_titlebar_update_button = show_custom_titlebar_buttons && cfg!(target_os = "macos");
        let show_titlebar_new_tab_button = show_custom_titlebar_buttons && self.use_tabs;
        let custom_titlebar_button_count = if show_custom_titlebar_buttons {
            1 + usize::from(show_titlebar_update_button) + usize::from(show_titlebar_new_tab_button)
        } else {
            0
        };
        let titlebar_side_slot_width = if custom_titlebar_button_count == 0 {
            0.0
        } else {
            let button_count = custom_titlebar_button_count as f32;
            (button_count * TITLEBAR_PLUS_SIZE) + ((button_count - 1.0) * TITLEBAR_BUTTON_GAP)
        };
        let titlebar_left_padding = if cfg!(target_os = "macos") {
            TOP_STRIP_MACOS_TRAFFIC_LIGHT_PADDING
        } else {
            TOP_STRIP_SIDE_PADDING
        };
        let titlebar_height = self.titlebar_height();
        let mut terminal_surface_bg = colors.background;
        terminal_surface_bg.a = self.scaled_background_alpha(terminal_surface_bg.a);
        let titlebar_bg = terminal_surface_bg;
        let mut titlebar_brand_text = colors.foreground;
        titlebar_brand_text.a = 0.9;
        let mut titlebar_context_text = colors.foreground;
        titlebar_context_text.a = 0.62;
        let mut titlebar_plus_bg = colors.foreground;
        titlebar_plus_bg.a = if show_custom_titlebar_buttons {
            self.scaled_chrome_alpha(0.08)
        } else {
            0.0
        };
        let mut titlebar_plus_text = colors.foreground;
        titlebar_plus_text.a = if show_custom_titlebar_buttons {
            0.92
        } else {
            0.0
        };
        let mut tabbar_bg = terminal_surface_bg;
        if !show_tab_bar {
            tabbar_bg.a = 0.0;
        }
        let tab_stroke_color = tab_chrome::resolve_tab_stroke_color(
            tabbar_bg,
            colors.foreground,
            TAB_STROKE_FOREGROUND_MIX,
        );
        let mut inactive_tab_bg = colors.foreground;
        inactive_tab_bg.a = self.scaled_chrome_alpha(0.10);
        let mut active_tab_bg = terminal_surface_bg;
        active_tab_bg.a = 0.0;
        let mut hovered_tab_bg = colors.foreground;
        hovered_tab_bg.a = self.scaled_chrome_alpha(0.13);
        let mut active_tab_text = colors.foreground;
        active_tab_text.a = 0.95;
        let mut inactive_tab_text = colors.foreground;
        inactive_tab_text.a = 0.7;
        let mut close_button_hover_bg = colors.foreground;
        close_button_hover_bg.a = self.scaled_chrome_alpha(0.24);
        let mut close_button_hover_text = colors.foreground;
        close_button_hover_text.a = 0.98;
        let mut selection_bg = colors.cursor;
        selection_bg.a = SELECTION_BG_ALPHA;
        let selection_fg = colors.background;
        let active_context_label = self.active_context_title().to_string();
        let hovered_link_range = self
            .hovered_link
            .as_ref()
            .map(|link| (link.row, link.start_col, link.end_col));
        let active_tab_index = (self.active_tab < self.tabs.len()).then_some(self.active_tab);
        let tab_chrome_layout = show_tab_bar.then(|| {
            tab_chrome::compute_tab_chrome_layout(
                self.tabs.iter().map(|tab| tab.display_width),
                tab_chrome::TabChromeInput {
                    active_index: active_tab_index,
                    tabbar_height: TABBAR_HEIGHT,
                    tab_item_height: TAB_ITEM_HEIGHT,
                    horizontal_padding: TAB_HORIZONTAL_PADDING,
                    tab_item_gap: TAB_ITEM_GAP,
                },
            )
        });
        debug_assert!(
            tab_chrome_layout
                .as_ref()
                .is_none_or(|layout| layout.tab_strokes.len() == self.tabs.len())
        );
        let render_tab_stroke = |stroke: tab_chrome::StrokeRect| {
            div()
                .absolute()
                .left(px(stroke.x))
                .top(px(stroke.y))
                .w(px(stroke.w))
                .h(px(stroke.h))
                .bg(tab_stroke_color)
        };
        let mut tabs_scroll_content = div()
            .id("tabs-scroll-content")
            .flex_1()
            .min_w(px(0.0))
            .h(px(if show_tab_bar { TABBAR_HEIGHT } else { 0.0 }))
            .flex()
            .relative()
            .items_end()
            .pl(px(TAB_HORIZONTAL_PADDING))
            .pr(px(TAB_HORIZONTAL_PADDING))
            .gap(px(TAB_ITEM_GAP))
            .overflow_x_scroll()
            .track_scroll(&self.tab_strip_scroll_handle)
            .on_scroll_wheel(
                cx.listener(|_this, _event: &ScrollWheelEvent, _window, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, _event: &MouseMoveEvent, _window, cx| {
                if this.hovered_tab.take().is_some() {
                    cx.notify();
                }
            }));

        if show_tab_bar {
            let tab_chrome_layout = tab_chrome_layout
                .as_ref()
                .expect("tab chrome layout must exist when tab bar is visible");
            for (index, tab) in self.tabs.iter().enumerate() {
                let switch_tab_index = index;
                let hover_tab_index = index;
                let close_tab_index = index;
                let is_active = index == self.active_tab;
                let is_hovered = self.hovered_tab == Some(index);
                let show_tab_close = Self::tab_shows_close(is_active, self.hovered_tab, index);
                let is_renaming = self.renaming_tab == Some(index);
                let label = tab.title.clone();
                let rename_text_color = if is_active {
                    active_tab_text
                } else {
                    inactive_tab_text
                };
                let mut rename_selection_color = colors.cursor;
                rename_selection_color.a = if is_active { 0.34 } else { 0.24 };

                let tab_bg = if is_active {
                    active_tab_bg
                } else if is_hovered {
                    hovered_tab_bg
                } else {
                    inactive_tab_bg
                };
                let tab_strokes = tab_chrome_layout.tab_strokes[index];

                let mut close_text_color = if is_active {
                    active_tab_text
                } else {
                    inactive_tab_text
                };
                if !show_tab_close {
                    close_text_color.a = 0.0;
                }

                let mut close_button = div()
                    .w(px(TAB_CLOSE_SLOT_WIDTH))
                    .h(px(TAB_CLOSE_HITBOX))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.0))
                    .text_color(close_text_color)
                    .text_size(px(12.0))
                    .child("×");
                if show_tab_close {
                    close_button = close_button
                        .hover(move |style| {
                            style
                                .bg(close_button_hover_bg)
                                .text_color(close_button_hover_text)
                        })
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _event: &MouseDownEvent, _window, cx| {
                                this.close_tab(close_tab_index, cx);
                                this.finish_tab_drag();
                                cx.stop_propagation();
                            }),
                        )
                        .on_mouse_move(cx.listener(
                            move |this, _event: &MouseMoveEvent, _window, cx| {
                                if this.hovered_tab != Some(hover_tab_index) {
                                    this.hovered_tab = Some(hover_tab_index);
                                    cx.notify();
                                }
                                cx.stop_propagation();
                            },
                        ));
                }

                let tab_shell = div()
                    .flex_none()
                    .relative()
                    .bg(tab_bg)
                    .w(px(tab.display_width))
                    .h(px(TAB_ITEM_HEIGHT))
                    .px(px(TAB_TEXT_PADDING_X))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                            this.switch_tab(switch_tab_index, cx);
                            this.begin_tab_drag(switch_tab_index);
                            if event.click_count == 2 {
                                this.begin_rename_tab(switch_tab_index, cx);
                                this.finish_tab_drag();
                            }
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_move(
                        cx.listener(move |this, event: &MouseMoveEvent, _window, cx| {
                            if this.hovered_tab != Some(hover_tab_index) {
                                this.hovered_tab = Some(hover_tab_index);
                                cx.notify();
                            }
                            if event.dragging() {
                                this.drag_tab_to(hover_tab_index, event.position.x.into(), cx);
                            }
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _event: &MouseUpEvent, _window, _cx| {
                            this.finish_tab_drag();
                        }),
                    )
                    .child(render_tab_stroke(tab_strokes.top))
                    .children(tab_strokes.left_boundary.map(render_tab_stroke))
                    .children(tab_strokes.right_boundary.map(render_tab_stroke));

                tabs_scroll_content = tabs_scroll_content.child(
                    tab_shell
                        .child(div().flex_1().min_w(px(0.0)).h_full().relative().child(
                            if is_renaming {
                                self.render_inline_input_layer(
                                    Font::default(),
                                    px(12.0),
                                    rename_text_color.into(),
                                    rename_selection_color.into(),
                                    InlineInputAlignment::Left,
                                    cx,
                                )
                            } else {
                                let title_is_path_like =
                                    label.contains('/') || label.contains('\\');
                                let mut title_text = div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .overflow_x_hidden()
                                    .whitespace_nowrap()
                                    .text_color(rename_text_color)
                                    .text_size(px(12.0));
                                title_text = if title_is_path_like {
                                    title_text.text_ellipsis_start()
                                } else {
                                    title_text.text_ellipsis()
                                };
                                title_text.child(label).into_any_element()
                            },
                        ))
                        .child(close_button),
                );
            }
        }

        if let Some(layout) = tab_chrome_layout.as_ref() {
            for segment in &layout.baseline_strokes {
                tabs_scroll_content = tabs_scroll_content.child(render_tab_stroke(*segment));
            }
            if let Some(start_x) = layout.open_ended_baseline_start_x {
                tabs_scroll_content = tabs_scroll_content.child(
                    div()
                        .absolute()
                        .left(px(start_x))
                        .right_0()
                        .top(px(layout.baseline_y))
                        .h(px(TAB_STROKE_THICKNESS))
                        .bg(tab_stroke_color),
                );
            }
        }

        let tabs_row = div()
            .w_full()
            .h(px(if show_tab_bar { TABBAR_HEIGHT } else { 0.0 }))
            .relative()
            .child(
                div()
                    .w_full()
                    .h_full()
                    .flex()
                    .items_end()
                    .child(tabs_scroll_content),
            );

        // Build update banner element (macOS only)
        #[cfg(target_os = "macos")]
        let banner_element: Option<AnyElement> = banner_state
            .as_ref()
            .and_then(|state| self.render_update_banner(state, &colors, cx));
        #[cfg(not(target_os = "macos"))]
        let banner_element: Option<AnyElement> = None;
        let terminal_surface_bg_hsla: gpui::Hsla = terminal_surface_bg.into();

        // Search highlight colors tuned for strong contrast on dark terminal themes.
        let search_match_bg = gpui::Hsla {
            h: 0.14,
            s: 0.92,
            l: 0.62,
            a: 0.62,
        };
        let search_current_bg = gpui::Hsla {
            h: 0.09,
            s: 0.98,
            l: 0.56,
            a: 0.86,
        };

        let terminal_grid = TerminalGrid {
            cells: cells_to_render,
            cell_size,
            cols: terminal_size.cols as usize,
            rows: terminal_size.rows as usize,
            clear_bg: gpui::Hsla::transparent_black(),
            default_bg: terminal_surface_bg_hsla,
            cursor_color: colors.cursor.into(),
            selection_bg: selection_bg.into(),
            selection_fg: selection_fg.into(),
            search_match_bg,
            search_current_bg,
            hovered_link_range,
            font_family: font_family.clone(),
            font_size,
            cursor_style: self.terminal_cursor_style(),
        };
        if self.terminal_scrollbar_mode() == ui_scrollbar::ScrollbarVisibilityMode::OnScroll
            && !self.terminal_scrollbar_animation_active
            && self.terminal_scrollbar_needs_animation(Instant::now())
        {
            self.start_terminal_scrollbar_animation(cx);
        }
        let terminal_track_height = self
            .terminal_surface_geometry(window)
            .map(|geometry| geometry.height)
            .unwrap_or(0.0);
        let terminal_scrollbar_layout =
            self.terminal_scrollbar_layout_for_track(terminal_track_height);
        if terminal_scrollbar_layout.is_none() {
            self.clear_terminal_scrollbar_marker_cache();
        }
        let terminal_scrollbar_overlay = terminal_scrollbar_layout.and_then(|layout| {
            self.render_terminal_scrollbar_overlay(layout, terminal_display_offset > 0)
        });
        let terminal_grid_layer = if let Some(viewport) = self.terminal_viewport_geometry() {
            div()
                .relative()
                .w(px(viewport.width))
                .h(px(viewport.height))
                .child(terminal_grid)
                .into_any_element()
        } else {
            div().child(terminal_grid).into_any_element()
        };
        let command_palette_overlay = if self.command_palette_open {
            Some(self.render_command_palette_modal(cx))
        } else {
            None
        };
        let search_overlay = if self.search_open {
            Some(self.render_search_bar(cx))
        } else {
            None
        };
        let key_context = if self.has_active_inline_input() {
            "Terminal InlineInput"
        } else {
            "Terminal"
        };
        let titlebar_element: Option<AnyElement> = (titlebar_height > 0.0).then(|| {
            let mut right_controls = div()
                .w(px(titlebar_side_slot_width))
                .h(px(TITLEBAR_PLUS_SIZE));
            if show_custom_titlebar_buttons {
                right_controls = div()
                    .flex()
                    .items_center()
                    .gap(px(TITLEBAR_BUTTON_GAP))
                    .child(self.render_chrome_icon_button(
                        "titlebar-settings",
                        "\u{2699}",
                        TITLEBAR_SETTINGS_ICON_SIZE,
                        TITLEBAR_SETTINGS_ICON_BASELINE_NUDGE_Y,
                        titlebar_plus_bg,
                        titlebar_plus_text,
                        |this, _event, window, cx| {
                            this.execute_command_action(
                                CommandAction::OpenSettings,
                                false,
                                window,
                                cx,
                            );
                            cx.stop_propagation();
                        },
                        cx,
                    ))
                    .children(show_titlebar_update_button.then(|| {
                        self.render_chrome_icon_button(
                            "titlebar-update",
                            "\u{21BB}",
                            TITLEBAR_BUTTON_ICON_SIZE,
                            TITLEBAR_BUTTON_ICON_BASELINE_NUDGE_Y,
                            titlebar_plus_bg,
                            titlebar_plus_text,
                            |this, _event, window, cx| {
                                this.execute_command_action(
                                    CommandAction::CheckForUpdates,
                                    false,
                                    window,
                                    cx,
                                );
                                cx.stop_propagation();
                            },
                            cx,
                        )
                    }))
                    .children(show_titlebar_new_tab_button.then(|| {
                        self.render_chrome_icon_button(
                            "titlebar-new-tab",
                            "+",
                            TITLEBAR_NEW_TAB_ICON_SIZE,
                            TITLEBAR_BUTTON_ICON_BASELINE_NUDGE_Y,
                            titlebar_plus_bg,
                            titlebar_plus_text,
                            |this, _event, _window, cx| {
                                this.add_tab(cx);
                                cx.stop_propagation();
                            },
                            cx,
                        )
                    }));
            }

            div()
                .id("titlebar")
                .w_full()
                .h(px(titlebar_height))
                .flex_none()
                .flex()
                .items_center()
                .window_control_area(WindowControlArea::Drag)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(Self::handle_titlebar_mouse_down),
                )
                .on_mouse_move(cx.listener(|this, _event: &MouseMoveEvent, _window, cx| {
                    let mut changed = false;
                    if this.hovered_tab.take().is_some() {
                        changed = true;
                    }
                    if this.tab_drag.is_some() {
                        this.finish_tab_drag();
                    }
                    if changed {
                        cx.notify();
                    }
                }))
                .bg(titlebar_bg)
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .mt(px(TOP_STRIP_CONTENT_OFFSET_Y))
                        .gap(px(8.0))
                        .pl(px(titlebar_left_padding))
                        .pr(px(TOP_STRIP_SIDE_PADDING))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .overflow_x_hidden()
                                .child(
                                    div()
                                        .mt(px(TOP_STRIP_TEXT_BASELINE_NUDGE_Y))
                                        .text_color(titlebar_brand_text)
                                        .text_size(px(TOP_STRIP_BRAND_TEXT_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("termy"),
                                )
                                .child(
                                    div()
                                        .mt(px(TOP_STRIP_TEXT_BASELINE_NUDGE_Y))
                                        .flex_1()
                                        .overflow_x_hidden()
                                        .truncate()
                                        .text_color(titlebar_context_text)
                                        .text_size(px(TOP_STRIP_CONTEXT_TEXT_SIZE))
                                        .child(active_context_label),
                                ),
                        )
                        .child(right_controls),
                )
                .into_any()
        });
        let toast_overlay = if self.toast_manager.active().is_empty() {
            None
        } else {
            let mut container = div().flex().flex_col().gap(px(6.0));
            for toast in self.toast_manager.active().iter() {
                let toast_id = toast.id;
                let toast_message = toast.message.clone();
                let is_hovered = self.hovered_toast == Some(toast_id);
                let is_copied = self
                    .copied_toast_feedback
                    .is_some_and(|(id, _)| id == toast_id);

                // Animation values
                let opacity = toast.opacity();
                let slide_offset = toast.slide_offset();

                // Clean, minimal icons and subtle accent colors
                let (icon, accent, _is_loading) = match toast.kind {
                    termy_toast::ToastKind::Info => (
                        "\u{2139}", // ℹ info symbol
                        gpui::Rgba {
                            r: 0.53,
                            g: 0.70,
                            b: 0.92,
                            a: opacity,
                        },
                        false,
                    ),
                    termy_toast::ToastKind::Success => (
                        "\u{2713}", // ✓ checkmark
                        gpui::Rgba {
                            r: 0.42,
                            g: 0.78,
                            b: 0.55,
                            a: opacity,
                        },
                        false,
                    ),
                    termy_toast::ToastKind::Warning => (
                        "\u{26A0}", // ⚠ warning
                        gpui::Rgba {
                            r: 0.94,
                            g: 0.76,
                            b: 0.38,
                            a: opacity,
                        },
                        false,
                    ),
                    termy_toast::ToastKind::Error => (
                        "\u{2715}", // ✕ x mark
                        gpui::Rgba {
                            r: 0.92,
                            g: 0.45,
                            b: 0.45,
                            a: opacity,
                        },
                        false,
                    ),
                    termy_toast::ToastKind::Loading => {
                        // Animated spinner using braille characters
                        const SPINNER_FRAMES: &[&str] =
                            &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                        let elapsed_ms = toast.created_at.elapsed().as_millis() as usize;
                        let frame_index = (elapsed_ms / 80) % SPINNER_FRAMES.len();
                        (
                            SPINNER_FRAMES[frame_index],
                            gpui::Rgba {
                                r: 0.53,
                                g: 0.70,
                                b: 0.92,
                                a: opacity,
                            },
                            true,
                        )
                    }
                };

                // Subtle, glassy background with animation
                let mut bg = colors.background;
                bg.a = 0.88 * opacity;
                let mut border = colors.foreground;
                border.a = 0.08 * opacity;
                let mut text = colors.foreground;
                text.a = 0.92 * opacity;

                container = container.child(
                    div()
                        .id(("toast", toast_id))
                        .w(px(320.0))
                        .mt(px(slide_offset))
                        .rounded_lg()
                        .bg(bg)
                        .border_1()
                        .border_color(border)
                        .shadow_md()
                        .overflow_hidden()
                        .child(
                            div()
                                .w_full()
                                .px(px(14.0))
                                .py(px(12.0))
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                // Icon
                                .child(div().text_size(px(14.0)).text_color(accent).child(icon))
                                // Message
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(13.0))
                                        .text_color(text)
                                        .child(toast_message.clone()),
                                )
                                .child(
                                    div()
                                        .w(px(68.0))
                                        .h(px(24.0))
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .children(is_copied.then(|| {
                                            let mut copied_bg = accent;
                                            copied_bg.a = 0.22;
                                            div()
                                                .rounded(px(6.0))
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .text_size(px(11.0))
                                                .text_color(accent)
                                                .bg(copied_bg)
                                                .child("Copied")
                                        }))
                                        .children((!is_copied && is_hovered).then(|| {
                                            let toast_message_for_copy = toast_message.clone();
                                            div()
                                                .rounded(px(6.0))
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .text_size(px(11.0))
                                                .text_color(text)
                                                .bg(border)
                                                .hover(|style| style.bg(accent))
                                                .cursor_pointer()
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |this, _event, _window, cx| {
                                                            cx.write_to_clipboard(
                                                                ClipboardItem::new_string(
                                                                    toast_message_for_copy.clone(),
                                                                ),
                                                            );
                                                            this.copied_toast_feedback =
                                                                Some((toast_id, Instant::now()));
                                                            cx.notify();
                                                            cx.spawn(
                                                                async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                                                                    smol::Timer::after(Duration::from_millis(
                                                                        TOAST_COPY_FEEDBACK_MS,
                                                                    ))
                                                                    .await;
                                                                    let _ = cx.update(|cx| {
                                                                        this.update(cx, |view, cx| {
                                                                            if view
                                                                                .copied_toast_feedback
                                                                                .is_some_and(
                                                                                    |(id, _)| {
                                                                                        id == toast_id
                                                                                    },
                                                                                )
                                                                            {
                                                                                view.copied_toast_feedback = None;
                                                                                cx.notify();
                                                                            }
                                                                        })
                                                                    });
                                                                },
                                                            )
                                                            .detach();
                                                            cx.stop_propagation();
                                                        },
                                                    ),
                                                )
                                                .child("Copy")
                                        })),
                                )
                                .on_mouse_move(cx.listener(move |this, _event, _window, cx| {
                                    if this.hovered_toast != Some(toast_id) {
                                        this.hovered_toast = Some(toast_id);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                })),
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
                            .flex_col()
                            .items_end()
                            .justify_end()
                            .pr(px(20.0))
                            .pb(px(20.0))
                            .on_mouse_move(cx.listener(|this, _event, _window, cx| {
                                if this.hovered_toast.is_some() {
                                    this.hovered_toast = None;
                                    cx.notify();
                                }
                            }))
                            .child(container),
                    )
                    .into_any(),
            )
        };
        let mut root_bg = colors.background;
        root_bg.a = self.scaled_background_alpha(root_bg.a);

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
                    .child(tabs_row),
            )
            .children(banner_element)
            .child(
                div()
                    .id("terminal")
                    .track_focus(&focus_handle)
                    .key_context(key_context)
                    .on_action(cx.listener(Self::handle_toggle_command_palette_action))
                    .on_action(cx.listener(Self::handle_import_colors_action))
                    .on_action(cx.listener(Self::handle_switch_theme_action))
                    .on_action(cx.listener(Self::handle_app_info_action))
                    .on_action(cx.listener(Self::handle_native_sdk_example_action))
                    .on_action(cx.listener(Self::handle_restart_app_action))
                    .on_action(cx.listener(Self::handle_rename_tab_action))
                    .on_action(cx.listener(Self::handle_check_for_updates_action))
                    .on_action(cx.listener(Self::handle_new_tab_action))
                    .on_action(cx.listener(Self::handle_close_tab_action))
                    .on_action(cx.listener(Self::handle_minimize_window_action))
                    .on_action(cx.listener(Self::handle_copy_action))
                    .on_action(cx.listener(Self::handle_paste_action))
                    .on_action(cx.listener(Self::handle_zoom_in_action))
                    .on_action(cx.listener(Self::handle_zoom_out_action))
                    .on_action(cx.listener(Self::handle_zoom_reset_action))
                    .on_action(cx.listener(Self::handle_quit_action))
                    .on_action(cx.listener(Self::handle_open_search_action))
                    .on_action(cx.listener(Self::handle_close_search_action))
                    .on_action(cx.listener(Self::handle_search_next_action))
                    .on_action(cx.listener(Self::handle_search_previous_action))
                    .on_action(cx.listener(Self::handle_toggle_search_case_sensitive_action))
                    .on_action(cx.listener(Self::handle_toggle_search_regex_action))
                    .on_action(cx.listener(Self::handle_inline_backspace_action))
                    .on_action(cx.listener(Self::handle_inline_delete_action))
                    .on_action(cx.listener(Self::handle_inline_move_left_action))
                    .on_action(cx.listener(Self::handle_inline_move_right_action))
                    .on_action(cx.listener(Self::handle_inline_select_left_action))
                    .on_action(cx.listener(Self::handle_inline_select_right_action))
                    .on_action(cx.listener(Self::handle_inline_select_all_action))
                    .on_action(cx.listener(Self::handle_inline_move_to_start_action))
                    .on_action(cx.listener(Self::handle_inline_move_to_end_action))
                    .on_action(cx.listener(Self::handle_inline_delete_word_backward_action))
                    .on_action(cx.listener(Self::handle_inline_delete_word_forward_action))
                    .on_action(cx.listener(Self::handle_inline_delete_to_start_action))
                    .on_action(cx.listener(Self::handle_inline_delete_to_end_action))
                    .on_key_down(cx.listener(Self::handle_key_down))
                    .on_scroll_wheel(cx.listener(Self::handle_terminal_scroll_wheel))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
                    .on_mouse_move(cx.listener(Self::handle_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
                    .on_drop(cx.listener(Self::handle_file_drop))
                    .relative()
                    .flex_1()
                    .w_full()
                    .px(px(effective_padding_x))
                    .py(px(effective_padding_y))
                    .overflow_hidden()
                    .bg(terminal_surface_bg_hsla)
                    .font_family(font_family.clone())
                    .text_size(font_size)
                    .child(terminal_grid_layer)
                    .children(terminal_scrollbar_overlay)
                    .children(command_palette_overlay)
                    .children(search_overlay),
            )
            .children(toast_overlay)
    }
}
