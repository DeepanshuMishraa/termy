use crate::colors::TerminalColors;
use crate::config::{set_config_value, AppConfig, CursorStyle, TabTitleMode};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgb, rgba, prelude::FluentBuilder,
};

const SIDEBAR_WIDTH: f32 = 220.0;
const SIDEBAR_BG: u32 = 0x12121a;
const CONTENT_BG: u32 = 0x1a1a24;
const ITEM_HOVER_BG: u32 = 0x2a2a3a;
const ITEM_ACTIVE_BG: u32 = 0x3d3d52;
const TEXT_PRIMARY: u32 = 0xf0f0f5;
const TEXT_SECONDARY: u32 = 0xb0b0c0;
const TEXT_MUTED: u32 = 0x707080;
const SWITCH_ON_BG: u32 = 0x6366f1;
const SWITCH_OFF_BG: u32 = 0x3a3a4a;
const SWITCH_KNOB: u32 = 0xffffff;
const BORDER_COLOR: u32 = 0x2a2a3a;
const ACCENT: u32 = 0x6366f1;
const CARD_BG: u32 = 0x15151d;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SettingsSection {
    Appearance,
    Terminal,
    Tabs,
    Advanced,
}

pub struct SettingsWindow {
    active_section: SettingsSection,
    config: AppConfig,
    #[allow(dead_code)]
    colors: TerminalColors,
}

impl SettingsWindow {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let config = AppConfig::load_or_create();
        let colors = TerminalColors::from_theme(&config.theme, &config.colors);
        Self {
            active_section: SettingsSection::Appearance,
            config,
            colors,
        }
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(rgb(SIDEBAR_BG))
            .border_r_1()
            .border_color(rgb(BORDER_COLOR))
            .flex()
            .flex_col()
            .child(
                div()
                    .px_5()
                    .pt_6()
                    .pb_4()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT_MUTED))
                            .child("SETTINGS"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .px_3()
                    .child(self.render_sidebar_item("Appearance", SettingsSection::Appearance, cx))
                    .child(self.render_sidebar_item("Terminal", SettingsSection::Terminal, cx))
                    .child(self.render_sidebar_item("Tabs", SettingsSection::Tabs, cx))
                    .child(self.render_sidebar_item("Advanced", SettingsSection::Advanced, cx)),
            )
    }

    fn render_sidebar_item(
        &self,
        label: &'static str,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.active_section == section;

        div()
            .id(SharedString::from(label))
            .px_3()
            .py(px(10.0))
            .rounded_lg()
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_3()
            .bg(if is_active { rgb(ITEM_ACTIVE_BG) } else { rgba(0x00000000) })
            .hover(|s| s.bg(rgb(ITEM_HOVER_BG)))
            .child(
                div()
                    .text_sm()
                    .font_weight(if is_active { gpui::FontWeight::MEDIUM } else { gpui::FontWeight::NORMAL })
                    .text_color(if is_active { rgb(TEXT_PRIMARY) } else { rgb(TEXT_SECONDARY) })
                    .child(label),
            )
            .when(is_active, |s| {
                s.child(
                    div()
                        .ml_auto()
                        .w(px(3.0))
                        .h(px(16.0))
                        .rounded(px(2.0))
                        .bg(rgb(ACCENT)),
                )
            })
            .on_click(cx.listener(move |view, _, _, cx| {
                view.active_section = section;
                cx.notify();
            }))
    }

    fn render_content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .child(match self.active_section {
                SettingsSection::Appearance => self.render_appearance_section(cx).into_any_element(),
                SettingsSection::Terminal => self.render_terminal_section(cx).into_any_element(),
                SettingsSection::Tabs => self.render_tabs_section(cx).into_any_element(),
                SettingsSection::Advanced => self.render_advanced_section(cx).into_any_element(),
            })
            .into_any_element()
    }

    fn render_section_header(&self, title: &'static str, subtitle: &'static str) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .mb_6()
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(TEXT_PRIMARY))
                    .child(title),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(TEXT_MUTED))
                    .child(subtitle),
            )
    }

    fn render_group_header(&self, title: &'static str) -> impl IntoElement {
        div()
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(rgb(TEXT_MUTED))
            .mt_4()
            .mb_2()
            .child(title)
    }

    fn render_setting_row(
        &self,
        id: &'static str,
        title: &'static str,
        description: &'static str,
        checked: bool,
        cx: &mut Context<Self>,
        on_toggle: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py_3()
            .px_4()
            .rounded_lg()
            .bg(rgb(CARD_BG))
            .border_1()
            .border_color(rgb(BORDER_COLOR))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(TEXT_PRIMARY))
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(TEXT_MUTED))
                            .child(description),
                    ),
            )
            .child(self.render_switch(id, checked, cx, on_toggle))
    }

    fn render_switch(
        &self,
        id: &'static str,
        checked: bool,
        cx: &mut Context<Self>,
        on_toggle: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(id))
            .w(px(44.0))
            .h(px(24.0))
            .rounded(px(12.0))
            .bg(if checked { rgb(SWITCH_ON_BG) } else { rgb(SWITCH_OFF_BG) })
            .cursor_pointer()
            .relative()
            .child(
                div()
                    .absolute()
                    .top(px(2.0))
                    .left(if checked { px(22.0) } else { px(2.0) })
                    .w(px(20.0))
                    .h(px(20.0))
                    .rounded_full()
                    .bg(rgb(SWITCH_KNOB))
                    .shadow_sm(),
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                on_toggle(view, cx);
                cx.notify();
            }))
    }

    fn render_info_row(&self, title: &'static str, value: String) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py_3()
            .px_4()
            .rounded_lg()
            .bg(rgb(CARD_BG))
            .border_1()
            .border_color(rgb(BORDER_COLOR))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(TEXT_PRIMARY))
                    .child(title),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(TEXT_MUTED))
                    .child(value),
            )
    }

    fn render_info_row_with_desc(
        &self,
        title: &'static str,
        description: &'static str,
        value: String,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py_3()
            .px_4()
            .rounded_lg()
            .bg(rgb(CARD_BG))
            .border_1()
            .border_color(rgb(BORDER_COLOR))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(TEXT_PRIMARY))
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(TEXT_MUTED))
                            .child(description),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(TEXT_SECONDARY))
                    .child(value),
            )
    }

    fn render_cursor_style_row(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.config.cursor_style;

        div()
            .flex()
            .items_center()
            .justify_between()
            .py_3()
            .px_4()
            .rounded_lg()
            .bg(rgb(CARD_BG))
            .border_1()
            .border_color(rgb(BORDER_COLOR))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(TEXT_PRIMARY))
                            .child("Cursor Style"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(TEXT_MUTED))
                            .child("Shape of the terminal cursor"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child({
                        let is_selected = current == CursorStyle::Block;
                        div()
                            .id("cursor-style-block")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .bg(if is_selected { rgb(ACCENT) } else { rgb(SWITCH_OFF_BG) })
                            .text_color(if is_selected { rgb(0xffffff) } else { rgb(TEXT_SECONDARY) })
                            .hover(|s| if !is_selected { s.bg(rgb(ITEM_HOVER_BG)) } else { s })
                            .child("Block")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.config.cursor_style = CursorStyle::Block;
                                let _ = set_config_value("cursor_style", "block");
                                cx.notify();
                            }))
                    })
                    .child({
                        let is_selected = current == CursorStyle::Line;
                        div()
                            .id("cursor-style-line")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .bg(if is_selected { rgb(ACCENT) } else { rgb(SWITCH_OFF_BG) })
                            .text_color(if is_selected { rgb(0xffffff) } else { rgb(TEXT_SECONDARY) })
                            .hover(|s| if !is_selected { s.bg(rgb(ITEM_HOVER_BG)) } else { s })
                            .child("Line")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.config.cursor_style = CursorStyle::Line;
                                let _ = set_config_value("cursor_style", "line");
                                cx.notify();
                            }))
                    }),
            )
    }

    fn render_tab_title_mode_row(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.config.tab_title.mode;

        div()
            .flex()
            .items_center()
            .justify_between()
            .py_3()
            .px_4()
            .rounded_lg()
            .bg(rgb(CARD_BG))
            .border_1()
            .border_color(rgb(BORDER_COLOR))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(TEXT_PRIMARY))
                            .child("Title Mode"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(TEXT_MUTED))
                            .child("How tab titles are determined"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child({
                        let is_selected = current == TabTitleMode::Smart;
                        div()
                            .id("tab-mode-smart")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .bg(if is_selected { rgb(ACCENT) } else { rgb(SWITCH_OFF_BG) })
                            .text_color(if is_selected { rgb(0xffffff) } else { rgb(TEXT_SECONDARY) })
                            .hover(|s| if !is_selected { s.bg(rgb(ITEM_HOVER_BG)) } else { s })
                            .child("Smart")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.config.tab_title.mode = TabTitleMode::Smart;
                                let _ = set_config_value("tab_title_mode", "smart");
                                cx.notify();
                            }))
                    })
                    .child({
                        let is_selected = current == TabTitleMode::Shell;
                        div()
                            .id("tab-mode-shell")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .bg(if is_selected { rgb(ACCENT) } else { rgb(SWITCH_OFF_BG) })
                            .text_color(if is_selected { rgb(0xffffff) } else { rgb(TEXT_SECONDARY) })
                            .hover(|s| if !is_selected { s.bg(rgb(ITEM_HOVER_BG)) } else { s })
                            .child("Shell")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.config.tab_title.mode = TabTitleMode::Shell;
                                let _ = set_config_value("tab_title_mode", "shell");
                                cx.notify();
                            }))
                    })
                    .child({
                        let is_selected = current == TabTitleMode::Explicit;
                        div()
                            .id("tab-mode-explicit")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .bg(if is_selected { rgb(ACCENT) } else { rgb(SWITCH_OFF_BG) })
                            .text_color(if is_selected { rgb(0xffffff) } else { rgb(TEXT_SECONDARY) })
                            .hover(|s| if !is_selected { s.bg(rgb(ITEM_HOVER_BG)) } else { s })
                            .child("Explicit")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.config.tab_title.mode = TabTitleMode::Explicit;
                                let _ = set_config_value("tab_title_mode", "explicit");
                                cx.notify();
                            }))
                    })
                    .child({
                        let is_selected = current == TabTitleMode::Static;
                        div()
                            .id("tab-mode-static")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .bg(if is_selected { rgb(ACCENT) } else { rgb(SWITCH_OFF_BG) })
                            .text_color(if is_selected { rgb(0xffffff) } else { rgb(TEXT_SECONDARY) })
                            .hover(|s| if !is_selected { s.bg(rgb(ITEM_HOVER_BG)) } else { s })
                            .child("Static")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.config.tab_title.mode = TabTitleMode::Static;
                                let _ = set_config_value("tab_title_mode", "static");
                                cx.notify();
                            }))
                    }),
            )
    }

    fn render_appearance_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let background_blur = self.config.background_blur;
        let background_opacity = self.config.background_opacity;
        let theme = self.config.theme.clone();
        let font_family = self.config.font_family.clone();
        let font_size = self.config.font_size;
        let padding_x = self.config.padding_x;
        let padding_y = self.config.padding_y;

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.render_section_header("Appearance", "Customize the look and feel"))
            .child(self.render_group_header("THEME"))
            .child(self.render_info_row("Theme", theme))
            .child(self.render_group_header("WINDOW"))
            .child(self.render_setting_row(
                "blur-toggle",
                "Background Blur",
                "Enable blur effect for transparent backgrounds",
                background_blur,
                cx,
                |view, _cx| {
                    view.config.background_blur = !view.config.background_blur;
                    let _ = set_config_value("background_blur", &view.config.background_blur.to_string());
                },
            ))
            .child(self.render_info_row_with_desc(
                "Background Opacity",
                "Window transparency (0-100%)",
                format!("{}%", (background_opacity * 100.0) as i32),
            ))
            .child(self.render_group_header("FONT"))
            .child(self.render_info_row("Font Family", font_family))
            .child(self.render_info_row("Font Size", format!("{}px", font_size as i32)))
            .child(self.render_group_header("PADDING"))
            .child(self.render_info_row("Horizontal Padding", format!("{}px", padding_x as i32)))
            .child(self.render_info_row("Vertical Padding", format!("{}px", padding_y as i32)))
    }

    fn render_terminal_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let cursor_blink = self.config.cursor_blink;
        let term = self.config.term.clone();
        let shell = self.config.shell.clone().unwrap_or_else(|| "System default".to_string());
        let colorterm = self.config.colorterm.clone().unwrap_or_else(|| "Disabled".to_string());
        let scrollback = self.config.scrollback_history;
        let scroll_mult = self.config.mouse_scroll_multiplier;
        let command_palette_show_keybinds = self.config.command_palette_show_keybinds;

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.render_section_header("Terminal", "Configure terminal behavior"))
            .child(self.render_group_header("CURSOR"))
            .child(self.render_setting_row(
                "cursor-blink-toggle",
                "Cursor Blink",
                "Enable blinking cursor animation",
                cursor_blink,
                cx,
                |view, _cx| {
                    view.config.cursor_blink = !view.config.cursor_blink;
                    let _ = set_config_value("cursor_blink", &view.config.cursor_blink.to_string());
                },
            ))
            .child(self.render_cursor_style_row(cx))
            .child(self.render_group_header("SHELL"))
            .child(self.render_info_row_with_desc("Shell", "Executable for new sessions", shell))
            .child(self.render_info_row_with_desc("TERM", "Terminal type for child apps", term))
            .child(self.render_info_row_with_desc("COLORTERM", "Color support advertisement", colorterm))
            .child(self.render_group_header("SCROLLING"))
            .child(self.render_info_row_with_desc(
                "Scrollback History",
                "Lines to keep in buffer",
                format!("{} lines", scrollback),
            ))
            .child(self.render_info_row_with_desc(
                "Scroll Multiplier",
                "Mouse wheel scroll speed",
                format!("{}x", scroll_mult),
            ))
            .child(self.render_group_header("UI"))
            .child(self.render_setting_row(
                "palette-keybinds-toggle",
                "Show Keybindings in Palette",
                "Display keyboard shortcuts in command palette",
                command_palette_show_keybinds,
                cx,
                |view, _cx| {
                    view.config.command_palette_show_keybinds = !view.config.command_palette_show_keybinds;
                    let _ = set_config_value("command_palette_show_keybinds", &view.config.command_palette_show_keybinds.to_string());
                },
            ))
    }

    fn render_tabs_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let use_tabs = self.config.use_tabs;
        let max_tabs = self.config.max_tabs;
        let shell_integration = self.config.tab_title.shell_integration;
        let fallback = self.config.tab_title.fallback.clone();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.render_section_header("Tabs", "Configure tab behavior and titles"))
            .child(self.render_group_header("TAB BAR"))
            .child(self.render_setting_row(
                "use-tabs-toggle",
                "Enable Tabs",
                "Show tab bar when multiple tabs are open",
                use_tabs,
                cx,
                |view, _cx| {
                    view.config.use_tabs = !view.config.use_tabs;
                    let _ = set_config_value("use_tabs", &view.config.use_tabs.to_string());
                },
            ))
            .child(self.render_info_row_with_desc(
                "Maximum Tabs",
                "Memory optimization limit",
                format!("{}", max_tabs),
            ))
            .child(self.render_group_header("TAB TITLES"))
            .child(self.render_tab_title_mode_row(cx))
            .child(self.render_setting_row(
                "shell-integration-toggle",
                "Shell Integration",
                "Export TERMY_* env vars for shell hooks",
                shell_integration,
                cx,
                |view, _cx| {
                    view.config.tab_title.shell_integration = !view.config.tab_title.shell_integration;
                    let _ = set_config_value("tab_title_shell_integration", &view.config.tab_title.shell_integration.to_string());
                },
            ))
            .child(self.render_info_row_with_desc(
                "Fallback Title",
                "Default when no other source available",
                fallback,
            ))
    }

    fn render_advanced_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let working_dir = self.config.working_dir.clone().unwrap_or_else(|| "Not set".to_string());
        let window_width = self.config.window_width;
        let window_height = self.config.window_height;

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.render_section_header("Advanced", "Advanced configuration options"))
            .child(self.render_group_header("STARTUP"))
            .child(self.render_info_row_with_desc(
                "Working Directory",
                "Initial directory for new sessions",
                working_dir,
            ))
            .child(self.render_group_header("WINDOW"))
            .child(self.render_info_row("Default Width", format!("{}px", window_width as i32)))
            .child(self.render_info_row("Default Height", format!("{}px", window_height as i32)))
            .child(self.render_group_header("CONFIG FILE"))
            .child(
                div()
                    .py_4()
                    .px_4()
                    .rounded_lg()
                    .bg(rgb(CARD_BG))
                    .border_1()
                    .border_color(rgb(BORDER_COLOR))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(TEXT_MUTED))
                            .child("To change these settings, edit the config file:"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(TEXT_SECONDARY))
                            .child("~/.config/termy/config.txt"),
                    )
                    .child(
                        div()
                            .id("open-config-btn")
                            .mt_2()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(ACCENT))
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x5558e3)))
                            .child("Open Config File")
                            .on_click(cx.listener(|_view, _, _, cx| {
                                crate::config::open_config_file();
                                cx.notify();
                            })),
                    ),
            )
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .bg(rgb(CONTENT_BG))
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .p_6()
                    .child(self.render_content(cx)),
            )
    }
}
