//! Embedded browser tabs. A browser tab hosts a native webview (wry /
//! WKWebView, macOS only for now) layered as a child view over the gpui
//! window. gpui renders the chrome (URL bar, buttons); every frame the
//! webview's bounds and visibility are synced to the tab content area, and
//! hidden whenever its tab is not the visible one (or an overlay like the
//! command palette is open, since native views always paint above gpui).

use super::*;
use gpui::prelude::FluentBuilder as _;
use std::sync::Mutex;

pub(super) const BROWSER_DEFAULT_URL: &str = "https://www.google.com/";
pub(super) const BROWSER_URL_BAR_HEIGHT: f32 = 34.0;

/// State written by wry's navigation/title callbacks and read by the view on
/// the next frame. Callbacks run on the main thread but outside the view's
/// borrow, hence the mutexes; the wakeup sender nudges a redraw.
pub(super) struct BrowserShared {
    pub(super) url: Mutex<String>,
    pub(super) title: Mutex<String>,
}

pub(super) struct BrowserTabState {
    #[cfg(target_os = "macos")]
    webview: Option<wry::WebView>,
    pub(super) shared: Arc<BrowserShared>,
    pub(super) url_input: InlineInputState,
    pub(super) editing_url: bool,
    /// Last page title pushed into the tab strip, to avoid relayout churn.
    applied_title: String,
    /// Last bounds handed to the webview (logical px, rounded).
    last_bounds: (i32, i32, i32, i32),
    visible: bool,
}

impl BrowserTabState {
    fn new(url: &str) -> Self {
        Self {
            #[cfg(target_os = "macos")]
            webview: None,
            shared: Arc::new(BrowserShared {
                url: Mutex::new(url.to_string()),
                title: Mutex::new(String::new()),
            }),
            url_input: InlineInputState::new(String::new()),
            editing_url: false,
            applied_title: String::new(),
            last_bounds: (0, 0, 0, 0),
            visible: false,
        }
    }

    pub(super) fn current_url(&self) -> String {
        self.shared
            .url
            .lock()
            .map(|url| url.clone())
            .unwrap_or_default()
    }
}

impl TerminalTab {
    pub(super) fn is_browser(&self) -> bool {
        matches!(self.kind, TabKind::Browser(_))
    }

    pub(super) fn browser_state(&self) -> Option<&BrowserTabState> {
        match &self.kind {
            TabKind::Browser(state) => Some(state),
            TabKind::Terminal => None,
        }
    }

    pub(super) fn browser_state_mut(&mut self) -> Option<&mut BrowserTabState> {
        match &mut self.kind {
            TabKind::Browser(state) => Some(state),
            TabKind::Terminal => None,
        }
    }
}

impl TerminalView {
    pub(crate) fn add_browser_tab(&mut self, cx: &mut Context<Self>) {
        if !self.browser_tabs_enabled {
            termy_toast::info("Enable Browser Tabs in Settings to use this command");
            self.notify_overlay(cx);
            return;
        }
        if cfg!(not(target_os = "macos")) {
            termy_toast::info("Browser tabs are only available on macOS for now");
            self.notify_overlay(cx);
            return;
        }
        if self.runtime_kind() != RuntimeKind::Native {
            termy_toast::info("Browser tabs are not available with the tmux runtime");
            self.notify_overlay(cx);
            return;
        }

        let tab_id = self.allocate_tab_id();
        let title = "New Tab".to_string();
        let display_width = Self::tab_display_width_for_text_px_with_max(0.0, TAB_MAX_WIDTH);
        let sticky_title_width =
            Self::tab_display_width_for_text_px_without_close_with_max(0.0, TAB_MAX_WIDTH);
        let tab = TerminalTab {
            id: tab_id,
            window_id: format!("@browser-{tab_id}"),
            window_index: 0,
            kind: TabKind::Browser(Box::new(BrowserTabState::new(BROWSER_DEFAULT_URL))),
            panes: Vec::new(),
            active_pane_id: String::new(),
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
        self.schedule_persist_native_workspace();
        self.start_new_tab_animation(tab_id, cx);
        cx.notify();
    }

    pub(super) fn active_tab_is_browser(&self) -> bool {
        self.active_tab_ref().is_some_and(TerminalTab::is_browser)
    }

    pub(super) fn browser_url_editing(&self) -> bool {
        self.active_tab_ref()
            .and_then(TerminalTab::browser_state)
            .is_some_and(|state| state.editing_url)
    }

    pub(super) fn active_browser_state(&self) -> Option<&BrowserTabState> {
        self.active_tab_ref().and_then(TerminalTab::browser_state)
    }

    fn active_browser_state_mut(&mut self) -> Option<&mut BrowserTabState> {
        self.tabs
            .get_mut(self.active_tab)
            .and_then(TerminalTab::browser_state_mut)
    }

    pub(super) fn begin_browser_url_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.active_browser_state_mut() else {
            return;
        };
        let url = state.current_url();
        state.url_input.set_text(url);
        state.url_input.select_all();
        state.editing_url = true;
        // The webview may hold first-responder status from a previous click
        // into the page; reclaim gpui keyboard focus so typing lands in the
        // address field instead of going nowhere.
        self.focus_handle.focus(window, cx);
        self.reset_cursor_blink_phase();
        self.inline_input_selecting = false;
        cx.notify();
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
        #[cfg(target_os = "macos")]
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

    pub(super) fn browser_history_back(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some(state) = self.active_browser_state()
            && let Some(webview) = &state.webview
        {
            let _ = webview.evaluate_script("history.back()");
        }
    }

    pub(super) fn browser_history_forward(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some(state) = self.active_browser_state()
            && let Some(webview) = &state.webview
        {
            let _ = webview.evaluate_script("history.forward()");
        }
    }

    pub(super) fn browser_reload(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some(state) = self.active_browser_state()
            && let Some(webview) = &state.webview
        {
            let _ = webview.reload();
        }
    }

    /// Push page titles from the webview callbacks into the tab strip.
    pub(super) fn sync_browser_tab_titles(&mut self) {
        for index in 0..self.tabs.len() {
            let next_title = {
                let Some(state) = self.tabs[index].browser_state() else {
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
            if let Some(state) = self.tabs[index].browser_state_mut() {
                state.applied_title = next_title.clone();
            }
            self.tabs[index].manual_title = Some(Self::truncate_tab_title(&next_title));
            self.refresh_tab_title(index);
        }
    }

    /// Create/position/show the active browser webview and hide every other
    /// one (inactive tabs, stashed workspaces, overlay open). Called once per
    /// render pass.
    #[cfg(target_os = "macos")]
    pub(super) fn sync_browser_webviews(&mut self, window: &Window) {
        let viewport = window.viewport_size();
        let viewport_width: f32 = viewport.width.into();
        let viewport_height: f32 = viewport.height.into();
        let host_x = self.workspace_sidebar_width();
        let host_y = self.terminal_content_top_inset() + BROWSER_URL_BAR_HEIGHT;
        let host_width = (viewport_width - host_x - self.effective_sidebar_width()).max(0.0);
        let host_height = (viewport_height - host_y - self.inspector_bottom_inset()).max(0.0);
        // Native views paint above all gpui content, so hide the webview
        // whenever a gpui overlay needs the area.
        let overlay_open = self.is_command_palette_open()
            || self.quit_prompt_in_flight
            || self.new_tab_menu_anchor.is_some();
        let active_tab = self.active_tab;
        let wakeup = self.event_wakeup_tx.clone();

        for (index, tab) in self.tabs.iter_mut().enumerate() {
            let Some(state) = tab.browser_state_mut() else {
                continue;
            };
            let show =
                index == active_tab && !overlay_open && host_width >= 1.0 && host_height >= 1.0;
            if show {
                if state.webview.is_none() {
                    state.webview = create_browser_webview(
                        window,
                        &state.shared,
                        &state.current_url(),
                        wakeup.clone(),
                    );
                }
                let Some(webview) = &state.webview else {
                    continue;
                };
                let bounds = (
                    host_x.round() as i32,
                    host_y.round() as i32,
                    host_width.round() as i32,
                    host_height.round() as i32,
                );
                if state.last_bounds != bounds {
                    let _ = webview.set_bounds(wry::Rect {
                        position: wry::dpi::LogicalPosition::new(bounds.0, bounds.1).into(),
                        size: wry::dpi::LogicalSize::new(bounds.2, bounds.3).into(),
                    });
                    state.last_bounds = bounds;
                }
                if !state.visible {
                    let _ = webview.set_visible(true);
                    state.visible = true;
                }
            } else if state.visible {
                if let Some(webview) = &state.webview {
                    let _ = webview.set_visible(false);
                }
                state.visible = false;
            }
        }
        for entry in &mut self.workspaces {
            for tab in &mut entry.tabs {
                if let Some(state) = tab.browser_state_mut()
                    && state.visible
                {
                    if let Some(webview) = &state.webview {
                        let _ = webview.set_visible(false);
                    }
                    state.visible = false;
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub(super) fn sync_browser_webviews(&mut self, _window: &Window) {}

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
        &mut self,
        colors: &TerminalColors,
        ui_font_family: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editing = self.browser_url_editing();
        let current_url = self
            .active_browser_state()
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
                    cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                        window.prevent_default();
                        this.begin_browser_url_edit(window, cx);
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
}

#[cfg(target_os = "macos")]
fn create_browser_webview(
    window: &Window,
    shared: &Arc<BrowserShared>,
    url: &str,
    wakeup: Sender<()>,
) -> Option<wry::WebView> {
    let nav_shared = shared.clone();
    let nav_wakeup = wakeup.clone();
    let title_shared = shared.clone();
    let title_wakeup = wakeup.clone();
    let load_shared = shared.clone();
    let load_wakeup = wakeup;
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

    let result = wry::WebViewBuilder::new()
        .with_url(url)
        .with_visible(false)
        .with_bounds(wry::Rect {
            position: wry::dpi::LogicalPosition::new(0, 0).into(),
            size: wry::dpi::LogicalSize::new(0, 0).into(),
        })
        .with_navigation_handler(move |url| {
            store_if_changed(&nav_shared.url, url, &nav_wakeup);
            true
        })
        .with_document_title_changed_handler(move |title| {
            store_if_changed(&title_shared.title, title, &title_wakeup);
        })
        .with_on_page_load_handler(move |_event, url| {
            store_if_changed(&load_shared.url, url, &load_wakeup);
        })
        .build_as_child(window);
    match result {
        Ok(webview) => Some(webview),
        Err(error) => {
            termy_toast::error(format!("Failed to create browser view: {error}"));
            None
        }
    }
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
    fn normalize_browser_url_searches_plain_text() {
        assert_eq!(
            TerminalView::normalize_browser_url("rust gpui webview"),
            "https://www.google.com/search?q=rust+gpui+webview"
        );
    }
}
