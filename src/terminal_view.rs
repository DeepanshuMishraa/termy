use crate::colors::TerminalColors;
use crate::config::{self, AppConfig, TabTitleConfig, TabTitleSource};
use crate::terminal_grid::{CellRenderInfo, TerminalGrid};
use crate::terminal::{
    TabTitleShellIntegration, Terminal, TerminalEvent, TerminalSize, keystroke_to_input,
};
use alacritty_terminal::term::cell::Flags;
use flume::{Sender, bounded};
use gpui::{
    App, AsyncApp, ClipboardItem, Context, FocusHandle, Focusable, Font, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Render, SharedString, Size, Styled, WeakEntity, Window,
    WindowControlArea, div, px,
};
use std::{fs, path::PathBuf, process::Command, time::{Duration, SystemTime}};

const MIN_FONT_SIZE: f32 = 8.0;
const MAX_FONT_SIZE: f32 = 40.0;
const ZOOM_STEP: f32 = 1.0;
const TITLEBAR_HEIGHT: f32 = 34.0;
const TABBAR_HEIGHT: f32 = 40.0;
const TITLEBAR_PLUS_SIZE: f32 = 22.0;
const WINDOWS_TITLEBAR_BUTTON_WIDTH: f32 = 46.0;
const WINDOWS_TITLEBAR_CONTROLS_WIDTH: f32 = WINDOWS_TITLEBAR_BUTTON_WIDTH * 3.0;
const TITLEBAR_SIDE_PADDING: f32 = 12.0;
const TAB_HORIZONTAL_PADDING: f32 = 12.0;
const TAB_PILL_HEIGHT: f32 = 32.0;
const TAB_PILL_NORMAL_PADDING: f32 = 10.0;
const TAB_PILL_COMPACT_PADDING: f32 = 6.0;
const TAB_PILL_COMPACT_THRESHOLD: f32 = 120.0;
const TAB_PILL_GAP: f32 = 8.0;
const TAB_CLOSE_HITBOX: f32 = 22.0;
const TAB_INACTIVE_CLOSE_MIN_WIDTH: f32 = 120.0;
const MAX_TAB_TITLE_CHARS: usize = 96;
const DEFAULT_TAB_TITLE: &str = "Terminal";
const COMMAND_TITLE_DELAY_MS: u64 = 250;
const CONFIG_WATCH_INTERVAL_MS: u64 = 750;
const SELECTION_BG_ALPHA: f32 = 0.35;
const DIM_TEXT_FACTOR: f32 = 0.66;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellPos {
    col: usize,
    row: usize,
}

#[derive(Clone, Copy, Debug)]
struct TabBarLayout {
    tab_pill_width: f32,
    tab_padding_x: f32,
    slot_width: f32,
}

struct TerminalTab {
    terminal: Terminal,
    manual_title: Option<String>,
    explicit_title: Option<String>,
    shell_title: Option<String>,
    pending_command_title: Option<String>,
    pending_command_token: u64,
    title: String,
}

impl TerminalTab {
    fn new(terminal: Terminal) -> Self {
        Self {
            terminal,
            manual_title: None,
            explicit_title: None,
            shell_title: None,
            pending_command_title: None,
            pending_command_token: 0,
            title: DEFAULT_TAB_TITLE.to_string(),
        }
    }
}

enum ExplicitTitlePayload {
    Prompt(String),
    Command(String),
    Title(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HoveredLink {
    row: usize,
    start_col: usize,
    end_col: usize,
    target: String,
}

/// The main terminal view component
pub struct TerminalView {
    tabs: Vec<TerminalTab>,
    active_tab: usize,
    renaming_tab: Option<usize>,
    rename_buffer: String,
    event_wakeup_tx: Sender<()>,
    focus_handle: FocusHandle,
    colors: TerminalColors,
    use_tabs: bool,
    tab_title: TabTitleConfig,
    tab_shell_integration: TabTitleShellIntegration,
    configured_working_dir: Option<String>,
    config_path: Option<PathBuf>,
    config_last_modified: Option<SystemTime>,
    font_family: SharedString,
    base_font_size: f32,
    font_size: Pixels,
    padding_x: f32,
    padding_y: f32,
    line_height: f32,
    selection_anchor: Option<CellPos>,
    selection_head: Option<CellPos>,
    selection_dragging: bool,
    selection_moved: bool,
    hovered_link: Option<HoveredLink>,
    /// Cached cell dimensions
    cell_size: Option<Size<Pixels>>,
}

impl TerminalView {
    fn config_last_modified(path: &PathBuf) -> Option<SystemTime> {
        fs::metadata(path).ok()?.modified().ok()
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let (event_wakeup_tx, event_wakeup_rx) = bounded(1);

        // Focus the terminal immediately
        focus_handle.focus(window, cx);

        // Process terminal events only when terminals signal activity.
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while event_wakeup_rx.recv_async().await.is_ok() {
                while event_wakeup_rx.try_recv().is_ok() {}
                let result = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        if view.process_terminal_events(cx) {
                            cx.notify();
                        }
                    })
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();

        // Poll config file timestamp and hot-reload UI settings on change.
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                smol::Timer::after(Duration::from_millis(CONFIG_WATCH_INTERVAL_MS)).await;
                let result = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        if view.reload_config_if_changed() {
                            cx.notify();
                        }
                    })
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();

        let config = AppConfig::load_or_create();
        let config_path = config::ensure_config_file();
        let config_last_modified = config_path
            .as_ref()
            .and_then(Self::config_last_modified);
        let colors = TerminalColors::from_theme(config.theme);
        let base_font_size = config.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        let padding_x = config.padding_x.max(0.0);
        let padding_y = config.padding_y.max(0.0);
        let configured_working_dir = config.working_dir.clone();
        let tab_title = config.tab_title.clone();
        let tab_shell_integration = TabTitleShellIntegration {
            enabled: tab_title.shell_integration,
            explicit_prefix: tab_title.explicit_prefix.clone(),
        };
        let terminal = Terminal::new(
            TerminalSize::default(),
            configured_working_dir.as_deref(),
            Some(event_wakeup_tx.clone()),
            Some(&tab_shell_integration),
        )
        .expect("Failed to create terminal");

        let mut view = Self {
            tabs: vec![TerminalTab::new(terminal)],
            active_tab: 0,
            renaming_tab: None,
            rename_buffer: String::new(),
            event_wakeup_tx,
            focus_handle,
            colors,
            use_tabs: config.use_tabs,
            tab_title,
            tab_shell_integration,
            configured_working_dir,
            config_path,
            config_last_modified,
            font_family: config.font_family.into(),
            base_font_size,
            font_size: px(base_font_size),
            padding_x,
            padding_y,
            line_height: 1.4,
            selection_anchor: None,
            selection_head: None,
            selection_dragging: false,
            selection_moved: false,
            hovered_link: None,
            cell_size: None,
        };
        view.refresh_tab_title(0);
        view
    }

    fn apply_runtime_config(&mut self, config: AppConfig) -> bool {
        self.colors = TerminalColors::from_theme(config.theme);
        self.use_tabs = config.use_tabs;
        self.tab_title = config.tab_title.clone();
        self.tab_shell_integration = TabTitleShellIntegration {
            enabled: self.tab_title.shell_integration,
            explicit_prefix: self.tab_title.explicit_prefix.clone(),
        };
        self.configured_working_dir = config.working_dir.clone();
        self.font_family = config.font_family.into();
        self.base_font_size = config.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.font_size = px(self.base_font_size);
        self.padding_x = config.padding_x.max(0.0);
        self.padding_y = config.padding_y.max(0.0);

        for index in 0..self.tabs.len() {
            self.refresh_tab_title(index);
        }

        true
    }

    fn reload_config_if_changed(&mut self) -> bool {
        let path = match self.config_path.clone() {
            Some(path) => path,
            None => {
                self.config_path = config::ensure_config_file();
                match self.config_path.clone() {
                    Some(path) => path,
                    None => return false,
                }
            }
        };

        let Some(modified) = Self::config_last_modified(&path) else {
            return false;
        };

        if let Some(last) = self.config_last_modified
            && modified <= last
        {
            return false;
        }

        self.config_last_modified = Some(modified);
        let config = AppConfig::load_or_create();
        self.apply_runtime_config(config)
    }

    fn process_terminal_events(&mut self, cx: &mut Context<Self>) -> bool {
        let mut should_redraw = false;
        let active_tab = self.active_tab;

        for index in 0..self.tabs.len() {
            let events = self.tabs[index].terminal.process_events();
            for event in events {
                match event {
                    TerminalEvent::Wakeup | TerminalEvent::Bell | TerminalEvent::Exit => {
                        if index == active_tab {
                            should_redraw = true;
                        }
                    }
                    TerminalEvent::Title(title) => {
                        if self.apply_terminal_title(index, &title, cx)
                            && (index == active_tab || self.show_tab_bar())
                        {
                            should_redraw = true;
                        }
                    }
                    TerminalEvent::ResetTitle => {
                        if self.clear_terminal_titles(index)
                            && (index == active_tab || self.show_tab_bar())
                        {
                            should_redraw = true;
                        }
                    }
                }
            }
        }

        should_redraw
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_head = None;
        self.selection_dragging = false;
        self.selection_moved = false;
    }

    fn clear_hovered_link(&mut self) -> bool {
        if self.hovered_link.is_some() {
            self.hovered_link = None;
            true
        } else {
            false
        }
    }

    fn show_tab_bar(&self) -> bool {
        self.use_tabs && self.tabs.len() > 1
    }

    fn active_terminal(&self) -> &Terminal {
        &self.tabs[self.active_tab].terminal
    }

    fn active_terminal_mut(&mut self) -> &mut Terminal {
        &mut self.tabs[self.active_tab].terminal
    }

    fn truncate_tab_title(title: &str) -> String {
        // Keep titles single-line so shell-provided newlines do not break tab layout.
        let normalized = title.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.chars().count() > MAX_TAB_TITLE_CHARS {
            return normalized.chars().take(MAX_TAB_TITLE_CHARS).collect();
        }
        normalized
    }

    fn fallback_title(&self) -> &str {
        let fallback = self.tab_title.fallback.trim();
        if fallback.is_empty() {
            DEFAULT_TAB_TITLE
        } else {
            fallback
        }
    }

    fn resolve_template(template: &str, cwd: Option<&str>, command: Option<&str>) -> String {
        template
            .replace("{cwd}", cwd.unwrap_or(""))
            .replace("{command}", command.unwrap_or(""))
    }

    fn parse_explicit_title(&self, title: &str) -> Option<ExplicitTitlePayload> {
        let prefix = self.tab_title.explicit_prefix.trim();
        if prefix.is_empty() {
            return None;
        }

        let payload = title.strip_prefix(prefix)?.trim();
        if payload.is_empty() {
            return None;
        }

        if let Some(prompt) = payload.strip_prefix("prompt:") {
            let prompt = prompt.trim();
            if prompt.is_empty() {
                return None;
            }
            return Some(ExplicitTitlePayload::Prompt(Self::resolve_template(
                &self.tab_title.prompt_format,
                Some(prompt),
                None,
            )));
        }

        if let Some(command) = payload.strip_prefix("command:") {
            let command = command.trim();
            if command.is_empty() {
                return None;
            }
            return Some(ExplicitTitlePayload::Command(Self::resolve_template(
                &self.tab_title.command_format,
                None,
                Some(command),
            )));
        }

        let explicit = payload.strip_prefix("title:").unwrap_or(payload).trim();
        if explicit.is_empty() {
            return None;
        }

        Some(ExplicitTitlePayload::Title(explicit.to_string()))
    }

    fn resolved_tab_title(&self, index: usize) -> String {
        let tab = &self.tabs[index];

        for source in &self.tab_title.priority {
            let candidate = match source {
                TabTitleSource::Manual => tab.manual_title.as_deref(),
                TabTitleSource::Explicit => tab.explicit_title.as_deref(),
                TabTitleSource::Shell => tab.shell_title.as_deref(),
                TabTitleSource::Fallback => Some(self.fallback_title()),
            };

            if let Some(candidate) = candidate.map(str::trim).filter(|value| !value.is_empty()) {
                return Self::truncate_tab_title(candidate);
            }
        }

        Self::truncate_tab_title(self.fallback_title())
    }

    fn refresh_tab_title(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        let next = self.resolved_tab_title(index);
        if self.tabs[index].title == next {
            return false;
        }

        self.tabs[index].title = next;
        true
    }

    fn cancel_pending_command_title(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }

        let tab = &mut self.tabs[index];
        tab.pending_command_token = tab.pending_command_token.wrapping_add(1);
        tab.pending_command_title = None;
    }

    fn set_explicit_title(&mut self, index: usize, explicit_title: String) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        let explicit_title = Self::truncate_tab_title(&explicit_title);
        if self.tabs[index].explicit_title.as_deref() == Some(explicit_title.as_str()) {
            return false;
        }

        self.tabs[index].explicit_title = Some(explicit_title);
        self.refresh_tab_title(index)
    }

    fn schedule_delayed_command_title(
        &mut self,
        index: usize,
        command_title: String,
        delay_ms: u64,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }

        let tab = &mut self.tabs[index];
        tab.pending_command_token = tab.pending_command_token.wrapping_add(1);
        tab.pending_command_title = Some(Self::truncate_tab_title(&command_title));
        let token = tab.pending_command_token;

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            smol::Timer::after(Duration::from_millis(delay_ms)).await;
            let _ = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    if view.activate_pending_command_title(index, token) {
                        cx.notify();
                    }
                })
            });
        })
        .detach();
    }

    fn activate_pending_command_title(&mut self, index: usize, token: u64) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        let tab = &mut self.tabs[index];
        if tab.pending_command_token != token {
            return false;
        }

        let Some(command_title) = tab.pending_command_title.take() else {
            return false;
        };

        if tab.explicit_title.as_deref() == Some(command_title.as_str()) {
            return false;
        }

        tab.explicit_title = Some(command_title);
        self.refresh_tab_title(index)
    }

    fn apply_terminal_title(&mut self, index: usize, title: &str, cx: &mut Context<Self>) -> bool {
        let title = title.trim();
        if title.is_empty() || index >= self.tabs.len() {
            return false;
        }

        if let Some(explicit_payload) = self.parse_explicit_title(title) {
            return match explicit_payload {
                ExplicitTitlePayload::Prompt(prompt_title)
                | ExplicitTitlePayload::Title(prompt_title) => {
                    self.cancel_pending_command_title(index);
                    self.set_explicit_title(index, prompt_title)
                }
                ExplicitTitlePayload::Command(command_title) => {
                    self.schedule_delayed_command_title(
                        index,
                        command_title,
                        COMMAND_TITLE_DELAY_MS,
                        cx,
                    );
                    false
                }
            };
        }

        let shell_title = Self::truncate_tab_title(title);
        if self.tabs[index].shell_title.as_deref() == Some(shell_title.as_str()) {
            return false;
        }

        self.tabs[index].shell_title = Some(shell_title);
        self.refresh_tab_title(index)
    }

    fn clear_terminal_titles(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        self.cancel_pending_command_title(index);
        let tab = &mut self.tabs[index];
        let had_shell = tab.shell_title.take().is_some();
        let had_explicit = tab.explicit_title.take().is_some();
        if !had_shell && !had_explicit {
            return false;
        }

        self.refresh_tab_title(index)
    }

    fn tab_bar_layout(&self, viewport_width: f32) -> TabBarLayout {
        let tab_count = self.tabs.len().max(1) as f32;
        let total_gap = (self.tabs.len().saturating_sub(1) as f32) * TAB_PILL_GAP;
        let available =
            (viewport_width - (TAB_HORIZONTAL_PADDING * 2.0) - total_gap).max(tab_count);
        let tab_pill_width = (available / tab_count).max(1.0);

        TabBarLayout {
            tab_pill_width,
            tab_padding_x: Self::tab_pill_padding_x(tab_pill_width),
            slot_width: tab_pill_width + TAB_PILL_GAP,
        }
    }

    fn tab_pill_padding_x(tab_pill_width: f32) -> f32 {
        if tab_pill_width >= TAB_PILL_COMPACT_THRESHOLD {
            TAB_PILL_NORMAL_PADDING
        } else {
            TAB_PILL_COMPACT_PADDING
        }
    }

    fn tab_shows_close(tab_pill_width: f32, is_active: bool, tab_padding_x: f32) -> bool {
        // Keep close affordance visible on the active tab whenever there is
        // enough room for the hit target and side padding.
        let min_hit_target_width = TAB_CLOSE_HITBOX + (tab_padding_x * 2.0);
        if tab_pill_width < min_hit_target_width {
            return false;
        }

        if is_active {
            return true;
        }

        tab_pill_width >= TAB_INACTIVE_CLOSE_MIN_WIDTH
    }

    fn add_tab(&mut self, cx: &mut Context<Self>) {
        if !self.use_tabs {
            return;
        }

        let terminal = Terminal::new(
            TerminalSize::default(),
            self.configured_working_dir.as_deref(),
            Some(self.event_wakeup_tx.clone()),
            Some(&self.tab_shell_integration),
        )
        .expect("Failed to create terminal tab");

        self.tabs.push(TerminalTab::new(terminal));
        self.active_tab = self.tabs.len() - 1;
        self.refresh_tab_title(self.active_tab);
        self.renaming_tab = None;
        self.rename_buffer.clear();
        self.clear_selection();
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return;
        }

        self.tabs.remove(index);

        if self.active_tab > index {
            self.active_tab -= 1;
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }

        match self.renaming_tab {
            Some(editing) if editing == index => {
                self.renaming_tab = None;
                self.rename_buffer.clear();
            }
            Some(editing) if editing > index => {
                self.renaming_tab = Some(editing - 1);
            }
            _ => {}
        }

        self.clear_selection();
        cx.notify();
    }

    fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        self.close_tab(self.active_tab, cx);
    }

    fn switch_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() || index == self.active_tab {
            return;
        }

        self.active_tab = index;
        self.renaming_tab = None;
        self.rename_buffer.clear();
        self.clear_selection();
        cx.notify();
    }

    fn commit_rename_tab(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.renaming_tab else {
            return;
        };

        let trimmed = self.rename_buffer.trim();
        self.tabs[index].manual_title = (!trimmed.is_empty())
            .then(|| Self::truncate_tab_title(trimmed))
            .filter(|title| !title.is_empty());
        self.refresh_tab_title(index);

        self.renaming_tab = None;
        self.rename_buffer.clear();
        cx.notify();
    }

    fn cancel_rename_tab(&mut self, cx: &mut Context<Self>) {
        if self.renaming_tab.is_none() {
            return;
        }

        self.renaming_tab = None;
        self.rename_buffer.clear();
        cx.notify();
    }

    fn has_selection(&self) -> bool {
        matches!((self.selection_anchor, self.selection_head), (Some(anchor), Some(head)) if self.selection_moved || anchor != head)
    }

    fn selection_range(&self) -> Option<(CellPos, CellPos)> {
        if !self.has_selection() {
            return None;
        }

        let (anchor, head) = (self.selection_anchor?, self.selection_head?);
        if (head.row, head.col) < (anchor.row, anchor.col) {
            Some((head, anchor))
        } else {
            Some((anchor, head))
        }
    }

    fn cell_is_selected(&self, col: usize, row: usize) -> bool {
        let Some((start, end)) = self.selection_range() else {
            return false;
        };

        let here = (row, col);
        here >= (start.row, start.col) && here <= (end.row, end.col)
    }

    fn is_copy_shortcut(key: &str, modifiers: gpui::Modifiers) -> bool {
        #[cfg(target_os = "macos")]
        {
            modifiers.platform
                && !modifiers.control
                && !modifiers.alt
                && !modifiers.function
                && key.eq_ignore_ascii_case("c")
        }
        #[cfg(not(target_os = "macos"))]
        {
            modifiers.control
                && modifiers.shift
                && !modifiers.alt
                && !modifiers.function
                && key.eq_ignore_ascii_case("c")
        }
    }

    fn is_paste_shortcut(key: &str, modifiers: gpui::Modifiers) -> bool {
        #[cfg(target_os = "macos")]
        {
            modifiers.platform
                && !modifiers.control
                && !modifiers.alt
                && !modifiers.function
                && key.eq_ignore_ascii_case("v")
        }
        #[cfg(not(target_os = "macos"))]
        {
            modifiers.control
                && modifiers.shift
                && !modifiers.alt
                && !modifiers.function
                && key.eq_ignore_ascii_case("v")
        }
    }

    fn position_to_cell(&self, position: gpui::Point<Pixels>, clamp: bool) -> Option<CellPos> {
        let size = self.active_terminal().size();
        if size.cols == 0 || size.rows == 0 {
            return None;
        }

        let mut x: f32 = position.x.into();
        let mut y: f32 = position.y.into();
        x -= self.padding_x;
        y -= self.chrome_height() + self.padding_y;

        let cell_width: f32 = size.cell_width.into();
        let cell_height: f32 = size.cell_height.into();
        if cell_width <= 0.0 || cell_height <= 0.0 {
            return None;
        }

        let mut col = (x / cell_width).floor() as i32;
        let mut row = (y / cell_height).floor() as i32;

        let max_col = i32::from(size.cols) - 1;
        let max_row = i32::from(size.rows) - 1;
        if max_col < 0 || max_row < 0 {
            return None;
        }

        if clamp {
            col = col.clamp(0, max_col);
            row = row.clamp(0, max_row);
        } else if col < 0 || col > max_col || row < 0 || row > max_row {
            return None;
        }

        Some(CellPos {
            col: col as usize,
            row: row as usize,
        })
    }

    fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        let size = self.active_terminal().size();
        let cols = size.cols as usize;
        let rows = size.rows as usize;
        if cols == 0 || rows == 0 {
            return None;
        }

        let mut grid = vec![vec![' '; cols]; rows];
        self.active_terminal().with_term(|term| {
            let content = term.renderable_content();
            for cell in content.display_iter {
                let row = cell.point.line.0;
                if row < 0 {
                    continue;
                }

                let row = row as usize;
                let col = cell.point.column.0;
                if row >= rows || col >= cols {
                    continue;
                }

                let c = cell.cell.c;
                if c != '\0' {
                    grid[row][col] = if c.is_control() { ' ' } else { c };
                }
            }
        });

        let mut lines = Vec::new();
        for row in start.row..=end.row {
            let col_start = if row == start.row { start.col } else { 0 };
            let col_end = if row == end.row {
                end.col
            } else {
                cols.saturating_sub(1)
            };
            let mut line: String = grid[row][col_start..=col_end].iter().collect();
            while line.ends_with(' ') {
                line.pop();
            }
            lines.push(line);
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    fn row_text(&self, row: usize) -> Option<Vec<char>> {
        let size = self.active_terminal().size();
        let cols = size.cols as usize;
        let rows = size.rows as usize;
        if cols == 0 || row >= rows {
            return None;
        }

        let mut line = vec![' '; cols];
        self.active_terminal().with_term(|term| {
            let content = term.renderable_content();
            for cell in content.display_iter {
                let cell_row = cell.point.line.0;
                if cell_row < 0 || cell_row as usize != row {
                    continue;
                }

                let col = cell.point.column.0;
                if col >= cols {
                    continue;
                }

                if cell.cell.flags.intersects(
                    Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER | Flags::HIDDEN,
                ) {
                    continue;
                }

                let c = cell.cell.c;
                if c != '\0' {
                    line[col] = if c.is_control() { ' ' } else { c };
                }
            }
        });

        Some(line)
    }

    fn edge_trim_char(c: char) -> bool {
        matches!(
            c,
            '\''
                | '"'
                | '`'
                | ','
                | '.'
                | ';'
                | '!'
                | '?'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
        )
    }

    fn classify_link_token(token: &str) -> Option<String> {
        if token.is_empty() {
            return None;
        }

        let lower = token.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Some(token.to_string());
        }

        if lower.starts_with("www.") {
            return Some(format!("https://{}", token));
        }

        if Self::is_ipv4_with_optional_port_and_path(token) || Self::looks_like_domain(token) {
            return Some(format!("http://{}", token));
        }

        None
    }

    fn is_ipv4_with_optional_port_and_path(input: &str) -> bool {
        let host_port = input.split('/').next().unwrap_or(input);
        let (host, port) = if let Some((host, port)) = host_port.rsplit_once(':') {
            (host, Some(port))
        } else {
            (host_port, None)
        };

        let octets: Vec<&str> = host.split('.').collect();
        if octets.len() != 4 {
            return false;
        }
        if octets
            .iter()
            .any(|octet| octet.is_empty() || octet.parse::<u8>().is_err())
        {
            return false;
        }

        if let Some(port) = port {
            if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
            if port.parse::<u16>().is_err() {
                return false;
            }
        }

        true
    }

    fn looks_like_domain(input: &str) -> bool {
        let host_port = input.split('/').next().unwrap_or(input);
        let (host, port) = if let Some((host, port)) = host_port.rsplit_once(':') {
            (host, Some(port))
        } else {
            (host_port, None)
        };

        if host.eq_ignore_ascii_case("localhost") {
            return true;
        }

        if !host.contains('.') {
            return false;
        }

        for label in host.split('.') {
            if label.is_empty() {
                return false;
            }
            if label.starts_with('-') || label.ends_with('-') {
                return false;
            }
            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return false;
            }
        }

        if let Some(port) = port {
            if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
            if port.parse::<u16>().is_err() {
                return false;
            }
        }

        true
    }

    fn link_at_cell(&self, cell: CellPos) -> Option<HoveredLink> {
        let line = self.row_text(cell.row)?;
        if cell.col >= line.len() || line[cell.col].is_whitespace() {
            return None;
        }

        let mut start = cell.col;
        while start > 0 && !line[start - 1].is_whitespace() {
            start -= 1;
        }

        let mut end = cell.col;
        while end + 1 < line.len() && !line[end + 1].is_whitespace() {
            end += 1;
        }

        while start <= end && Self::edge_trim_char(line[start]) {
            start += 1;
        }
        while end >= start && Self::edge_trim_char(line[end]) {
            if end == 0 {
                break;
            }
            end -= 1;
        }

        if start > end {
            return None;
        }

        let token: String = line[start..=end].iter().collect();
        let target = Self::classify_link_token(token.trim_end_matches(':'))?;

        Some(HoveredLink {
            row: cell.row,
            start_col: start,
            end_col: end,
            target,
        })
    }

    fn open_link(url: &str) {
        #[cfg(target_os = "macos")]
        let _ = Command::new("open").arg(url).status();
        #[cfg(target_os = "linux")]
        let _ = Command::new("xdg-open").arg(url).status();
        #[cfg(target_os = "windows")]
        let _ = Command::new("cmd").args(["/C", "start", "", url]).status();
    }

    fn is_link_modifier(modifiers: gpui::Modifiers) -> bool {
        modifiers.secondary() && !modifiers.alt && !modifiers.function
    }

    fn update_zoom(&mut self, next_size: f32, cx: &mut Context<Self>) {
        let clamped = next_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        let current: f32 = self.font_size.into();
        if (current - clamped).abs() < f32::EPSILON {
            return;
        }

        self.font_size = px(clamped);
        // Force cell size recalc so terminal grid reflows at the new zoom.
        self.cell_size = None;
        cx.notify();
    }

    fn calculate_cell_size(&self, window: &mut Window, _cx: &App) -> Size<Pixels> {
        if let Some(cell_size) = self.cell_size {
            return cell_size;
        }

        let font = Font {
            family: self.font_family.clone(),
            weight: FontWeight::NORMAL,
            ..Default::default()
        };

        // Measure 'M' character width for monospace
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&font);
        let cell_width = text_system
            .advance(font_id, self.font_size, 'M')
            .map(|advance| advance.width)
            .unwrap_or(px(9.0));

        let cell_height = self.font_size * self.line_height;

        Size {
            width: cell_width,
            height: cell_height,
        }
    }

    fn sync_terminal_size(&mut self, window: &Window, cell_size: Size<Pixels>) {
        let viewport = window.viewport_size();
        let viewport_width: f32 = viewport.width.into();
        let viewport_height: f32 = viewport.height.into();
        let cell_width: f32 = cell_size.width.into();
        let cell_height: f32 = cell_size.height.into();

        if cell_width <= 0.0 || cell_height <= 0.0 {
            return;
        }

        let terminal_width = (viewport_width - (self.padding_x * 2.0)).max(cell_width * 2.0);
        let terminal_height =
            (viewport_height - self.chrome_height() - (self.padding_y * 2.0)).max(cell_height);
        let cols = (terminal_width / cell_width).floor().max(2.0) as u16;
        let rows = (terminal_height / cell_height).floor().max(1.0) as u16;

        let current = self.active_terminal().size();
        if current.cols != cols || current.rows != rows {
            self.active_terminal_mut().resize(TerminalSize {
                cols,
                rows,
                cell_width: cell_size.width,
                cell_height: cell_size.height,
            });
        }
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let key_char = event.keystroke.key_char.as_deref();

        if self.renaming_tab.is_some() {
            match key {
                "enter" => {
                    self.commit_rename_tab(cx);
                    return;
                }
                "escape" => {
                    self.cancel_rename_tab(cx);
                    return;
                }
                "backspace" => {
                    self.rename_buffer.pop();
                    cx.notify();
                    return;
                }
                "space"
                    if !modifiers.control
                        && !modifiers.alt
                        && !modifiers.function
                        && !modifiers.platform =>
                {
                    if self.rename_buffer.chars().count() < MAX_TAB_TITLE_CHARS {
                        self.rename_buffer.push(' ');
                        cx.notify();
                    }
                    return;
                }
                _ if key.len() == 1
                    && key_char.is_some()
                    && !modifiers.control
                    && !modifiers.alt
                    && !modifiers.function
                    && !modifiers.platform =>
                {
                    if let Some(input) = key_char {
                        let input_len = input.chars().count();
                        if input_len > 0
                            && self.rename_buffer.chars().count() + input_len <= MAX_TAB_TITLE_CHARS
                        {
                            self.rename_buffer.push_str(input);
                            cx.notify();
                        }
                    }
                    return;
                }
                _ => return,
            }
        }

        if Self::is_copy_shortcut(key, modifiers) {
            if let Some(selected) = self.selected_text() {
                cx.write_to_clipboard(ClipboardItem::new_string(selected));
            }
            return;
        }

        if Self::is_paste_shortcut(key, modifiers) {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.active_terminal().write(text.as_bytes());
                self.clear_selection();
                cx.notify();
            }
            return;
        }

        if self.use_tabs
            && modifiers.secondary()
            && !modifiers.alt
            && !modifiers.function
            && key.eq_ignore_ascii_case("w")
        {
            self.close_active_tab(cx);
            return;
        }

        if self.use_tabs
            && modifiers.secondary()
            && !modifiers.alt
            && !modifiers.function
            && key.eq_ignore_ascii_case("t")
        {
            self.add_tab(cx);
            return;
        }

        if modifiers.secondary() && !modifiers.alt && !modifiers.function {
            let current: f32 = self.font_size.into();
            match key {
                "=" | "+" | "plus" => {
                    self.update_zoom(current + ZOOM_STEP, cx);
                    return;
                }
                "-" | "_" | "minus" => {
                    self.update_zoom(current - ZOOM_STEP, cx);
                    return;
                }
                "0" => {
                    self.update_zoom(self.base_font_size, cx);
                    return;
                }
                _ => {}
            }
        }

        if let Some(input) = keystroke_to_input(&event.keystroke) {
            self.active_terminal().write(&input);
            self.clear_selection();
            // Request a redraw to show the typed character
            cx.notify();
        }
    }

    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Focus the terminal on click
        self.focus_handle.focus(window, cx);

        if event.button != MouseButton::Left {
            return;
        }

        if Self::is_link_modifier(event.modifiers) {
            if let Some(cell) = self.position_to_cell(event.position, false) {
                if let Some(link) = self.link_at_cell(cell) {
                    Self::open_link(&link.target);
                    if self.clear_hovered_link() {
                        cx.notify();
                    }
                    return;
                }
            }
        }

        let Some(cell) = self.position_to_cell(event.position, false) else {
            self.clear_selection();
            self.clear_hovered_link();
            cx.notify();
            return;
        };

        self.selection_anchor = Some(cell);
        self.selection_head = Some(cell);
        self.selection_dragging = true;
        self.selection_moved = false;
        self.clear_hovered_link();
        cx.notify();
    }

    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selection_dragging || !event.dragging() {
            if Self::is_link_modifier(event.modifiers) {
                let next = self
                    .position_to_cell(event.position, false)
                    .and_then(|cell| self.link_at_cell(cell));
                if self.hovered_link != next {
                    self.hovered_link = next;
                    cx.notify();
                }
            } else if self.clear_hovered_link() {
                cx.notify();
            }
            return;
        }

        let Some(next_cell) = self.position_to_cell(event.position, true) else {
            return;
        };

        if self.selection_head != Some(next_cell) {
            self.selection_head = Some(next_cell);
            if self.selection_anchor != self.selection_head {
                self.selection_moved = true;
            }
            self.clear_hovered_link();
            cx.notify();
        }
    }

    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left || !self.selection_dragging {
            return;
        }

        if let Some(next_cell) = self.position_to_cell(event.position, true) {
            self.selection_head = Some(next_cell);
            if self.selection_anchor != self.selection_head {
                self.selection_moved = true;
            }
        }

        self.selection_dragging = false;
        if !self.selection_moved {
            self.clear_selection();
        }
        self.clear_hovered_link();
        cx.notify();
    }

    fn handle_tabbar_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left || !self.show_tab_bar() {
            return;
        }

        let mut x: f32 = event.position.x.into();
        x -= TAB_HORIZONTAL_PADDING;
        if x < 0.0 {
            return;
        }

        let viewport = window.viewport_size();
        let layout = self.tab_bar_layout(viewport.width.into());
        let index = (x / layout.slot_width).floor() as usize;
        if index >= self.tabs.len() {
            return;
        }

        let x_in_slot = x - (index as f32 * layout.slot_width);
        if x_in_slot > layout.tab_pill_width {
            return;
        }

        let is_active = index == self.active_tab;
        let show_close =
            Self::tab_shows_close(layout.tab_pill_width, is_active, layout.tab_padding_x);
        if show_close {
            let close_left =
                (layout.tab_pill_width - layout.tab_padding_x - TAB_CLOSE_HITBOX).max(0.0);
            let close_right = (layout.tab_pill_width - layout.tab_padding_x).max(0.0);
            if x_in_slot >= close_left && x_in_slot <= close_right {
                self.close_tab(index, cx);
                return;
            }
        }

        self.switch_tab(index, cx);
    }

    fn handle_titlebar_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }

        if self.use_tabs && !cfg!(target_os = "windows") {
            let viewport = window.viewport_size();
            let viewport_width: f32 = viewport.width.into();
            let x: f32 = event.position.x.into();
            let y: f32 = event.position.y.into();
            let plus_left = viewport_width - TITLEBAR_SIDE_PADDING - TITLEBAR_PLUS_SIZE;
            let plus_top = (self.titlebar_height() - TITLEBAR_PLUS_SIZE) * 0.5;
            let plus_right = plus_left + TITLEBAR_PLUS_SIZE;
            let plus_bottom = plus_top + TITLEBAR_PLUS_SIZE;

            if x >= plus_left && x <= plus_right && y >= plus_top && y <= plus_bottom {
                self.add_tab(cx);
                return;
            }
        }

        if event.click_count == 2 {
            #[cfg(target_os = "macos")]
            window.titlebar_double_click();
            #[cfg(not(target_os = "macos"))]
            window.zoom_window();
            return;
        }

        window.start_window_move();
    }

    fn tab_bar_height(&self) -> f32 {
        if self.show_tab_bar() {
            TABBAR_HEIGHT
        } else {
            0.0
        }
    }

    fn titlebar_height(&self) -> f32 {
        #[cfg(target_os = "windows")]
        {
            0.0
        }
        #[cfg(not(target_os = "windows"))]
        {
            TITLEBAR_HEIGHT
        }
    }

    fn chrome_height(&self) -> f32 {
        self.titlebar_height() + self.tab_bar_height()
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cell_size = self.calculate_cell_size(window, cx);
        let colors = self.colors.clone();
        let font_family = self.font_family.clone();
        let font_size = self.font_size;

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

        div()
            .id("termy-root")
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .child(
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
                    .bg(titlebar_bg)
                    .border_b(px(if titlebar_height > 0.0 { 1.0 } else { 0.0 }))
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
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(titlebar_plus_bg)
                                    .text_color(titlebar_plus_text)
                                    .text_size(px(16.0))
                                    .child(if show_titlebar_plus { "+" } else { "" })
                            }),
                    ),
            )
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
                    .font_family(font_family.clone())
                    .text_size(font_size)
                    .child(TerminalGrid {
                        cells: cells_to_render,
                        cell_size,
                        cols: terminal_size.cols as usize,
                        rows: terminal_size.rows as usize,
                        cursor_color: colors.cursor.into(),
                        selection_bg: selection_bg.into(),
                        selection_fg: selection_fg.into(),
                        hovered_link_range,
                        font_family,
                        font_size,
                    }),
            )
    }
}
