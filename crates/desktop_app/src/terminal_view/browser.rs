//! Embedded browser tabs. On macOS, a browser tab hosts a native wry webview
//! layered as a child view over the gpui window. gpui renders the chrome (URL
//! bar, buttons); every frame the webview's bounds and visibility are synced to
//! the tab content area, and hidden whenever its tab is not the visible one (or
//! an overlay like the command palette is open, since native views always
//! paint above gpui).

use super::*;
use gpui::prelude::FluentBuilder as _;
use std::sync::Mutex;
use termy_command_core::{browser_tabs_supported, browser_tabs_unsupported_message};

pub(super) const BROWSER_DEFAULT_URL: &str = "https://www.google.com/";

/// Browser-style user agent so sites serve their modern UI instead of the
/// unknown-engine fallback wry's bare default triggers.
#[cfg(target_os = "macos")]
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
     (KHTML, like Gecko) Version/18.4 Safari/605.1.15";
pub(super) const BROWSER_URL_BAR_HEIGHT: f32 = 34.0;

/// State written by wry's navigation/title callbacks and read by the view on
/// the next frame. Callbacks run on the main thread but outside the view's
/// borrow, hence the mutexes; the wakeup sender nudges a redraw.
pub(super) struct BrowserShared {
    pub(super) url: Mutex<String>,
    pub(super) title: Mutex<String>,
    /// Set by the page's mousedown IPC hook; drained on the next frame to end
    /// any in-progress URL edit, since clicks inside the native webview never
    /// reach gpui's mouse handlers.
    pub(super) pointer_down: std::sync::atomic::AtomicBool,
    /// URLs from `window.open`/`target="_blank"` requests, drained on the
    /// next frame into new browser tabs (the native "new window" flow is
    /// always denied in favor of Termy's own tabs).
    pub(super) pending_new_tab_urls: Mutex<Vec<String>>,
}

pub(super) struct BrowserTabState {
    webview: Option<BrowserWebview>,
    pub(super) shared: Arc<BrowserShared>,
    pub(super) url_input: InlineInputState,
    pub(super) editing_url: bool,
    /// Last page title pushed into the tab strip, to avoid relayout churn.
    applied_title: String,
    /// Last bounds handed to the webview (logical px, rounded).
    last_bounds: (i32, i32, i32, i32),
    visible: bool,
    /// Whether gpui currently owns AppKit keyboard focus for this pane. The
    /// webview steals first-responder status on any click into the page and
    /// never gives it back on its own; this flag edge-triggers a
    /// `focus_parent()` reclaim when the pane stops being the active,
    /// unobstructed browser pane.
    gpui_owns_keyboard: bool,
    webview_creation_error: Option<String>,
}

impl BrowserTabState {
    pub(super) fn new(url: &str) -> Self {
        Self {
            webview: None,
            shared: Arc::new(BrowserShared {
                url: Mutex::new(url.to_string()),
                title: Mutex::new(String::new()),
                pointer_down: std::sync::atomic::AtomicBool::new(false),
                pending_new_tab_urls: Mutex::new(Vec::new()),
            }),
            url_input: InlineInputState::new(String::new()),
            editing_url: false,
            applied_title: String::new(),
            last_bounds: (0, 0, 0, 0),
            visible: false,
            gpui_owns_keyboard: true,
            webview_creation_error: None,
        }
    }

    pub(super) fn current_url(&self) -> String {
        self.shared
            .url
            .lock()
            .map(|url| url.clone())
            .unwrap_or_default()
    }

    pub(super) fn webview_creation_error(&self) -> Option<&str> {
        self.webview_creation_error.as_deref()
    }
}

impl TerminalView {
    pub(crate) fn browser_tabs_available(&self) -> bool {
        self.browser_tabs_enabled && Self::browser_tabs_supported()
    }

    pub(crate) fn browser_tabs_supported() -> bool {
        browser_tabs_supported()
    }

    pub(crate) fn browser_tabs_unsupported_message() -> &'static str {
        browser_tabs_unsupported_message()
    }

    pub(crate) fn add_browser_tab(&mut self, cx: &mut Context<Self>) {
        self.add_browser_tab_with_url(BROWSER_DEFAULT_URL, cx);
    }

    pub(crate) fn add_browser_tab_with_url(&mut self, url: &str, cx: &mut Context<Self>) {
        if !self.browser_tabs_enabled {
            termy_toast::info("Enable Browser Tabs in Settings to use this command");
            self.notify_overlay(cx);
            return;
        }
        if !Self::browser_tabs_supported() {
            termy_toast::info(Self::browser_tabs_unsupported_message());
            self.notify_overlay(cx);
            return;
        }
        if self.runtime_kind() != RuntimeKind::Native {
            termy_toast::info("Browser tabs are not available with the tmux runtime");
            self.notify_overlay(cx);
            return;
        }

        let tab_id = self.allocate_tab_id();
        let pane_id = format!("%browser-{tab_id}");
        let title = "New Tab".to_string();
        let display_width = Self::tab_display_width_for_text_px_with_max(0.0, TAB_MAX_WIDTH);
        let sticky_title_width =
            Self::tab_display_width_for_text_px_without_close_with_max(0.0, TAB_MAX_WIDTH);
        let tab = TerminalTab {
            id: tab_id,
            window_id: format!("@browser-{tab_id}"),
            window_index: 0,
            panes: vec![TerminalPane::new_browser(
                pane_id.clone(),
                0,
                0,
                120,
                40,
                url,
            )],
            active_pane_id: pane_id,
            pinned: false,
            manual_title: Some(title.clone()),
            explicit_title: None,
            explicit_title_is_prediction: false,
            shell_title: None,
            current_command: None,
            pending_command_title: None,
            pending_command_token: 0,
            last_prompt_cwd: None,
            title,
            title_text_width: 0.0,
            sticky_title_width,
            display_width,
            running_process: false,
            command_lifecycle: CommandLifecycle::default(),
        };

        let new_tab_index = self.active_tab.saturating_add(1).min(self.tabs.len());
        self.tabs.insert(new_tab_index, tab);
        self.active_tab = new_tab_index;
        self.mark_tab_strip_layout_dirty();
        self.reset_tab_interaction_state();
        self.sync_tab_strip_for_active_tab();
        self.sync_plugin_lifecycle_state(false, cx);
        self.schedule_persist_native_workspace(cx);
        self.start_new_tab_animation(tab_id, cx);
        cx.notify();
    }

    pub(super) fn browser_url_editing(&self) -> bool {
        self.active_browser_state()
            .is_some_and(|state| state.editing_url)
    }

    pub(super) fn active_browser_state(&self) -> Option<&BrowserTabState> {
        self.active_pane_ref().and_then(TerminalPane::browser_state)
    }

    fn active_browser_state_mut(&mut self) -> Option<&mut BrowserTabState> {
        self.tabs
            .get_mut(self.active_tab)
            .and_then(|tab| {
                tab.active_pane_index()
                    .and_then(|index| tab.panes.get_mut(index))
            })
            .and_then(TerminalPane::browser_state_mut)
    }

    pub(super) fn begin_browser_url_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.active_browser_state_mut() else {
            return;
        };
        let url = state.current_url();
        state.url_input.set_text(url);
        state.url_input.select_all();
        state.editing_url = true;
        // The webview steals first-responder status on any click into the
        // page; hand it back to the gpui view at the AppKit level, otherwise
        // typing keeps flowing into the webview instead of the address field.
        if let Some(webview) = &state.webview {
            webview.focus_parent();
        }
        state.gpui_owns_keyboard = true;
        self.focus_handle.focus(window, cx);
        self.reset_cursor_blink_phase();
        self.inline_input_selecting = false;
        cx.notify();
    }

    pub(super) fn begin_browser_url_edit_for_pane(
        &mut self,
        pane_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.focus_pane_target(pane_id.as_str(), cx);
        self.begin_browser_url_edit(window, cx);
    }

    /// Drop any in-progress URL edit on the active tab without committing,
    /// e.g. when the user switches tabs mid-edit. The address bar falls back
    /// to showing the live page URL.
    pub(super) fn cancel_active_browser_url_edit_quietly(&mut self) {
        if let Some(state) = self.active_browser_state_mut() {
            state.editing_url = false;
        }
    }

    pub(super) fn cancel_browser_url_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.active_browser_state_mut() {
            if !state.editing_url {
                return;
            }
            state.editing_url = false;
            cx.notify();
        }
    }

    pub(super) fn commit_browser_url(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.active_browser_state_mut() else {
            return;
        };
        let raw = state.url_input.text().trim().to_string();
        state.editing_url = false;
        if raw.is_empty() {
            cx.notify();
            return;
        }
        let url = Self::normalize_browser_url(&raw);
        if let Ok(mut shared_url) = state.shared.url.lock() {
            *shared_url = url.clone();
        }
        if let Some(webview) = &state.webview
            && let Err(error) = webview.load_url(&url)
        {
            termy_toast::error(format!("Failed to load URL: {error}"));
        }
        cx.notify();
    }

    /// Turn address-bar text into a loadable URL: pass through full URLs,
    /// add https:// to host-like input, otherwise search for it.
    fn normalize_browser_url(input: &str) -> String {
        if input.starts_with("http://") || input.starts_with("https://") {
            return input.to_string();
        }
        let host_like = !input.contains(char::is_whitespace)
            && (input.contains('.') || input.starts_with("localhost"));
        if host_like {
            return format!("https://{input}");
        }
        format!(
            "https://www.google.com/search?q={}",
            urlencoding_encode(input)
        )
    }

    /// Route a Copy/Paste action into the active browser pane's webview.
    /// Returns false when the active pane is not a browser page (URL bar
    /// editing is handled by the inline-input path before this).
    pub(super) fn forward_edit_action_to_active_browser(&self, action: BrowserEditAction) -> bool {
        let Some(state) = self.active_browser_state() else {
            return false;
        };
        if state.editing_url {
            return false;
        }
        let Some(webview) = &state.webview else {
            return false;
        };
        webview.send_edit_action(action);
        true
    }

    pub(super) fn browser_history_back(&mut self) {
        if let Some(state) = self.active_browser_state()
            && let Some(webview) = &state.webview
        {
            webview.evaluate_script("history.back()");
        }
    }

    pub(super) fn browser_history_forward(&mut self) {
        if let Some(state) = self.active_browser_state()
            && let Some(webview) = &state.webview
        {
            webview.evaluate_script("history.forward()");
        }
    }

    pub(super) fn browser_reload(&mut self) {
        if let Some(state) = self.active_browser_state()
            && let Some(webview) = &state.webview
        {
            webview.reload();
        }
    }

    fn browser_pane_webview_layouts(&self, window: &Window) -> Vec<BrowserPaneWebviewLayout> {
        let Some(active_tab) = self.active_tab_ref() else {
            return Vec::new();
        };
        let Some(content_bounds) = self.terminal_content_bounds(window) else {
            return Vec::new();
        };
        // Pane layout coordinates are local to the terminal surface; the
        // webview is a native child of the window's content view, so shift
        // back past the left workspace sidebar and the titlebar/tab strip.
        let window_offset_x = self.workspace_sidebar_width();
        let window_offset_y = self.terminal_content_top_inset();
        active_tab
            .panes
            .iter()
            .filter(|pane| pane.is_browser())
            .filter_map(|pane| {
                let layout = self.terminal_pane_layout(active_tab, pane, content_bounds)?;
                let host_x = window_offset_x + layout.content_frame.origin_x;
                let host_y =
                    window_offset_y + layout.content_frame.origin_y + BROWSER_URL_BAR_HEIGHT;
                let host_width = layout.content_frame.width.max(0.0);
                let host_height = (layout.content_frame.height - BROWSER_URL_BAR_HEIGHT).max(0.0);
                (host_width >= 1.0 && host_height >= 1.0).then(|| BrowserPaneWebviewLayout {
                    pane_id: pane.id.clone(),
                    bounds: (
                        host_x.round() as i32,
                        host_y.round() as i32,
                        host_width.round() as i32,
                        host_height.round() as i32,
                    ),
                })
            })
            .collect()
    }

    /// Push page titles from the webview callbacks into the tab strip.
    pub(super) fn sync_browser_tab_titles(&mut self) {
        for index in 0..self.tabs.len() {
            let next_title = {
                let Some(state) = self.tabs[index]
                    .panes
                    .iter()
                    .find_map(TerminalPane::browser_state)
                else {
                    continue;
                };
                let title = state
                    .shared
                    .title
                    .lock()
                    .map(|title| title.trim().to_string())
                    .unwrap_or_default();
                if title.is_empty() || title == state.applied_title {
                    continue;
                }
                title
            };
            if let Some(state) = self.tabs[index]
                .panes
                .iter_mut()
                .find_map(TerminalPane::browser_state_mut)
            {
                state.applied_title = next_title.clone();
            }
            self.tabs[index].manual_title = Some(Self::truncate_tab_title(&next_title));
            self.refresh_tab_title(index);
        }
    }

    /// Create/position/show the active browser webview and hide every other
    /// one (inactive tabs, stashed workspaces, overlay open). Called once per
    /// render pass.
    pub(super) fn sync_browser_webviews(&mut self, window: &Window, cx: &mut Context<Self>) {
        // A click into the page (reported via the webview's IPC hook) ends any
        // in-progress URL edit, mirroring a click anywhere else in the window.
        // Queued window.open/target=_blank URLs become new tabs, deferred so
        // the tab list is not mutated mid-render.
        let mut new_tab_urls = Vec::new();
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                let Some(state) = pane.browser_state_mut() else {
                    continue;
                };
                if state
                    .shared
                    .pointer_down
                    .swap(false, std::sync::atomic::Ordering::Relaxed)
                    && state.editing_url
                {
                    state.editing_url = false;
                    cx.notify();
                }
                if let Ok(mut pending) = state.shared.pending_new_tab_urls.lock()
                    && !pending.is_empty()
                {
                    new_tab_urls.append(&mut *pending);
                }
            }
        }
        if !new_tab_urls.is_empty() {
            let view = cx.entity();
            cx.defer(move |cx| {
                view.update(cx, |this, cx| {
                    for url in new_tab_urls {
                        this.add_browser_tab_with_url(&url, cx);
                    }
                });
            });
        }
        // Native views paint above all gpui content, so hide the webview
        // whenever a gpui overlay needs the area.
        let overlay_open = self.is_command_palette_open()
            || self.plugin_ui.is_some()
            || self.quit_prompt_in_flight
            || self.new_tab_menu_anchor.is_some();
        let active_tab = self.active_tab;
        let active_pane_id = self.active_pane_id().map(str::to_string);
        let wakeup = self.event_wakeup_tx.clone();
        let browser_layouts = if overlay_open {
            Vec::new()
        } else {
            self.browser_pane_webview_layouts(window)
        };

        for (index, tab) in self.tabs.iter_mut().enumerate() {
            for pane in &mut tab.panes {
                let pane_id = pane.id.clone();
                let Some(state) = pane.browser_state_mut() else {
                    continue;
                };
                let layout = (index == active_tab)
                    .then(|| {
                        browser_layouts
                            .iter()
                            .find(|layout| layout.pane_id == pane_id)
                    })
                    .flatten();
                if let Some(layout) = layout {
                    if state.webview.is_none() && state.webview_creation_error.is_none() {
                        match create_browser_webview(
                            window,
                            &state.shared,
                            &state.current_url(),
                            wakeup.clone(),
                        ) {
                            Ok(webview) => state.webview = Some(webview),
                            Err(error) => {
                                termy_toast::error(format!(
                                    "Failed to create browser view: {error}"
                                ));
                                state.webview_creation_error = Some(error);
                                cx.notify();
                            }
                        }
                    }
                    let Some(webview) = &state.webview else {
                        continue;
                    };
                    if state.last_bounds != layout.bounds {
                        webview.set_bounds(layout.bounds);
                        state.last_bounds = layout.bounds;
                    }
                    if !state.visible {
                        webview.set_visible(true);
                        state.visible = true;
                    }
                } else if state.visible {
                    if let Some(webview) = &state.webview {
                        webview.set_visible(false);
                    }
                    state.visible = false;
                }
                // Reclaim AppKit keyboard focus the moment this pane stops
                // being the active, unobstructed browser pane (tab switch,
                // pane switch, overlay open, URL edit). A hidden or inactive
                // webview otherwise keeps first-responder status and silently
                // eats every keystroke meant for the terminal.
                let webview_owns_keyboard = index == active_tab
                    && !overlay_open
                    && !state.editing_url
                    && active_pane_id.as_deref() == Some(pane_id.as_str());
                if webview_owns_keyboard {
                    state.gpui_owns_keyboard = false;
                } else if !state.gpui_owns_keyboard {
                    if let Some(webview) = &state.webview {
                        webview.focus_parent();
                    }
                    state.gpui_owns_keyboard = true;
                }
            }
        }
        for entry in &mut self.workspaces {
            for tab in &mut entry.tabs {
                for pane in &mut tab.panes {
                    if let Some(state) = pane.browser_state_mut() {
                        if state.visible {
                            if let Some(webview) = &state.webview {
                                webview.set_visible(false);
                            }
                            state.visible = false;
                        }
                        if !state.gpui_owns_keyboard {
                            if let Some(webview) = &state.webview {
                                webview.focus_parent();
                            }
                            state.gpui_owns_keyboard = true;
                        }
                    }
                }
            }
        }
    }

    fn browser_nav_button(
        &self,
        id: &'static str,
        glyph: &'static str,
        colors: &TerminalColors,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let mut button_text = colors.foreground;
        button_text.a = 0.72;
        let mut hover_bg = colors.foreground;
        hover_bg.a = self.scaled_chrome_alpha(0.10);
        div()
            .id(id)
            .w(px(24.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(TAB_ITEM_RADIUS))
            .text_size(px(14.0))
            .text_color(button_text)
            .hover(move |style| style.bg(hover_bg))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                    window.prevent_default();
                    on_click(this, cx);
                    cx.stop_propagation();
                }),
            )
            .child(glyph)
            .into_any_element()
    }

    /// The browser tab's URL bar: back / forward / reload plus an address
    /// field that flips into an inline text input while editing.
    pub(super) fn render_browser_chrome(
        &self,
        pane_id: String,
        colors: &TerminalColors,
        ui_font_family: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editing = self.active_pane_id() == Some(pane_id.as_str()) && self.browser_url_editing();
        let current_url = self
            .tabs
            .get(self.active_tab)
            .and_then(|tab| tab.panes.iter().find(|pane| pane.id == pane_id))
            .and_then(TerminalPane::browser_state)
            .map(BrowserTabState::current_url)
            .unwrap_or_default();
        let mut stroke = colors.foreground;
        stroke.a = self.scaled_chrome_alpha(0.12);
        let mut field_bg = colors.foreground;
        field_bg.a = self.scaled_chrome_alpha(0.07);
        let mut field_focus_border = colors.cursor;
        field_focus_border.a = self.scaled_chrome_accent_alpha(0.65);
        let mut url_text = colors.foreground;
        url_text.a = 0.85;
        let mut selection: gpui::Rgba = colors.cursor;
        selection.a = 0.30;

        let address: AnyElement = if editing {
            div()
                .relative()
                .flex_1()
                .h(px(24.0))
                .child(self.render_inline_input_layer(
                    Font {
                        family: ui_font_family.clone(),
                        ..Font::default()
                    },
                    px(12.0),
                    url_text.into(),
                    selection.into(),
                    InlineInputAlignment::Left,
                    cx,
                ))
                .into_any_element()
        } else {
            div()
                .id("browser-url-display")
                .flex_1()
                .h(px(24.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .font_family(ui_font_family.clone())
                .text_size(px(12.0))
                .text_color(url_text)
                .cursor_text()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                        window.prevent_default();
                        this.begin_browser_url_edit_for_pane(pane_id.clone(), window, cx);
                        cx.stop_propagation();
                    }),
                )
                .child(current_url)
                .into_any_element()
        };

        div()
            .id("browser-url-bar")
            .w_full()
            .h(px(BROWSER_URL_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(8.0))
            .border_b_1()
            .border_color(stroke)
            .child(self.browser_nav_button(
                "browser-back-button",
                "‹",
                colors,
                cx,
                |this, _cx| {
                    this.browser_history_back();
                },
            ))
            .child(self.browser_nav_button(
                "browser-forward-button",
                "›",
                colors,
                cx,
                |this, _cx| {
                    this.browser_history_forward();
                },
            ))
            .child(self.browser_nav_button(
                "browser-reload-button",
                "⟳",
                colors,
                cx,
                |this, _cx| {
                    this.browser_reload();
                },
            ))
            .child(
                div()
                    .flex_1()
                    .h(px(24.0))
                    .rounded(px(5.0))
                    .bg(field_bg)
                    .when(editing, |field| {
                        field.border_1().border_color(field_focus_border)
                    })
                    .flex()
                    .items_center()
                    .child(address),
            )
            .into_any_element()
    }

    pub(super) fn render_browser_fallback(
        &self,
        pane_id: String,
        error: &str,
        colors: &TerminalColors,
        ui_font_family: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut border = colors.foreground;
        border.a = self.scaled_chrome_alpha(0.14);
        let mut text = colors.foreground;
        text.a = 0.82;
        let mut muted = colors.foreground;
        muted.a = 0.58;
        let mut button_bg = colors.foreground;
        button_bg.a = self.scaled_chrome_alpha(0.08);
        let mut button_hover_bg = colors.foreground;
        button_hover_bg.a = self.scaled_chrome_alpha(0.14);

        let open_pane_id = pane_id.clone();
        let retry_pane_id = pane_id;

        div()
            .id("browser-fallback")
            .absolute()
            .left_0()
            .right_0()
            .top(px(BROWSER_URL_BAR_HEIGHT))
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .px(px(24.0))
            .child(
                div()
                    .max_w(px(520.0))
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(border)
                    .font_family(ui_font_family.clone())
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text)
                            .child("Browser view unavailable"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .line_height(px(17.0))
                            .text_color(muted)
                            .child(error.to_string()),
                    )
                    .child(
                        div().mt(px(4.0)).flex().gap(px(8.0)).children([
                            div()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded(px(5.0))
                                .text_size(px(12.0))
                                .text_color(text)
                                .bg(button_bg)
                                .hover(move |style| style.bg(button_hover_bg))
                                .cursor_pointer()
                                .child("Open in Browser")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        move |this, _event: &MouseDownEvent, _window, cx| {
                                            this.open_browser_url_for_pane_externally(
                                                open_pane_id.as_str(),
                                                cx,
                                            );
                                            cx.stop_propagation();
                                        },
                                    ),
                                )
                                .into_any_element(),
                            div()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded(px(5.0))
                                .text_size(px(12.0))
                                .text_color(text)
                                .bg(button_bg)
                                .hover(move |style| style.bg(button_hover_bg))
                                .cursor_pointer()
                                .child("Retry")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        move |this, _event: &MouseDownEvent, _window, cx| {
                                            this.retry_browser_webview_for_pane(
                                                retry_pane_id.as_str(),
                                                cx,
                                            );
                                            cx.stop_propagation();
                                        },
                                    ),
                                )
                                .into_any_element(),
                        ]),
                    ),
            )
            .into_any_element()
    }

    fn open_browser_url_for_pane_externally(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        let Some(url) = self
            .tabs
            .iter()
            .flat_map(|tab| tab.panes.iter())
            .find(|pane| pane.id == pane_id)
            .and_then(TerminalPane::browser_state)
            .map(BrowserTabState::current_url)
            .filter(|url| !url.trim().is_empty())
        else {
            return;
        };

        match webbrowser::open(&url) {
            Ok(()) => termy_toast::success("Opened in default browser"),
            Err(error) => termy_toast::error(format!("Failed to open browser: {error}")),
        }
        self.notify_overlay(cx);
    }

    fn retry_browser_webview_for_pane(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                if pane.id != pane_id {
                    continue;
                }
                if let Some(state) = pane.browser_state_mut() {
                    state.webview_creation_error = None;
                    state.visible = false;
                    state.last_bounds = (0, 0, 0, 0);
                    cx.notify();
                    return;
                }
            }
        }
    }
}

struct BrowserPaneWebviewLayout {
    pane_id: String,
    bounds: (i32, i32, i32, i32),
}

/// Standard editing actions forwarded to the native webview.
#[derive(Clone, Copy)]
pub(super) enum BrowserEditAction {
    Copy,
    Paste,
    SelectAll,
}

struct BrowserWebview {
    #[cfg(target_os = "macos")]
    inner: wry::WebView,
}

impl BrowserWebview {
    fn set_bounds(&self, bounds: (i32, i32, i32, i32)) {
        #[cfg(target_os = "macos")]
        {
            let _ = self.inner.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(bounds.0, bounds.1).into(),
                size: wry::dpi::LogicalSize::new(bounds.2, bounds.3).into(),
            });
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = bounds;
        }
    }

    fn set_visible(&self, visible: bool) {
        #[cfg(target_os = "macos")]
        {
            let _ = self.inner.set_visible(visible);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = visible;
        }
    }

    fn load_url(&self, url: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            self.inner.load_url(url).map_err(|error| error.to_string())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = url;
            Err("browser webview backend is unsupported".to_string())
        }
    }

    fn evaluate_script(&self, script: &str) {
        #[cfg(target_os = "macos")]
        {
            let _ = self.inner.evaluate_script(script);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = script;
        }
    }

    fn reload(&self) {
        #[cfg(target_os = "macos")]
        {
            let _ = self.inner.reload();
        }
    }

    /// Hand AppKit first-responder status back to the hosting gpui view. The
    /// webview grabs it on any click into the page and never returns it.
    fn focus_parent(&self) {
        #[cfg(target_os = "macos")]
        {
            let _ = self.inner.focus_parent();
        }
    }

    /// Send a standard Cocoa editing action to the webview. gpui's
    /// `performKeyEquivalent:` override consumes cmd+C/V/A at the window
    /// level (dispatching Termy's own actions) before AppKit would deliver
    /// them to the webview, so the app's edit actions re-dispatch here when a
    /// browser pane is active.
    fn send_edit_action(&self, action: BrowserEditAction) {
        #[cfg(target_os = "macos")]
        {
            use cocoa::base::nil;
            use objc::{msg_send, sel, sel_impl};
            use wry::WebViewExtMacOS as _;

            let webview = self.inner.webview();
            let target = std::ptr::from_ref(&*webview)
                .cast::<objc::runtime::Object>()
                .cast_mut();
            unsafe {
                match action {
                    BrowserEditAction::Copy => {
                        let _: () = msg_send![target, copy: nil];
                    }
                    BrowserEditAction::Paste => {
                        let _: () = msg_send![target, paste: nil];
                    }
                    BrowserEditAction::SelectAll => {
                        let _: () = msg_send![target, selectAll: nil];
                    }
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = action;
        }
    }
}

/// macOS function keys (arrows, home, page up/down…) carry Apple's
/// private-use codepoints (U+F700–U+F8FF) in the NSEvent. In wry's child
/// webview those leak through as literal text insertions — pressing arrow
/// down types U+F701 into focused fields. Cancel any insertion of those
/// codepoints; keydown events still fire so page-level key handling (menu
/// navigation, games) keeps working.
#[cfg(target_os = "macos")]
const FUNCTION_KEY_INSERTION_FIX_SCRIPT: &str = "\
(function () { \
  'use strict'; \
  var PUA = /[\\uF700-\\uF8FF]/; \
  window.addEventListener('beforeinput', function (event) { \
    if (event.data && PUA.test(event.data)) { event.preventDefault(); } \
  }, true); \
  window.addEventListener('keypress', function (event) { \
    if (event.charCode >= 0xF700 && event.charCode <= 0xF8FF) { \
      event.preventDefault(); \
    } \
  }, true); \
})();";

/// WKWebView disables WebAuthn (Apple gates it behind the restricted
/// `com.apple.developer.web-browser.public-key-credential` entitlement), so
/// passkey calls would fail cryptically or hang. Leaving the interfaces in
/// place but answering "unavailable" makes sites report confusing states
/// like GitHub's "partial passkey support" banner — so remove the WebAuthn
/// surface entirely and Termy presents as a browser without passkey support;
/// sites then hide their passkey UI and offer password/OAuth flows. Any
/// call that still sneaks through gets a clean NotAllowedError, and
/// user-initiated attempts surface a toast suggesting the system browser
/// (conditional-mediation autofill probes stay silent to avoid spam).
#[cfg(target_os = "macos")]
const WEBAUTHN_FALLBACK_SCRIPT: &str = "\
(function () { \
  'use strict'; \
  function notify() { \
    try { window.ipc.postMessage('webauthn-attempt'); } catch (e) {} \
  } \
  function rejection() { \
    return Promise.reject(new DOMException( \
      'Passkeys are not supported in this browser.', 'NotAllowedError')); \
  } \
  [ \
    'PublicKeyCredential', \
    'AuthenticatorResponse', \
    'AuthenticatorAttestationResponse', \
    'AuthenticatorAssertionResponse', \
  ].forEach(function (name) { \
    try { delete window[name]; } catch (e) {} \
    if (window[name] !== undefined) { \
      try { \
        Object.defineProperty(window, name, { value: undefined }); \
      } catch (e) {} \
    } \
  }); \
  if (navigator.credentials) { \
    var originalCreate = navigator.credentials.create ? \
      navigator.credentials.create.bind(navigator.credentials) : null; \
    var originalGet = navigator.credentials.get ? \
      navigator.credentials.get.bind(navigator.credentials) : null; \
    navigator.credentials.create = function (options) { \
      if (options && options.publicKey) { notify(); return rejection(); } \
      return originalCreate ? originalCreate(options) : rejection(); \
    }; \
    navigator.credentials.get = function (options) { \
      if (options && options.publicKey) { \
        if (!options.mediation || options.mediation !== 'conditional') { notify(); } \
        return rejection(); \
      } \
      return originalGet ? originalGet(options) : rejection(); \
    }; \
  } \
})();";

/// What to do with a navigation or new-window request for a given URL.
#[cfg(any(test, target_os = "macos"))]
#[derive(Debug, PartialEq, Eq)]
enum BrowserNavigationPolicy {
    /// Web content the engine may load (also covers subframes, so `data:` and
    /// `blob:` stay allowed).
    Load,
    /// Hand off to the OS (mail, phone). Never loaded in the webview.
    OpenExternally,
    /// Blocked: `file://` would let web content read local files, and
    /// arbitrary app schemes would launch other apps without consent.
    Deny,
}

#[cfg(any(test, target_os = "macos"))]
fn browser_navigation_policy(url: &str) -> BrowserNavigationPolicy {
    let scheme = url.split(':').next().unwrap_or_default();
    if ["http", "https", "about", "blob", "data"]
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    {
        return BrowserNavigationPolicy::Load;
    }
    if ["mailto", "tel", "facetime", "sms"]
        .iter()
        .any(|external| scheme.eq_ignore_ascii_case(external))
    {
        return BrowserNavigationPolicy::OpenExternally;
    }
    BrowserNavigationPolicy::Deny
}

/// Only http(s)/about URLs belong in the address bar; `data:`/`blob:` URLs
/// can be megabytes and usually come from subframes.
#[cfg(any(test, target_os = "macos"))]
fn browser_url_display_worthy(url: &str) -> bool {
    let scheme = url.split(':').next().unwrap_or_default();
    ["http", "https", "about"]
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
}

fn create_browser_webview(
    window: &Window,
    shared: &Arc<BrowserShared>,
    url: &str,
    wakeup: Sender<()>,
) -> Result<BrowserWebview, String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, shared, url, wakeup);
        return Err("browser webview backend is unsupported".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let nav_shared = shared.clone();
        let nav_wakeup = wakeup.clone();
        let title_shared = shared.clone();
        let title_wakeup = wakeup.clone();
        let load_shared = shared.clone();
        let load_wakeup = wakeup.clone();
        let ipc_shared = shared.clone();
        let ipc_wakeup = wakeup.clone();
        let new_window_shared = shared.clone();
        let new_window_wakeup = wakeup;
        // Only nudge a redraw when a value actually changed: pages can mutate the
        // URL (history API) and title on every keystroke, and an unconditional
        // wakeup per event triggers full re-render storms that starve the
        // webview's own main-thread work — the browser feels laggy.
        fn store_if_changed(slot: &Mutex<String>, next: String, wakeup: &Sender<()>) {
            let changed = slot.lock().is_ok_and(|mut current| {
                if *current == next {
                    false
                } else {
                    *current = next;
                    true
                }
            });
            if changed {
                let _ = wakeup.try_send(());
            }
        }

        let result = with_browser_builder(move |builder| {
            builder
                .with_url(url)
                .with_visible(false)
                .with_bounds(wry::Rect {
                    position: wry::dpi::LogicalPosition::new(0, 0).into(),
                    size: wry::dpi::LogicalSize::new(0, 0).into(),
                })
                // Without a Safari-style user agent (wry's default has no
                // Version/Safari tokens) sites like Google serve their legacy
                // fallback UI for unrecognized engines.
                .with_user_agent(BROWSER_USER_AGENT)
                // Match browser defaults: swipe to go back/forward, no autoplay
                // without a user gesture, devtools via right-click → Inspect,
                // clicks land even when the window is inactive.
                .with_back_forward_navigation_gestures(true)
                .with_autoplay(false)
                .with_devtools(true)
                .with_accept_first_mouse(true)
                .with_clipboard(true)
                .with_hotkeys_zoom(true)
                .with_navigation_handler(move |url| match browser_navigation_policy(&url) {
                    BrowserNavigationPolicy::Load => {
                        if browser_url_display_worthy(&url) {
                            store_if_changed(&nav_shared.url, url, &nav_wakeup);
                        }
                        true
                    }
                    BrowserNavigationPolicy::OpenExternally => {
                        let _ = webbrowser::open(&url);
                        false
                    }
                    BrowserNavigationPolicy::Deny => false,
                })
                .with_new_window_req_handler(move |url, _features| {
                    // Never let the engine spawn its own native window; route the
                    // request into a Termy browser tab on the next frame instead.
                    match browser_navigation_policy(&url) {
                        BrowserNavigationPolicy::Load => {
                            if let Ok(mut pending) = new_window_shared.pending_new_tab_urls.lock() {
                                pending.push(url);
                            }
                            let _ = new_window_wakeup.try_send(());
                        }
                        BrowserNavigationPolicy::OpenExternally => {
                            let _ = webbrowser::open(&url);
                        }
                        BrowserNavigationPolicy::Deny => {}
                    }
                    wry::NewWindowResponse::Deny
                })
                .with_document_title_changed_handler(move |title| {
                    store_if_changed(&title_shared.title, title, &title_wakeup);
                })
                .with_on_page_load_handler(move |_event, url| {
                    if browser_url_display_worthy(&url) {
                        store_if_changed(&load_shared.url, url, &load_wakeup);
                    }
                })
                // Clicks inside the native webview never reach gpui's mouse
                // handlers; report them so the view can drop an in-progress URL
                // edit (and its focus ring) on the next frame.
                .with_initialization_script(
                    "window.addEventListener('mousedown', function () { \
                         window.ipc.postMessage('pointer-down'); \
                     }, true);",
                )
                .with_initialization_script(WEBAUTHN_FALLBACK_SCRIPT)
                .with_initialization_script(FUNCTION_KEY_INSERTION_FIX_SCRIPT)
                .with_ipc_handler(move |request| match request.body().as_str() {
                    "pointer-down" => {
                        ipc_shared
                            .pointer_down
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = ipc_wakeup.try_send(());
                    }
                    "webauthn-attempt" => {
                        termy_toast::info(
                            "Passkeys aren't supported in Termy's browser yet — \
                             use \u{201c}Open in Browser\u{201d} to sign in with one",
                        );
                        let _ = ipc_wakeup.try_send(());
                    }
                    _ => {}
                })
                .build_as_child(window)
        });

        match result {
            Ok(webview) => Ok(BrowserWebview { inner: webview }),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(target_os = "macos")]
fn with_browser_builder<R>(f: impl for<'a> FnOnce(wry::WebViewBuilder<'a>) -> R) -> R {
    f(wry::WebViewBuilder::new())
}

/// Minimal percent-encoding for search queries (we only need to make the
/// query safe inside a URL, not full RFC 3986 coverage).
fn urlencoding_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_browser_url_passes_full_urls_through() {
        assert_eq!(
            TerminalView::normalize_browser_url("https://example.com/a?b=1"),
            "https://example.com/a?b=1"
        );
        assert_eq!(
            TerminalView::normalize_browser_url("http://localhost:3777/"),
            "http://localhost:3777/"
        );
    }

    #[test]
    fn normalize_browser_url_adds_scheme_to_host_like_input() {
        assert_eq!(
            TerminalView::normalize_browser_url("example.com"),
            "https://example.com"
        );
        assert_eq!(
            TerminalView::normalize_browser_url("localhost:3777"),
            "https://localhost:3777"
        );
    }

    #[test]
    fn navigation_policy_allows_web_content() {
        for url in [
            "https://example.com/",
            "http://localhost:3777/",
            "about:blank",
            "blob:https://example.com/uuid",
            "data:text/plain;base64,aGk=",
            "HTTPS://UPPER.example/",
        ] {
            assert_eq!(
                browser_navigation_policy(url),
                BrowserNavigationPolicy::Load
            );
        }
    }

    #[test]
    fn navigation_policy_hands_communication_schemes_to_the_os() {
        for url in ["mailto:a@b.c", "tel:+4512345678", "facetime:a@b.c"] {
            assert_eq!(
                browser_navigation_policy(url),
                BrowserNavigationPolicy::OpenExternally
            );
        }
    }

    #[test]
    fn navigation_policy_blocks_local_and_unknown_schemes() {
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "vscode://open",
            "ssh://host",
            "chrome://settings",
            "",
        ] {
            assert_eq!(
                browser_navigation_policy(url),
                BrowserNavigationPolicy::Deny
            );
        }
    }

    #[test]
    fn address_bar_only_shows_http_like_urls() {
        assert!(browser_url_display_worthy("https://example.com/"));
        assert!(browser_url_display_worthy("about:blank"));
        assert!(!browser_url_display_worthy("data:text/html,hi"));
        assert!(!browser_url_display_worthy("blob:https://example.com/x"));
    }

    #[test]
    fn normalize_browser_url_searches_plain_text() {
        assert_eq!(
            TerminalView::normalize_browser_url("rust gpui webview"),
            "https://www.google.com/search?q=rust+gpui+webview"
        );
    }
}
