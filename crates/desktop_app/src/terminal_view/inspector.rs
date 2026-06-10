//! Ghostty-style developer inspector: a bottom pane (toggled with the
//! `toggle_inspector` command, default `secondary-alt-i`) showing live
//! terminal state, a keyboard event log, and render statistics.

use super::*;
use std::collections::VecDeque;

const INSPECTOR_DEFAULT_HEIGHT: f32 = 280.0;
const INSPECTOR_MIN_HEIGHT: f32 = 140.0;
const INSPECTOR_MAX_HEIGHT: f32 = 640.0;
// The terminal always keeps at least this share of the viewport height.
const INSPECTOR_MAX_VIEWPORT_RATIO: f32 = 0.7;
const INSPECTOR_RESIZE_HANDLE_HEIGHT: f32 = 6.0;
const INSPECTOR_TAB_BAR_HEIGHT: f32 = 34.0;
const INSPECTOR_KEY_LOG_CAPACITY: usize = 200;
const INSPECTOR_TEXT_SIZE: f32 = 11.5;
const INSPECTOR_ROW_HEIGHT: f32 = 22.0;
const INSPECTOR_LABEL_WIDTH: f32 = 250.0;
const INSPECTOR_TAB_RADIUS: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InspectorTab {
    Terminal,
    Keyboard,
    Render,
}

impl InspectorTab {
    const ALL: [Self; 3] = [Self::Terminal, Self::Keyboard, Self::Render];

    fn label(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Keyboard => "Keyboard",
            Self::Render => "Render",
        }
    }
}

pub(super) struct InspectorKeyLogEntry {
    seq: u64,
    key: String,
    modifiers: String,
    key_char: Option<String>,
    route: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct InspectorResizeDrag {
    start_window_y: f32,
    start_height: f32,
}

pub(super) struct InspectorState {
    pub(super) open: bool,
    pub(super) tab: InspectorTab,
    height: f32,
    resize_drag: Option<InspectorResizeDrag>,
    key_log: VecDeque<InspectorKeyLogEntry>,
    next_seq: u64,
}

impl InspectorState {
    pub(super) fn new(configured_height: f32) -> Self {
        Self {
            open: false,
            tab: InspectorTab::Terminal,
            height: Self::clamp_height(configured_height),
            resize_drag: None,
            key_log: VecDeque::new(),
            next_seq: 1,
        }
    }

    fn clamp_height(height: f32) -> f32 {
        if height.is_finite() {
            height.clamp(INSPECTOR_MIN_HEIGHT, INSPECTOR_MAX_HEIGHT)
        } else {
            INSPECTOR_DEFAULT_HEIGHT
        }
    }
}

fn keystroke_modifiers_label(modifiers: gpui::Modifiers) -> String {
    let mut label = String::new();
    if modifiers.control {
        label.push('⌃');
    }
    if modifiers.alt {
        label.push('⌥');
    }
    if modifiers.shift {
        label.push('⇧');
    }
    if modifiers.platform {
        label.push('⌘');
    }
    if modifiers.function {
        label.push_str("fn");
    }
    if label.is_empty() {
        label.push('-');
    }
    label
}

struct InspectorPaneSnapshot {
    id: String,
    is_active: bool,
    cols: u16,
    rows: u16,
    cell_geometry: (u16, u16, u16, u16),
    display_offset: usize,
    history_size: usize,
    alternate_screen: bool,
    zoom_steps: i16,
    degraded: bool,
    progress: ProgressState,
}

impl TerminalView {
    pub(super) fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        self.inspector.open = !self.inspector.open;
        cx.notify();
    }

    /// Height reserved at the bottom of the terminal area while the inspector
    /// is open; feeds PTY grid sizing and pane content bounds.
    pub(super) fn inspector_bottom_inset(&self) -> f32 {
        if !self.inspector.open {
            return 0.0;
        }
        let viewport_cap = self
            .last_viewport_size_px
            .map_or(INSPECTOR_MAX_HEIGHT, |(_, viewport_height)| {
                viewport_height as f32 * INSPECTOR_MAX_VIEWPORT_RATIO
            });
        self.inspector
            .height
            .min(viewport_cap)
            .max(INSPECTOR_MIN_HEIGHT)
    }

    pub(super) fn inspector_resize_drag_active(&self) -> bool {
        self.inspector.open && self.inspector.resize_drag.is_some()
    }

    pub(super) fn update_inspector_resize_drag(&mut self, window_y: f32) -> bool {
        let Some(drag) = self.inspector.resize_drag else {
            return false;
        };
        // Dragging the handle upward grows the panel.
        let next_height =
            InspectorState::clamp_height(drag.start_height + (drag.start_window_y - window_y));
        if (next_height - self.inspector.height).abs() < f32::EPSILON {
            return false;
        }
        self.inspector.height = next_height;
        true
    }

    pub(super) fn finish_inspector_resize_drag(&mut self) -> bool {
        let Some(drag) = self.inspector.resize_drag.take() else {
            return false;
        };
        // Remember the final height across restarts. Skip the write when the
        // drag ended where it started.
        if (self.inspector.height - drag.start_height).abs() >= 1.0
            && let Err(error) = crate::config::set_root_setting(
                termy_config_core::RootSettingId::InspectorHeight,
                &format!("{:.0}", self.inspector.height),
            )
        {
            log::error!("Failed to persist inspector height: {error}");
        }
        true
    }

    /// Adopt an externally edited config value unless a resize drag owns the
    /// height right now.
    pub(super) fn sync_inspector_height_from_config(&mut self, configured_height: f32) -> bool {
        if self.inspector.resize_drag.is_some() {
            return false;
        }
        let next = InspectorState::clamp_height(configured_height);
        if (next - self.inspector.height).abs() < f32::EPSILON {
            return false;
        }
        self.inspector.height = next;
        true
    }

    pub(super) fn inspector_collects_render_stats(&self) -> bool {
        self.inspector.open && self.inspector.tab == InspectorTab::Render
    }

    pub(super) fn record_inspector_key_event(
        &mut self,
        event: &gpui::KeyDownEvent,
        route: &'static str,
        cx: &mut Context<Self>,
    ) {
        if !self.inspector.open {
            return;
        }
        let seq = self.inspector.next_seq;
        self.inspector.next_seq += 1;
        self.inspector.key_log.push_front(InspectorKeyLogEntry {
            seq,
            key: event.keystroke.key.clone(),
            modifiers: keystroke_modifiers_label(event.keystroke.modifiers),
            key_char: event.keystroke.key_char.clone(),
            route,
        });
        self.inspector.key_log.truncate(INSPECTOR_KEY_LOG_CAPACITY);
        if self.inspector.tab == InspectorTab::Keyboard {
            cx.notify();
        }
    }

    fn inspector_pane_snapshots(&self) -> Vec<InspectorPaneSnapshot> {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return Vec::new();
        };
        let active_pane_id = tab.active_pane_id.as_str();
        tab.panes
            .iter()
            .map(|pane| {
                let size = pane.terminal.size();
                let (display_offset, history_size) = pane.terminal.scroll_state();
                InspectorPaneSnapshot {
                    id: pane.id.clone(),
                    is_active: pane.id == active_pane_id,
                    cols: size.cols,
                    rows: size.rows,
                    cell_geometry: (pane.left, pane.top, pane.width, pane.height),
                    display_offset,
                    history_size,
                    alternate_screen: pane.terminal.alternate_screen_mode(),
                    zoom_steps: pane.pane_zoom_steps,
                    degraded: pane.degraded,
                    progress: pane.progress_state,
                }
            })
            .collect()
    }

    fn inspector_row(
        label: &'static str,
        value: String,
        label_color: gpui::Rgba,
        value_color: gpui::Rgba,
    ) -> AnyElement {
        div()
            .flex_none()
            .h(px(INSPECTOR_ROW_HEIGHT))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .flex_none()
                    .w(px(INSPECTOR_LABEL_WIDTH))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .truncate()
                    .text_color(label_color)
                    .child(label),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .truncate()
                    .text_color(value_color)
                    .child(value),
            )
            .into_any_element()
    }

    fn render_inspector_terminal_tab(
        &self,
        text_primary: gpui::Rgba,
        text_muted: gpui::Rgba,
        accent: gpui::Rgba,
    ) -> AnyElement {
        let runtime_label = match self.runtime_kind() {
            RuntimeKind::Native => "native",
            RuntimeKind::Tmux => "tmux",
        };
        let tab_summary = self.tabs.get(self.active_tab).map(|tab| {
            (
                tab.title.clone(),
                tab.window_id.clone(),
                tab.panes.len(),
                tab.aggregate_progress_state(),
            )
        });
        let panes = self.inspector_pane_snapshots();
        let font_size: f32 = self.font_size.into();

        let mut content = div().flex().flex_col();
        content = content.child(Self::inspector_row(
            "Runtime",
            runtime_label.to_string(),
            text_muted,
            text_primary,
        ));
        content = content.child(Self::inspector_row(
            "Font Size",
            format!("{font_size:.1}px"),
            text_muted,
            text_primary,
        ));
        content = content.child(Self::inspector_row(
            "Shell Integration",
            if self.shell_integration_enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
            text_muted,
            text_primary,
        ));
        if let Some((title, window_id, pane_count, progress)) = tab_summary {
            content = content.child(Self::inspector_row(
                "Active Tab",
                format!("{title}  ({window_id}, {pane_count} pane(s))"),
                text_muted,
                text_primary,
            ));
            content = content.child(Self::inspector_row(
                "Tab Progress",
                format!("{progress:?}"),
                text_muted,
                text_primary,
            ));
        }

        for pane in panes {
            let header_color = if pane.is_active { accent } else { text_muted };
            let header = format!(
                "Pane {}{}",
                pane.id,
                if pane.is_active { "  (active)" } else { "" }
            );
            content = content.child(
                div()
                    .flex_none()
                    .h(px(INSPECTOR_ROW_HEIGHT))
                    .mt(px(8.0))
                    .flex()
                    .items_center()
                    .text_color(header_color)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(header),
            );
            let (left, top, width, height) = pane.cell_geometry;
            content = content
                .child(Self::inspector_row(
                    "Grid",
                    format!("{} × {} cells", pane.cols, pane.rows),
                    text_muted,
                    text_primary,
                ))
                .child(Self::inspector_row(
                    "Cell Rect",
                    format!("x {left}  y {top}  w {width}  h {height}"),
                    text_muted,
                    text_primary,
                ))
                .child(Self::inspector_row(
                    "Scroll",
                    format!(
                        "offset {} / history {}",
                        pane.display_offset, pane.history_size
                    ),
                    text_muted,
                    text_primary,
                ))
                .child(Self::inspector_row(
                    "Screen",
                    if pane.alternate_screen {
                        "alternate".to_string()
                    } else {
                        "primary".to_string()
                    },
                    text_muted,
                    text_primary,
                ))
                .child(Self::inspector_row(
                    "Zoom / Degraded",
                    format!("{} steps / {}", pane.zoom_steps, pane.degraded),
                    text_muted,
                    text_primary,
                ))
                .child(Self::inspector_row(
                    "Progress",
                    format!("{:?}", pane.progress),
                    text_muted,
                    text_primary,
                ));
        }

        content.into_any_element()
    }

    fn render_inspector_keyboard_tab(
        &self,
        text_primary: gpui::Rgba,
        text_muted: gpui::Rgba,
    ) -> AnyElement {
        let header = div()
            .flex_none()
            .h(px(INSPECTOR_ROW_HEIGHT))
            .flex()
            .items_center()
            .gap(px(12.0))
            .text_color(text_muted)
            .font_weight(FontWeight::SEMIBOLD)
            .child(div().flex_none().w(px(48.0)).child("#"))
            .child(div().flex_none().w(px(140.0)).child("Key"))
            .child(div().flex_none().w(px(80.0)).child("Mods"))
            .child(div().flex_none().w(px(80.0)).child("Char"))
            .child(div().flex_1().child("Routed To"));

        let mut list = div().flex().flex_col().child(header);
        if self.inspector.key_log.is_empty() {
            list = list.child(
                div()
                    .flex_none()
                    .h(px(INSPECTOR_ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .text_color(text_muted)
                    .child("Press keys to record events…"),
            );
        }
        for entry in &self.inspector.key_log {
            list = list.child(
                div()
                    .flex_none()
                    .h(px(INSPECTOR_ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .text_color(text_primary)
                    .child(
                        div()
                            .flex_none()
                            .w(px(48.0))
                            .text_color(text_muted)
                            .child(entry.seq.to_string()),
                    )
                    .child(div().flex_none().w(px(140.0)).child(entry.key.clone()))
                    .child(div().flex_none().w(px(80.0)).child(entry.modifiers.clone()))
                    .child(
                        div()
                            .flex_none()
                            .w(px(80.0))
                            .text_color(text_muted)
                            .child(entry.key_char.clone().unwrap_or_else(|| "-".to_string())),
                    )
                    .child(div().flex_1().text_color(text_muted).child(entry.route)),
            );
        }
        list.into_any_element()
    }

    fn render_inspector_render_tab(
        &self,
        text_primary: gpui::Rgba,
        text_muted: gpui::Rgba,
    ) -> AnyElement {
        let stats = &self.debug_overlay_stats;
        let rows: Vec<(&'static str, String)> = vec![
            ("FPS", format!("{:.1}", stats.fps)),
            (
                "Frame p50 / p95 / p99",
                format!(
                    "{:.2} ms / {:.2} ms / {:.2} ms",
                    stats.frame_p50_ms, stats.frame_p95_ms, stats.frame_p99_ms
                ),
            ),
            ("CPU", format!("{:.1}%", stats.cpu_percent)),
            ("Memory", self.debug_overlay_memory_label()),
            ("View Wake Signals", stats.view_wake_signals.to_string()),
            (
                "Event Drain Passes",
                stats.terminal_event_drain_passes.to_string(),
            ),
            ("Terminal Redraws", stats.terminal_redraws.to_string()),
            (
                "Alt-Screen Fallback Redraws",
                stats.alt_screen_fallback_redraws.to_string(),
            ),
            (
                "Spans (damage/rebuild/shape/paint)",
                format!(
                    "{:.2} / {:.2} / {:.2} / {:.2} ms",
                    stats.span_damage_ms,
                    stats.span_rebuild_ms,
                    stats.span_shaping_ms,
                    stats.span_paint_ms
                ),
            ),
        ];

        let mut content = div().flex().flex_col();
        for (label, value) in rows {
            content = content.child(Self::inspector_row(label, value, text_muted, text_primary));
        }
        content = content.child(
            div()
                .flex_none()
                .mt(px(8.0))
                .text_color(text_muted)
                .child("Sampled while this tab is visible; trends update on new frames."),
        );
        content.into_any_element()
    }

    pub(super) fn render_inspector_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.inspector.open {
            return None;
        }

        let colors = self.colors.clone();
        let mut text_primary = colors.foreground;
        text_primary.a = 0.92;
        let mut text_muted = colors.foreground;
        text_muted.a = 0.58;
        let accent = colors.cursor;
        let stroke = resolve_chrome_stroke_color(
            colors.background,
            colors.foreground,
            self.chrome_contrast_profile().stroke_mix,
        );
        let mut panel_bg = colors.foreground;
        panel_bg.a = self.scaled_chrome_surface_alpha(0.025);
        let mut active_tab_bg = colors.foreground;
        active_tab_bg.a = self.scaled_chrome_surface_alpha(0.11);
        let mut hover_tab_bg = colors.foreground;
        hover_tab_bg.a = self.scaled_chrome_surface_alpha(0.05);
        let mut indicator = colors.cursor;
        indicator.a = self.scaled_chrome_accent_alpha(0.90);

        let active_tab = self.inspector.tab;
        let mut tab_bar = div()
            .flex_none()
            .h(px(INSPECTOR_TAB_BAR_HEIGHT))
            .px(px(8.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .border_b_1()
            .border_color(stroke);

        for tab in InspectorTab::ALL {
            let is_active = tab == active_tab;
            let chip_text = if is_active { text_primary } else { text_muted };
            tab_bar = tab_bar.child(
                div()
                    .id(SharedString::from(format!("inspector-tab-{}", tab.label())))
                    .relative()
                    .h(px(24.0))
                    .px(px(10.0))
                    .rounded(px(INSPECTOR_TAB_RADIUS))
                    .flex()
                    .items_center()
                    .bg(if is_active {
                        active_tab_bg
                    } else {
                        gpui::transparent_black().into()
                    })
                    .hover(move |s| s.bg(hover_tab_bg))
                    .cursor_pointer()
                    .text_size(px(INSPECTOR_TEXT_SIZE))
                    .text_color(chip_text)
                    .children(is_active.then(|| {
                        div()
                            .absolute()
                            .left_0()
                            .top(px(6.0))
                            .w(px(2.0))
                            .h(px(12.0))
                            .rounded_full()
                            .bg(indicator)
                    }))
                    .child(tab.label())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event: &MouseDownEvent, _window, cx| {
                            cx.stop_propagation();
                            if this.inspector.tab != tab {
                                this.inspector.tab = tab;
                                cx.notify();
                            }
                        }),
                    ),
            );
        }

        tab_bar = tab_bar.child(div().flex_1());

        if active_tab == InspectorTab::Keyboard {
            tab_bar = tab_bar.child(
                div()
                    .id("inspector-clear-key-log")
                    .h(px(22.0))
                    .px(px(8.0))
                    .rounded(px(INSPECTOR_TAB_RADIUS))
                    .flex()
                    .items_center()
                    .text_size(px(INSPECTOR_TEXT_SIZE))
                    .text_color(text_muted)
                    .hover(move |s| s.bg(hover_tab_bg).text_color(text_primary))
                    .cursor_pointer()
                    .child("Clear")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _event: &MouseDownEvent, _window, cx| {
                            cx.stop_propagation();
                            this.inspector.key_log.clear();
                            cx.notify();
                        }),
                    ),
            );
        }

        tab_bar = tab_bar.child(
            div()
                .id("inspector-close")
                .h(px(22.0))
                .w(px(22.0))
                .rounded(px(INSPECTOR_TAB_RADIUS))
                .flex()
                .items_center()
                .justify_center()
                .text_color(text_muted)
                .hover(move |s| s.bg(hover_tab_bg).text_color(text_primary))
                .cursor_pointer()
                .child(
                    gpui::svg()
                        .path(gpui::SharedString::from("icons/tab_strip/x.svg"))
                        .size(px(10.0))
                        .text_color(text_muted),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _event: &MouseDownEvent, _window, cx| {
                        cx.stop_propagation();
                        this.toggle_inspector(cx);
                    }),
                ),
        );

        let body = match active_tab {
            InspectorTab::Terminal => {
                self.render_inspector_terminal_tab(text_primary, text_muted, accent)
            }
            InspectorTab::Keyboard => self.render_inspector_keyboard_tab(text_primary, text_muted),
            InspectorTab::Render => self.render_inspector_render_tab(text_primary, text_muted),
        };

        let resize_handle = div()
            .id("inspector-resize-handle")
            .absolute()
            .top(px(-(INSPECTOR_RESIZE_HANDLE_HEIGHT * 0.5)))
            .left_0()
            .right_0()
            .h(px(INSPECTOR_RESIZE_HANDLE_HEIGHT))
            .cursor_row_resize()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    cx.stop_propagation();
                    this.inspector.resize_drag = Some(InspectorResizeDrag {
                        start_window_y: event.position.y.into(),
                        start_height: this.inspector_bottom_inset(),
                    });
                    cx.notify();
                }),
            );

        Some(
            div()
                .id("terminal-inspector")
                .relative()
                .flex_none()
                .w_full()
                .h(px(self.inspector_bottom_inset()))
                .flex()
                .flex_col()
                .bg(panel_bg)
                .border_t_1()
                .border_color(stroke)
                .font_family(self.ui_font_family.clone())
                .text_size(px(INSPECTOR_TEXT_SIZE))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_this, _event: &MouseDownEvent, _window, cx| {
                        cx.stop_propagation();
                    }),
                )
                .child(tab_bar)
                .child(
                    div()
                        .id("inspector-body")
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .px(px(12.0))
                        .py(px(8.0))
                        .child(body),
                )
                .child(resize_handle)
                .into_any_element(),
        )
    }
}
