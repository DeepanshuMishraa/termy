use crate::colors::TerminalColors;
use crate::config::{AppConfig, CursorStyle, TabTitleMode, set_config_value};
use crate::text_input::{TextInputAlignment, TextInputElement, TextInputProvider, TextInputState};
use gpui::{
    AnyElement, Context, FocusHandle, Font, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render, Rgba,
    ScrollWheelEvent, SharedString, StatefulInteractiveElement, Styled, TextAlign, Window, div,
    prelude::FluentBuilder, px,
};

const SIDEBAR_WIDTH: f32 = 220.0;
const NUMERIC_INPUT_WIDTH: f32 = 220.0;
const NUMERIC_INPUT_HEIGHT: f32 = 34.0;
const NUMERIC_STEP_BUTTON_SIZE: f32 = 24.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EditableField {
    Theme,
    BackgroundOpacity,
    FontFamily,
    FontSize,
    PaddingX,
    PaddingY,
    Shell,
    Term,
    Colorterm,
    ScrollbackHistory,
    ScrollMultiplier,
    MaxTabs,
    TabFallbackTitle,
    WorkingDirectory,
    WindowWidth,
    WindowHeight,
}

#[derive(Clone, Debug)]
struct ActiveTextInput {
    field: EditableField,
    state: TextInputState,
    selecting: bool,
}

impl ActiveTextInput {
    fn new(field: EditableField, text: String) -> Self {
        Self {
            field,
            state: TextInputState::new(text),
            selecting: false,
        }
    }
}

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
    focus_handle: FocusHandle,
    active_input: Option<ActiveTextInput>,
    colors: TerminalColors,
}

impl SettingsWindow {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = AppConfig::load_or_create();
        let colors = TerminalColors::from_theme(&config.theme, &config.colors);
        Self {
            active_section: SettingsSection::Appearance,
            config,
            focus_handle: cx.focus_handle(),
            active_input: None,
            colors,
        }
    }

    // Color helpers derived from terminal theme
    fn bg_primary(&self) -> Rgba {
        self.colors.background
    }

    fn bg_secondary(&self) -> Rgba {
        let mut c = self.colors.background;
        c.a = 0.7;
        c
    }

    fn bg_card(&self) -> Rgba {
        let mut c = self.colors.background;
        c.a = 0.5;
        c
    }

    fn bg_input(&self) -> Rgba {
        let mut c = self.colors.background;
        c.a = 0.3;
        c
    }

    fn bg_hover(&self) -> Rgba {
        let mut c = self.colors.foreground;
        c.a = 0.1;
        c
    }

    fn bg_active(&self) -> Rgba {
        let mut c = self.colors.foreground;
        c.a = 0.15;
        c
    }

    fn text_primary(&self) -> Rgba {
        self.colors.foreground
    }

    fn text_secondary(&self) -> Rgba {
        let mut c = self.colors.foreground;
        c.a = 0.7;
        c
    }

    fn text_muted(&self) -> Rgba {
        let mut c = self.colors.foreground;
        c.a = 0.5;
        c
    }

    fn border_color(&self) -> Rgba {
        let mut c = self.colors.foreground;
        c.a = 0.15;
        c
    }

    fn accent(&self) -> Rgba {
        self.colors.cursor
    }

    fn accent_with_alpha(&self, alpha: f32) -> Rgba {
        let mut c = self.colors.cursor;
        c.a = alpha;
        c
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(self.bg_secondary())
            .border_r_1()
            .border_color(self.border_color())
            .flex()
            .flex_col()
            .child(
                div().px_5().pt_6().pb_4().child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(self.text_muted())
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
        let active_bg = self.bg_active();
        let hover_bg = self.bg_hover();
        let text_primary = self.text_primary();
        let text_secondary = self.text_secondary();
        let accent = self.accent();

        div()
            .id(SharedString::from(label))
            .px_3()
            .py(px(10.0))
            .rounded_lg()
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_3()
            .bg(if is_active {
                active_bg
            } else {
                Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }
            })
            .hover(|s| s.bg(hover_bg))
            .child(
                div()
                    .text_sm()
                    .font_weight(if is_active {
                        gpui::FontWeight::MEDIUM
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .text_color(if is_active {
                        text_primary
                    } else {
                        text_secondary
                    })
                    .child(label),
            )
            .when(is_active, |s| {
                s.child(
                    div()
                        .ml_auto()
                        .w(px(3.0))
                        .h(px(16.0))
                        .rounded(px(2.0))
                        .bg(accent),
                )
            })
            .on_click(cx.listener(move |view, _, _, cx| {
                view.active_section = section;
                view.active_input = None;
                cx.notify();
            }))
    }

    fn editable_field_value(&self, field: EditableField) -> String {
        match field {
            EditableField::Theme => self.config.theme.clone(),
            EditableField::BackgroundOpacity => format!(
                "{}",
                (self.config.background_opacity * 100.0).round() as i32
            ),
            EditableField::FontFamily => self.config.font_family.clone(),
            EditableField::FontSize => format!("{}", self.config.font_size.round() as i32),
            EditableField::PaddingX => format!("{}", self.config.padding_x.round() as i32),
            EditableField::PaddingY => format!("{}", self.config.padding_y.round() as i32),
            EditableField::Shell => self.config.shell.clone().unwrap_or_default(),
            EditableField::Term => self.config.term.clone(),
            EditableField::Colorterm => self.config.colorterm.clone().unwrap_or_default(),
            EditableField::ScrollbackHistory => self.config.scrollback_history.to_string(),
            EditableField::ScrollMultiplier => format!("{}", self.config.mouse_scroll_multiplier),
            EditableField::MaxTabs => self.config.max_tabs.to_string(),
            EditableField::TabFallbackTitle => self.config.tab_title.fallback.clone(),
            EditableField::WorkingDirectory => self.config.working_dir.clone().unwrap_or_default(),
            EditableField::WindowWidth => format!("{}", self.config.window_width.round() as i32),
            EditableField::WindowHeight => format!("{}", self.config.window_height.round() as i32),
        }
    }

    fn apply_editable_field(&mut self, field: EditableField, raw: &str) -> Result<(), String> {
        let value = raw.trim();
        match field {
            EditableField::Theme => {
                if value.is_empty() {
                    return Err("Theme cannot be empty".to_string());
                }
                let message = crate::config::set_theme_in_config(value)?;
                let canonical_theme = message
                    .strip_prefix("Theme set to ")
                    .unwrap_or(value)
                    .to_string();
                self.config.theme = canonical_theme;
                Ok(())
            }
            EditableField::BackgroundOpacity => {
                let parsed = value
                    .trim_end_matches('%')
                    .parse::<f32>()
                    .map_err(|_| "Background opacity must be a number from 0 to 100".to_string())?;
                let opacity = (parsed / 100.0).clamp(0.0, 1.0);
                self.config.background_opacity = opacity;
                set_config_value("background_opacity", &format!("{:.3}", opacity))
            }
            EditableField::FontFamily => {
                if value.is_empty() {
                    return Err("Font family cannot be empty".to_string());
                }
                self.config.font_family = value.to_string();
                set_config_value("font_family", value)
            }
            EditableField::FontSize => {
                let parsed = value
                    .parse::<f32>()
                    .map_err(|_| "Font size must be a positive number".to_string())?;
                if parsed <= 0.0 {
                    return Err("Font size must be greater than 0".to_string());
                }
                self.config.font_size = parsed;
                set_config_value("font_size", &format!("{}", parsed))
            }
            EditableField::PaddingX => {
                let parsed = value
                    .parse::<f32>()
                    .map_err(|_| "Horizontal padding must be a number".to_string())?;
                if parsed < 0.0 {
                    return Err("Horizontal padding cannot be negative".to_string());
                }
                self.config.padding_x = parsed;
                set_config_value("padding_x", &format!("{}", parsed))
            }
            EditableField::PaddingY => {
                let parsed = value
                    .parse::<f32>()
                    .map_err(|_| "Vertical padding must be a number".to_string())?;
                if parsed < 0.0 {
                    return Err("Vertical padding cannot be negative".to_string());
                }
                self.config.padding_y = parsed;
                set_config_value("padding_y", &format!("{}", parsed))
            }
            EditableField::Shell => {
                if value.is_empty() {
                    self.config.shell = None;
                    set_config_value("shell", "none")
                } else {
                    self.config.shell = Some(value.to_string());
                    set_config_value("shell", value)
                }
            }
            EditableField::Term => {
                if value.is_empty() {
                    return Err("TERM cannot be empty".to_string());
                }
                self.config.term = value.to_string();
                set_config_value("term", value)
            }
            EditableField::Colorterm => {
                if value.is_empty() {
                    self.config.colorterm = None;
                    set_config_value("colorterm", "none")
                } else {
                    self.config.colorterm = Some(value.to_string());
                    set_config_value("colorterm", value)
                }
            }
            EditableField::ScrollbackHistory => {
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| "Scrollback history must be a positive integer".to_string())?;
                let parsed = parsed.min(100_000);
                self.config.scrollback_history = parsed;
                set_config_value("scrollback_history", &parsed.to_string())
            }
            EditableField::ScrollMultiplier => {
                let parsed = value
                    .parse::<f32>()
                    .map_err(|_| "Scroll multiplier must be a number".to_string())?;
                if !parsed.is_finite() {
                    return Err("Scroll multiplier must be finite".to_string());
                }
                let parsed = parsed.clamp(0.1, 1000.0);
                self.config.mouse_scroll_multiplier = parsed;
                set_config_value("mouse_scroll_multiplier", &parsed.to_string())
            }
            EditableField::MaxTabs => {
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| "Maximum tabs must be a positive integer".to_string())?;
                if parsed == 0 {
                    return Err("Maximum tabs must be greater than 0".to_string());
                }
                self.config.max_tabs = parsed;
                set_config_value("max_tabs", &parsed.to_string())
            }
            EditableField::TabFallbackTitle => {
                if value.is_empty() {
                    return Err("Fallback title cannot be empty".to_string());
                }
                self.config.tab_title.fallback = value.to_string();
                set_config_value("tab_title_fallback", value)
            }
            EditableField::WorkingDirectory => {
                if value.is_empty() {
                    self.config.working_dir = None;
                    set_config_value("working_dir", "none")
                } else {
                    self.config.working_dir = Some(value.to_string());
                    set_config_value("working_dir", value)
                }
            }
            EditableField::WindowWidth => {
                let parsed = value
                    .parse::<f32>()
                    .map_err(|_| "Default width must be a positive number".to_string())?;
                if parsed <= 0.0 {
                    return Err("Default width must be greater than 0".to_string());
                }
                self.config.window_width = parsed;
                set_config_value("window_width", &parsed.to_string())
            }
            EditableField::WindowHeight => {
                let parsed = value
                    .parse::<f32>()
                    .map_err(|_| "Default height must be a positive number".to_string())?;
                if parsed <= 0.0 {
                    return Err("Default height must be greater than 0".to_string());
                }
                self.config.window_height = parsed;
                set_config_value("window_height", &parsed.to_string())
            }
        }
    }

    fn begin_editing_field(
        &mut self,
        field: EditableField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_input = Some(ActiveTextInput::new(
            field,
            self.editable_field_value(field),
        ));
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn is_numeric_field(field: EditableField) -> bool {
        matches!(
            field,
            EditableField::BackgroundOpacity
                | EditableField::FontSize
                | EditableField::PaddingX
                | EditableField::PaddingY
                | EditableField::ScrollbackHistory
                | EditableField::ScrollMultiplier
                | EditableField::MaxTabs
                | EditableField::WindowWidth
                | EditableField::WindowHeight
        )
    }

    fn step_numeric_field(&mut self, field: EditableField, delta: i32, cx: &mut Context<Self>) {
        let result = match field {
            EditableField::BackgroundOpacity => {
                let next = (self.config.background_opacity + (delta as f32 * 0.05)).clamp(0.0, 1.0);
                self.config.background_opacity = next;
                set_config_value("background_opacity", &format!("{:.3}", next))
            }
            EditableField::FontSize => {
                let next = (self.config.font_size + delta as f32).max(1.0);
                self.config.font_size = next;
                set_config_value("font_size", &next.to_string())
            }
            EditableField::PaddingX => {
                let next = (self.config.padding_x + delta as f32).max(0.0);
                self.config.padding_x = next;
                set_config_value("padding_x", &next.to_string())
            }
            EditableField::PaddingY => {
                let next = (self.config.padding_y + delta as f32).max(0.0);
                self.config.padding_y = next;
                set_config_value("padding_y", &next.to_string())
            }
            EditableField::ScrollbackHistory => {
                let next = (self.config.scrollback_history as i64 + (delta as i64 * 100))
                    .clamp(0, 100_000) as usize;
                self.config.scrollback_history = next;
                set_config_value("scrollback_history", &next.to_string())
            }
            EditableField::ScrollMultiplier => {
                let next =
                    (self.config.mouse_scroll_multiplier + (delta as f32 * 0.1)).clamp(0.1, 1000.0);
                self.config.mouse_scroll_multiplier = next;
                set_config_value("mouse_scroll_multiplier", &next.to_string())
            }
            EditableField::MaxTabs => {
                let next = (self.config.max_tabs as i64 + delta as i64).max(1) as usize;
                self.config.max_tabs = next;
                set_config_value("max_tabs", &next.to_string())
            }
            EditableField::WindowWidth => {
                let next = (self.config.window_width + (delta as f32 * 20.0)).max(1.0);
                self.config.window_width = next;
                set_config_value("window_width", &next.to_string())
            }
            EditableField::WindowHeight => {
                let next = (self.config.window_height + (delta as f32 * 20.0)).max(1.0);
                self.config.window_height = next;
                set_config_value("window_height", &next.to_string())
            }
            _ => Ok(()),
        };

        if let Err(error) = result {
            termy_toast::error(error);
        }
        self.active_input = None;
        cx.notify();
    }

    fn ordered_theme_ids_for_settings(&self) -> Vec<String> {
        let mut theme_ids: Vec<String> = termy_themes::available_theme_ids()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect();

        if !theme_ids.iter().any(|theme| theme == &self.config.theme) {
            theme_ids.push(self.config.theme.clone());
        }

        theme_ids.sort_unstable();
        theme_ids.dedup();
        theme_ids
    }

    fn filtered_theme_suggestions(&self, query: &str) -> Vec<String> {
        let normalized = query.trim().to_ascii_lowercase();
        let themes = self.ordered_theme_ids_for_settings();

        if normalized.is_empty() {
            return themes.into_iter().take(16).collect();
        }

        let mut matched = Vec::new();
        let mut rest = Vec::new();
        for theme in themes {
            let lower = theme.to_ascii_lowercase();
            if lower.contains(&normalized) || lower.replace('-', " ").contains(&normalized) {
                matched.push(theme);
            } else {
                rest.push(theme);
            }
        }
        matched.extend(rest);
        matched.into_iter().take(16).collect()
    }

    fn apply_theme_selection(&mut self, theme_id: &str, cx: &mut Context<Self>) {
        if let Err(error) = self.apply_editable_field(EditableField::Theme, theme_id) {
            termy_toast::error(error);
        }
        self.active_input = None;
        cx.notify();
    }

    fn commit_active_input(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.active_input.take() else {
            return;
        };

        if let Err(error) = self.apply_editable_field(input.field, input.state.text()) {
            termy_toast::error(error);
            self.active_input = Some(input);
        }
        cx.notify();
    }

    fn cancel_active_input(&mut self, cx: &mut Context<Self>) {
        self.active_input = None;
        cx.notify();
    }

    fn render_content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w_full()
            .child(match self.active_section {
                SettingsSection::Appearance => {
                    self.render_appearance_section(cx).into_any_element()
                }
                SettingsSection::Terminal => self.render_terminal_section(cx).into_any_element(),
                SettingsSection::Tabs => self.render_tabs_section(cx).into_any_element(),
                SettingsSection::Advanced => self.render_advanced_section(cx).into_any_element(),
            })
            .into_any_element()
    }

    fn render_section_header(
        &self,
        title: &'static str,
        subtitle: &'static str,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .mb_6()
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(self.text_primary())
                    .child(title),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(self.text_muted())
                    .child(subtitle),
            )
    }

    fn render_group_header(&self, title: &'static str) -> impl IntoElement {
        div()
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(self.text_muted())
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
            .bg(self.bg_card())
            .border_1()
            .border_color(self.border_color())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(self.text_primary())
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(self.text_muted())
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
        let accent = self.accent();
        // Off state: use a more visible muted foreground color
        let mut bg_off = self.colors.foreground;
        bg_off.a = 0.25;
        // Knob: white/light when on, slightly dimmer when off
        let knob_color = if checked {
            self.text_primary()
        } else {
            self.text_secondary()
        };

        div()
            .id(SharedString::from(id))
            .w(px(44.0))
            .h(px(24.0))
            .rounded(px(12.0))
            .bg(if checked { accent } else { bg_off })
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
                    .bg(knob_color)
                    .shadow_sm(),
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                on_toggle(view, cx);
                cx.notify();
            }))
    }

    fn render_editable_row(
        &mut self,
        field: EditableField,
        title: &'static str,
        description: &'static str,
        display_value: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_numeric = Self::is_numeric_field(field);
        let is_active = self
            .active_input
            .as_ref()
            .is_some_and(|input| input.field == field);
        let is_theme_field = field == EditableField::Theme;
        let accent_inner_border = is_numeric || is_theme_field;
        let theme_suggestions = if is_theme_field && is_active {
            let query = self
                .active_input
                .as_ref()
                .map(|input| input.state.text())
                .unwrap_or("");
            self.filtered_theme_suggestions(query)
        } else {
            Vec::new()
        };

        // Cache colors for closures
        let text_secondary = self.text_secondary();
        let hover_bg = self.bg_hover();
        let input_bg = self.bg_input();
        let border_color = self.border_color();
        let accent = self.accent();
        let bg_card = self.bg_card();
        let text_primary = self.text_primary();
        let text_muted = self.text_muted();

        let mut theme_dropdown = None;
        let theme_dropdown_open = is_theme_field && is_active && !theme_suggestions.is_empty();
        if theme_dropdown_open {
            let mut list = div().flex().flex_col().py_1();
            for theme_id in theme_suggestions {
                let theme_label = theme_id.clone();
                list = list.child(
                    div()
                        .id(SharedString::from(format!("theme-option-{theme_label}")))
                        .px_3()
                        .py_1()
                        .text_sm()
                        .text_color(text_secondary)
                        .cursor_pointer()
                        .hover(|this| this.bg(hover_bg))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, _event: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                                view.apply_theme_selection(&theme_id, cx);
                            }),
                        )
                        .child(theme_label),
                );
            }

            // Use a fully opaque background for the dropdown so it covers content below
            let dropdown_bg = self.bg_primary();
            theme_dropdown = Some(
                div()
                    .id("theme-suggestions-list")
                    .occlude()
                    .absolute()
                    .top(px(34.0))
                    .left_0()
                    .right_0()
                    .max_h(px(180.0))
                    .overflow_scroll()
                    .overflow_x_hidden()
                    .rounded_md()
                    .bg(dropdown_bg)
                    .border_1()
                    .border_color(border_color)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_view, _event: &MouseDownEvent, _window, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .on_scroll_wheel(cx.listener(
                        |_view, _event: &ScrollWheelEvent, _window, cx| {
                            cx.stop_propagation();
                        },
                    ))
                    .child(list)
                    .into_any_element(),
            );
        }

        let value_element = if is_numeric {
            div()
                .h_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_1()
                .child(
                    div()
                        .id(SharedString::from(format!("dec-{field:?}")))
                        .w(px(NUMERIC_STEP_BUTTON_SIZE))
                        .h(px(NUMERIC_STEP_BUTTON_SIZE))
                        .rounded_sm()
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(bg_card)
                        .text_color(text_primary)
                        .text_sm()
                        .child("-")
                        .on_click(cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            view.step_numeric_field(field, -1, cx);
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(text_secondary)
                        .text_align(TextAlign::Center)
                        .child(display_value),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("inc-{field:?}")))
                        .w(px(NUMERIC_STEP_BUTTON_SIZE))
                        .h(px(NUMERIC_STEP_BUTTON_SIZE))
                        .rounded_sm()
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(bg_card)
                        .text_color(text_primary)
                        .text_sm()
                        .child("+")
                        .on_click(cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            view.step_numeric_field(field, 1, cx);
                        })),
                )
                .into_any_element()
        } else if is_active {
            let font = Font {
                family: self.config.font_family.clone().into(),
                ..Font::default()
            };
            let selection_color = self.accent_with_alpha(0.3);
            TextInputElement::new(
                cx.entity(),
                self.focus_handle.clone(),
                font,
                px(13.0),
                text_secondary.into(),
                selection_color.into(),
                TextInputAlignment::Left,
            )
            .into_any_element()
        } else {
            div()
                .text_sm()
                .text_color(text_secondary)
                .child(display_value)
                .into_any_element()
        };

        div()
            .id(SharedString::from(format!("editable-row-{field:?}")))
            .flex()
            .items_start()
            .gap_4()
            .py_3()
            .px_4()
            .rounded_lg()
            .bg(bg_card)
            .border_1()
            .border_color(if theme_dropdown_open {
                Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }
            } else {
                border_color
            })
            .cursor_pointer()
            .when(!is_numeric, |s| {
                s.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        if !view
                            .active_input
                            .as_ref()
                            .is_some_and(|input| input.field == field)
                        {
                            view.begin_editing_field(field, window, cx);
                        }

                        if let Some(input) = view.active_input.as_mut() {
                            let index = input.state.character_index_for_point(event.position);
                            if event.modifiers.shift {
                                input.state.select_to_utf16(index);
                            } else {
                                input.state.set_cursor_utf16(index);
                            }
                            input.selecting = true;
                        }

                        view.focus_handle.focus(window, cx);
                        cx.notify();
                    }),
                )
                .on_mouse_move(
                    cx.listener(move |view, event: &MouseMoveEvent, _window, cx| {
                        let Some(input) = view.active_input.as_mut() else {
                            return;
                        };
                        if input.field != field || !input.selecting || !event.dragging() {
                            return;
                        }
                        let index = input.state.character_index_for_point(event.position);
                        input.state.select_to_utf16(index);
                        cx.notify();
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |view, _event: &MouseUpEvent, _window, cx| {
                        if let Some(input) = view.active_input.as_mut()
                            && input.field == field
                        {
                            input.selecting = false;
                            cx.notify();
                        }
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(move |view, _event: &MouseUpEvent, _window, cx| {
                        if let Some(input) = view.active_input.as_mut()
                            && input.field == field
                        {
                            input.selecting = false;
                            cx.notify();
                        }
                    }),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(text_primary)
                            .child(title),
                    )
                    .child(div().text_xs().text_color(text_muted).child(description)),
            )
            .child(
                div()
                    .when(is_numeric, |s| s.w(px(NUMERIC_INPUT_WIDTH)).flex_none())
                    .when(!is_numeric, |s| {
                        s.flex_1().min_w(px(220.0)).max_w(px(560.0))
                    })
                    .relative()
                    .h(if is_numeric {
                        px(NUMERIC_INPUT_HEIGHT)
                    } else {
                        px(28.0)
                    })
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .h_full()
                            .px_2()
                            .rounded_md()
                            .bg(input_bg)
                            .border_1()
                            .border_color(if is_active && accent_inner_border {
                                accent.into()
                            } else {
                                border_color
                            })
                            .child(value_element),
                    )
                    .when_some(theme_dropdown, |s, dropdown| s.child(dropdown)),
            )
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_input.is_none() {
            return;
        }

        match event.keystroke.key.as_str() {
            "enter" => self.commit_active_input(cx),
            "escape" => self.cancel_active_input(cx),
            "tab" => {
                if self
                    .active_input
                    .as_ref()
                    .is_some_and(|input| input.field == EditableField::Theme)
                    && let Some(first) = self
                        .active_input
                        .as_ref()
                        .map(|input| self.filtered_theme_suggestions(input.state.text()))
                        .and_then(|items| items.into_iter().next())
                {
                    self.apply_theme_selection(&first, cx);
                }
            }
            "backspace" => {
                if let Some(input) = self.active_input.as_mut() {
                    input.state.delete_backward();
                }
                cx.notify();
            }
            "delete" => {
                if let Some(input) = self.active_input.as_mut() {
                    input.state.delete_forward();
                }
                cx.notify();
            }
            "left" => {
                if let Some(input) = self.active_input.as_mut() {
                    input.state.move_left();
                }
                cx.notify();
            }
            "right" => {
                if let Some(input) = self.active_input.as_mut() {
                    input.state.move_right();
                }
                cx.notify();
            }
            "home" => {
                if let Some(input) = self.active_input.as_mut() {
                    input.state.move_to_start();
                }
                cx.notify();
            }
            "end" => {
                if let Some(input) = self.active_input.as_mut() {
                    input.state.move_to_end();
                }
                cx.notify();
            }
            _ => {}
        }
    }

    fn render_cursor_style_row(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.config.cursor_style;
        let bg_card = self.bg_card();
        let border_color = self.border_color();
        let text_primary = self.text_primary();
        let text_muted = self.text_muted();
        let text_secondary = self.text_secondary();
        let accent = self.accent();
        let hover_bg = self.bg_hover();
        let switch_off_bg = self.bg_input();
        let white = self.colors.foreground;

        div()
            .flex()
            .items_center()
            .justify_between()
            .py_3()
            .px_4()
            .rounded_lg()
            .bg(bg_card)
            .border_1()
            .border_color(border_color)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(text_primary)
                            .child("Cursor Style"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_muted)
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
                            .bg(if is_selected {
                                accent.into()
                            } else {
                                switch_off_bg
                            })
                            .text_color(if is_selected {
                                white
                            } else {
                                text_secondary.into()
                            })
                            .hover(|s| if !is_selected { s.bg(hover_bg) } else { s })
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
                            .bg(if is_selected {
                                accent.into()
                            } else {
                                switch_off_bg
                            })
                            .text_color(if is_selected {
                                white
                            } else {
                                text_secondary.into()
                            })
                            .hover(|s| if !is_selected { s.bg(hover_bg) } else { s })
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
        let bg_card = self.bg_card();
        let border_color = self.border_color();
        let text_primary = self.text_primary();
        let text_muted = self.text_muted();
        let text_secondary = self.text_secondary();
        let accent = self.accent();
        let hover_bg = self.bg_hover();
        let switch_off_bg = self.bg_input();
        let white = self.colors.foreground;

        div()
            .flex()
            .items_center()
            .justify_between()
            .py_3()
            .px_4()
            .rounded_lg()
            .bg(bg_card)
            .border_1()
            .border_color(border_color)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(text_primary)
                            .child("Title Mode"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_muted)
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
                            .bg(if is_selected {
                                accent.into()
                            } else {
                                switch_off_bg
                            })
                            .text_color(if is_selected {
                                white
                            } else {
                                text_secondary.into()
                            })
                            .hover(|s| if !is_selected { s.bg(hover_bg) } else { s })
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
                            .bg(if is_selected {
                                accent.into()
                            } else {
                                switch_off_bg
                            })
                            .text_color(if is_selected {
                                white
                            } else {
                                text_secondary.into()
                            })
                            .hover(|s| if !is_selected { s.bg(hover_bg) } else { s })
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
                            .bg(if is_selected {
                                accent.into()
                            } else {
                                switch_off_bg
                            })
                            .text_color(if is_selected {
                                white
                            } else {
                                text_secondary.into()
                            })
                            .hover(|s| if !is_selected { s.bg(hover_bg) } else { s })
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
                            .bg(if is_selected {
                                accent.into()
                            } else {
                                switch_off_bg
                            })
                            .text_color(if is_selected {
                                white
                            } else {
                                text_secondary.into()
                            })
                            .hover(|s| if !is_selected { s.bg(hover_bg) } else { s })
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
            .child(self.render_editable_row(
                EditableField::Theme,
                "Theme",
                "Current color scheme name",
                theme,
                cx,
            ))
            .child(self.render_group_header("WINDOW"))
            .child(self.render_setting_row(
                "blur-toggle",
                "Background Blur",
                "Enable blur effect for transparent backgrounds",
                background_blur,
                cx,
                |view, _cx| {
                    view.config.background_blur = !view.config.background_blur;
                    let _ = set_config_value(
                        "background_blur",
                        &view.config.background_blur.to_string(),
                    );
                },
            ))
            .child(self.render_editable_row(
                EditableField::BackgroundOpacity,
                "Background Opacity",
                "Window transparency (0-100%)",
                format!("{}%", (background_opacity * 100.0) as i32),
                cx,
            ))
            .child(self.render_group_header("FONT"))
            .child(self.render_editable_row(
                EditableField::FontFamily,
                "Font Family",
                "Font family used in terminal UI",
                font_family,
                cx,
            ))
            .child(self.render_editable_row(
                EditableField::FontSize,
                "Font Size",
                "Terminal font size in pixels",
                format!("{}px", font_size as i32),
                cx,
            ))
            .child(self.render_group_header("PADDING"))
            .child(self.render_editable_row(
                EditableField::PaddingX,
                "Horizontal Padding",
                "Left and right terminal padding",
                format!("{}px", padding_x as i32),
                cx,
            ))
            .child(self.render_editable_row(
                EditableField::PaddingY,
                "Vertical Padding",
                "Top and bottom terminal padding",
                format!("{}px", padding_y as i32),
                cx,
            ))
    }

    fn render_terminal_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let cursor_blink = self.config.cursor_blink;
        let term = self.config.term.clone();
        let shell = self
            .config
            .shell
            .clone()
            .unwrap_or_else(|| "System default".to_string());
        let colorterm = self
            .config
            .colorterm
            .clone()
            .unwrap_or_else(|| "Disabled".to_string());
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
            .child(self.render_editable_row(
                EditableField::Shell,
                "Shell",
                "Executable for new sessions",
                shell,
                cx,
            ))
            .child(self.render_editable_row(
                EditableField::Term,
                "TERM",
                "Terminal type for child apps",
                term,
                cx,
            ))
            .child(self.render_editable_row(
                EditableField::Colorterm,
                "COLORTERM",
                "Color support advertisement",
                colorterm,
                cx,
            ))
            .child(self.render_group_header("SCROLLING"))
            .child(self.render_editable_row(
                EditableField::ScrollbackHistory,
                "Scrollback History",
                "Lines to keep in buffer",
                format!("{} lines", scrollback),
                cx,
            ))
            .child(self.render_editable_row(
                EditableField::ScrollMultiplier,
                "Scroll Multiplier",
                "Mouse wheel scroll speed",
                format!("{}x", scroll_mult),
                cx,
            ))
            .child(self.render_group_header("UI"))
            .child(self.render_setting_row(
                "palette-keybinds-toggle",
                "Show Keybindings in Palette",
                "Display keyboard shortcuts in command palette",
                command_palette_show_keybinds,
                cx,
                |view, _cx| {
                    view.config.command_palette_show_keybinds =
                        !view.config.command_palette_show_keybinds;
                    let _ = set_config_value(
                        "command_palette_show_keybinds",
                        &view.config.command_palette_show_keybinds.to_string(),
                    );
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
            .child(self.render_editable_row(
                EditableField::MaxTabs,
                "Maximum Tabs",
                "Memory optimization limit",
                format!("{}", max_tabs),
                cx,
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
                    view.config.tab_title.shell_integration =
                        !view.config.tab_title.shell_integration;
                    let _ = set_config_value(
                        "tab_title_shell_integration",
                        &view.config.tab_title.shell_integration.to_string(),
                    );
                },
            ))
            .child(self.render_editable_row(
                EditableField::TabFallbackTitle,
                "Fallback Title",
                "Default when no other source available",
                fallback,
                cx,
            ))
    }

    fn render_advanced_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let working_dir = self
            .config
            .working_dir
            .clone()
            .unwrap_or_else(|| "Not set".to_string());
        let window_width = self.config.window_width;
        let window_height = self.config.window_height;
        let bg_card = self.bg_card();
        let border_color = self.border_color();
        let text_muted = self.text_muted();
        let text_secondary = self.text_secondary();
        let accent = self.accent();
        let white = self.colors.foreground;
        let accent_hover = self.accent_with_alpha(0.8);

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.render_section_header("Advanced", "Advanced configuration options"))
            .child(self.render_group_header("STARTUP"))
            .child(self.render_editable_row(
                EditableField::WorkingDirectory,
                "Working Directory",
                "Initial directory for new sessions",
                working_dir,
                cx,
            ))
            .child(self.render_group_header("WINDOW"))
            .child(self.render_editable_row(
                EditableField::WindowWidth,
                "Default Width",
                "Window width on startup",
                format!("{}px", window_width as i32),
                cx,
            ))
            .child(self.render_editable_row(
                EditableField::WindowHeight,
                "Default Height",
                "Window height on startup",
                format!("{}px", window_height as i32),
                cx,
            ))
            .child(self.render_group_header("CONFIG FILE"))
            .child(
                div()
                    .py_4()
                    .px_4()
                    .rounded_lg()
                    .bg(bg_card)
                    .border_1()
                    .border_color(border_color)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(text_muted)
                            .child("To change these settings, edit the config file:"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(text_secondary)
                            .child("~/.config/termy/config.txt"),
                    )
                    .child(
                        div()
                            .id("open-config-btn")
                            .mt_2()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(accent)
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(white)
                            .cursor_pointer()
                            .hover(|s| s.bg(accent_hover))
                            .child("Open Config File")
                            .on_click(cx.listener(|_view, _, _, cx| {
                                crate::config::open_config_file();
                                cx.notify();
                            })),
                    ),
            )
    }
}

impl TextInputProvider for SettingsWindow {
    fn text_input_state(&self) -> Option<&TextInputState> {
        self.active_input.as_ref().map(|input| &input.state)
    }

    fn text_input_state_mut(&mut self) -> Option<&mut TextInputState> {
        self.active_input.as_mut().map(|input| &mut input.state)
    }
}

crate::impl_text_input_handler!(SettingsWindow);

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = self.bg_primary();
        div()
            .id("settings-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_any_mouse_down(cx.listener(|view, _event: &MouseDownEvent, _window, cx| {
                if view.active_input.is_some() {
                    view.cancel_active_input(cx);
                }
            }))
            .flex()
            .size_full()
            .bg(bg)
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .id("settings-content-scroll")
                    .flex_1()
                    .h_full()
                    .overflow_y_scroll()
                    .overflow_x_hidden()
                    .p_6()
                    .child(self.render_content(cx)),
            )
    }
}
