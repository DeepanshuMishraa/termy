use super::super::*;
use super::render_palette::TabStripPalette;
use super::state::TabStripOrientation;
use gpui::prelude::FluentBuilder as _;

impl TerminalView {
    /// Render the left workspace sidebar: a header with bell / search / new
    /// workspace actions, a divider, then one row per workspace. Shown when
    /// `sidebar_enabled` is on.
    pub(crate) fn render_workspace_sidebar(
        &mut self,
        colors: &TerminalColors,
        font_family: &SharedString,
        sidebar_bg: gpui::Rgba,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.resolve_tab_strip_palette(colors, sidebar_bg);
        // The bell / search / new-workspace actions normally live in the
        // titlebar lane above this sidebar; fall back to an in-sidebar header
        // when there is no horizontal tab strip to host them.
        let header = (self.tab_strip_orientation() == TabStripOrientation::Vertical)
            .then(|| self.render_workspace_sidebar_header(&palette, cx));
        let rows = self.render_workspace_sidebar_rows(&palette, font_family, cx);
        let sidebar_width = self.workspace_sidebar_width();
        let resize_active = self.workspace_sidebar_resize_drag_active();
        let mut resize_handle_bg = palette.tab_stroke_color;
        resize_handle_bg.a = if resize_active { 0.55 } else { 0.0 };
        let mut resize_handle_hover_bg = palette.tab_stroke_color;
        resize_handle_hover_bg.a = 0.45;

        div()
            .id("workspace-sidebar")
            .relative()
            .flex_none()
            .w(px(sidebar_width))
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(palette.tab_stroke_color)
            .children(header)
            .child(
                div()
                    .id("workspace-sidebar-rows")
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .overflow_y_scroll()
                    .child(rows),
            )
            .child(
                div()
                    .id("workspace-sidebar-resize-handle")
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .w(px(WORKSPACE_SIDEBAR_RESIZE_HANDLE_WIDTH))
                    .cursor_col_resize()
                    .bg(resize_handle_bg)
                    .hover(move |s| s.bg(resize_handle_hover_bg))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            window.prevent_default();
                            this.begin_workspace_sidebar_resize_drag(event.position.x.into());
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .into_any_element()
    }

    /// Bell / search / new-workspace actions rendered inside the tab strip's
    /// left inset lane, so they sit in the titlebar row directly above the
    /// sidebar (after the platform window controls).
    pub(super) fn render_workspace_sidebar_titlebar_actions(
        &mut self,
        palette: &TabStripPalette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let leading_inset = Self::titlebar_left_padding_for_platform();
        let sidebar_width = self.workspace_sidebar_width();
        div()
            .id("workspace-sidebar-titlebar-actions")
            .absolute()
            .left(px(leading_inset))
            .top_0()
            .bottom_0()
            .w(px((sidebar_width - leading_inset).max(0.0)))
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(4.0))
            .child(self.workspace_sidebar_icon_button(
                "workspace-sidebar-bell-button",
                "icons/sidebar/bell.svg",
                palette,
                cx,
                |_this, _cx| {
                    termy_toast::info("No notifications");
                },
            ))
            .child(self.workspace_sidebar_icon_button(
                "workspace-sidebar-search-button",
                "icons/settings/search.svg",
                palette,
                cx,
                |this, cx| this.open_search(cx),
            ))
            .child(div().flex_1())
            .child(self.workspace_sidebar_icon_button(
                "workspace-sidebar-new-workspace-button",
                "icons/tab_strip/plus.svg",
                palette,
                cx,
                |this, cx| this.add_workspace(cx),
            ))
            .into_any_element()
    }

    fn render_workspace_sidebar_header(
        &mut self,
        palette: &TabStripPalette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("workspace-sidebar-header")
            .flex_none()
            .w_full()
            .h(px(WORKSPACE_SIDEBAR_HEADER_HEIGHT))
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(WORKSPACE_SIDEBAR_PADDING_X))
            .border_b_1()
            .border_color(palette.tab_stroke_color)
            .child(self.workspace_sidebar_icon_button(
                "workspace-sidebar-bell-button",
                "icons/sidebar/bell.svg",
                palette,
                cx,
                |_this, _cx| {
                    termy_toast::info("No notifications");
                },
            ))
            .child(self.workspace_sidebar_icon_button(
                "workspace-sidebar-search-button",
                "icons/settings/search.svg",
                palette,
                cx,
                |this, cx| this.open_search(cx),
            ))
            .child(div().flex_1())
            .child(self.workspace_sidebar_icon_button(
                "workspace-sidebar-new-workspace-button",
                "icons/tab_strip/plus.svg",
                palette,
                cx,
                |this, cx| this.add_workspace(cx),
            ))
            .into_any_element()
    }

    fn render_workspace_sidebar_rows(
        &mut self,
        palette: &TabStripPalette,
        font_family: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut rows = div()
            .id("workspace-sidebar-rows-content")
            .flex()
            .flex_col()
            .w_full()
            .px(px(WORKSPACE_SIDEBAR_PADDING_X))
            .py(px(WORKSPACE_SIDEBAR_PADDING_Y))
            .gap(px(WORKSPACE_SIDEBAR_ROW_GAP));

        for index in 0..self.workspaces.len() {
            let is_active = index == self.active_workspace;
            let name = self.workspaces[index].name.clone();
            let tab_count = if is_active {
                self.tabs.len()
            } else {
                self.workspaces[index].tabs.len()
            };

            let row_bg = if is_active {
                palette.active_tab_bg
            } else {
                palette.inactive_tab_bg
            };
            let row_text = if is_active {
                palette.active_tab_text
            } else {
                palette.inactive_tab_text
            };
            let hover_bg = palette.hovered_tab_bg;
            let mut count_text = row_text;
            count_text.a = (count_text.a * 0.65).min(1.0);

            rows = rows.child(
                div()
                    .id(("workspace-sidebar-row", index))
                    .w_full()
                    .h(px(WORKSPACE_SIDEBAR_ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(8.0))
                    .rounded(px(TAB_ITEM_RADIUS))
                    .bg(row_bg)
                    .when(!is_active, |row| row.hover(move |s| s.bg(hover_bg)))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                            window.prevent_default();
                            this.switch_workspace(index, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .font_family(font_family.clone())
                            .text_size(px(TAB_TITLE_FONT_SIZE))
                            .text_color(row_text)
                            .child(name),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(font_family.clone())
                            .text_size(px(10.0))
                            .text_color(count_text)
                            .child(format!("{tab_count}")),
                    ),
            );
        }

        rows.into_any_element()
    }

    fn workspace_sidebar_icon_button(
        &self,
        id: &'static str,
        icon_path: &'static str,
        palette: &TabStripPalette,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let mut button_bg = palette.hovered_tab_bg;
        button_bg.a = 0.0;
        let mut button_hover_bg = palette.hovered_tab_bg;
        button_hover_bg.a = (button_hover_bg.a * 1.45).min(1.0);
        let mut icon_color = palette.inactive_tab_text;
        icon_color.a = icon_color.a.max(0.70);

        div()
            .id(id)
            .w(px(24.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(TAB_ITEM_RADIUS))
            .bg(button_bg)
            .text_color(icon_color)
            .hover(move |style| style.bg(button_hover_bg))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                    window.prevent_default();
                    on_click(this, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                gpui::svg()
                    .path(gpui::SharedString::from(icon_path))
                    .size(px(13.0))
                    .text_color(icon_color),
            )
            .into_any_element()
    }
}
