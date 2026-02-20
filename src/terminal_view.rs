use crate::colors::TerminalColors;
use crate::config::AppConfig;
use crate::terminal::{Terminal, TerminalEvent, TerminalSize, keystroke_to_input};
use alacritty_terminal::term::cell::Flags;
use flume::{Sender, bounded};
use gpui::{
    App, AsyncApp, Bounds, ClipboardItem, Context, Element, FocusHandle, Focusable, Font,
    FontWeight, Hsla, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, SharedString, Size, Styled,
    TextAlign, TextRun, WeakEntity, Window, WindowControlArea, div, point, px, quad,
};

const MIN_FONT_SIZE: f32 = 8.0;
const MAX_FONT_SIZE: f32 = 40.0;
const ZOOM_STEP: f32 = 1.0;
const TITLEBAR_HEIGHT: f32 = 34.0;
const TABBAR_HEIGHT: f32 = 40.0;
const TITLEBAR_PLUS_SIZE: f32 = 22.0;
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
    title: String,
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
    configured_working_dir: Option<String>,
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
    /// Cached cell dimensions
    cell_size: Option<Size<Pixels>>,
}

impl TerminalView {
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
                        if view.process_terminal_events() {
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
        let colors = TerminalColors::from_theme(config.theme);
        let base_font_size = config.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        let padding_x = config.padding_x.max(0.0);
        let padding_y = config.padding_y.max(0.0);
        let configured_working_dir = config.working_dir.clone();
        let terminal = Terminal::new(
            TerminalSize::default(),
            configured_working_dir.as_deref(),
            Some(event_wakeup_tx.clone()),
        )
        .expect("Failed to create terminal");

        Self {
            tabs: vec![TerminalTab {
                terminal,
                title: "Terminal".to_string(),
            }],
            active_tab: 0,
            renaming_tab: None,
            rename_buffer: String::new(),
            event_wakeup_tx,
            focus_handle,
            colors,
            use_tabs: config.use_tabs,
            configured_working_dir,
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
            cell_size: None,
        }
    }

    fn process_terminal_events(&mut self) -> bool {
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
                        let title = title.trim();
                        if !title.is_empty() && self.use_tabs {
                            let next = Self::truncate_tab_title(title);
                            if self.tabs[index].title != next && self.renaming_tab != Some(index) {
                                self.tabs[index].title = next;
                                if self.show_tab_bar() {
                                    should_redraw = true;
                                }
                            }
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
        )
        .expect("Failed to create terminal tab");

        self.tabs.push(TerminalTab {
            terminal,
            title: "Terminal".to_string(),
        });
        self.active_tab = self.tabs.len() - 1;
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

    fn begin_rename_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }

        self.active_tab = index;
        self.renaming_tab = Some(index);
        self.rename_buffer = self.tabs[index].title.clone();
        self.clear_selection();
        cx.notify();
    }

    fn commit_rename_tab(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.renaming_tab else {
            return;
        };

        let trimmed = self.rename_buffer.trim();
        if !trimmed.is_empty() {
            self.tabs[index].title = Self::truncate_tab_title(trimmed);
        }

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

        let Some(cell) = self.position_to_cell(event.position, false) else {
            self.clear_selection();
            cx.notify();
            return;
        };

        self.selection_anchor = Some(cell);
        self.selection_head = Some(cell);
        self.selection_dragging = true;
        self.selection_moved = false;
        cx.notify();
    }

    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selection_dragging || !event.dragging() {
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

        if event.click_count >= 2 {
            self.begin_rename_tab(index, cx);
            return;
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

        if self.use_tabs {
            let viewport = window.viewport_size();
            let viewport_width: f32 = viewport.width.into();
            let x: f32 = event.position.x.into();
            let y: f32 = event.position.y.into();
            let plus_left = viewport_width - TITLEBAR_SIDE_PADDING - TITLEBAR_PLUS_SIZE;
            let plus_top = (TITLEBAR_HEIGHT - TITLEBAR_PLUS_SIZE) * 0.5;
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

    fn chrome_height(&self) -> f32 {
        TITLEBAR_HEIGHT + self.tab_bar_height()
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
                    _italic: cell_content.flags.contains(Flags::ITALIC),
                    is_cursor,
                    selected,
                });
            }
        });

        let terminal_size = self.active_terminal().size();
        let focus_handle = self.focus_handle.clone();
        let show_tab_bar = self.show_tab_bar();
        let show_titlebar_plus = self.use_tabs;
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
        let viewport = window.viewport_size();
        let tab_layout = self.tab_bar_layout(viewport.width.into());

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
                    .h(px(TITLEBAR_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(Self::handle_titlebar_mouse_down),
                    )
                    .bg(titlebar_bg)
                    .border_b(px(1.0))
                    .border_color(titlebar_border)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .px(px(TITLEBAR_SIDE_PADDING))
                            .child(div().w(px(TITLEBAR_PLUS_SIZE)).h(px(TITLEBAR_PLUS_SIZE)))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_center()
                                    .text_color(titlebar_text)
                                    .text_size(px(12.0))
                                    .child("Termy"),
                            )
                            .child(
                                div()
                                    .w(px(TITLEBAR_PLUS_SIZE))
                                    .h(px(TITLEBAR_PLUS_SIZE))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(titlebar_plus_bg)
                                    .text_color(titlebar_plus_text)
                                    .text_size(px(16.0))
                                    .child(if show_titlebar_plus { "+" } else { "" }),
                            ),
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
                        font_family,
                        font_size,
                    }),
            )
    }
}

/// Info needed to render a single cell
#[derive(Clone)]
struct CellRenderInfo {
    col: usize,
    row: usize,
    char: char,
    fg: Hsla,
    bg: Hsla,
    bold: bool,
    render_text: bool,
    _italic: bool,
    is_cursor: bool,
    selected: bool,
}

/// Custom element for rendering the terminal grid
struct TerminalGrid {
    cells: Vec<CellRenderInfo>,
    cell_size: Size<Pixels>,
    cols: usize,
    rows: usize,
    cursor_color: Hsla,
    selection_bg: Hsla,
    selection_fg: Hsla,
    font_family: SharedString,
    font_size: Pixels,
}

impl IntoElement for TerminalGrid {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalGrid {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let width = self.cell_size.width * self.cols as f32;
        let height = self.cell_size.height * self.rows as f32;

        let layout_id = window.request_layout(
            gpui::Style {
                size: gpui::Size {
                    width: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                        gpui::AbsoluteLength::Pixels(width),
                    )),
                    height: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                        gpui::AbsoluteLength::Pixels(height),
                    )),
                },
                ..Default::default()
            },
            [],
            cx,
        );

        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = bounds.origin;

        // Paint background colors and cursor first
        for cell in &self.cells {
            let x = origin.x + self.cell_size.width * cell.col as f32;
            let y = origin.y + self.cell_size.height * cell.row as f32;

            // Draw background if not default
            let cell_bounds = Bounds {
                origin: point(x, y),
                size: self.cell_size,
            };

            if cell.is_cursor {
                // Draw cursor
                window.paint_quad(quad(
                    cell_bounds,
                    px(0.0),
                    self.cursor_color,
                    gpui::Edges::default(),
                    Hsla::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            } else if cell.selected {
                window.paint_quad(quad(
                    cell_bounds,
                    px(0.0),
                    self.selection_bg,
                    gpui::Edges::default(),
                    Hsla::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            } else if cell.bg.a > 0.01 {
                // Draw cell background
                window.paint_quad(quad(
                    cell_bounds,
                    px(0.0),
                    cell.bg,
                    gpui::Edges::default(),
                    Hsla::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            }
        }

        // Paint text
        for cell in &self.cells {
            if !cell.render_text || cell.char == ' ' || cell.char == '\0' || cell.char.is_control()
            {
                continue;
            }

            let x = origin.x + self.cell_size.width * cell.col as f32;
            let y = origin.y + self.cell_size.height * cell.row as f32;

            let fg_color = if cell.is_cursor {
                // Invert color for cursor
                Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.0,
                    a: 1.0,
                }
            } else if cell.selected {
                self.selection_fg
            } else {
                cell.fg
            };

            let text: SharedString = cell.char.to_string().into();
            let font_weight = if cell.bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            };

            let font = Font {
                family: self.font_family.clone(),
                weight: font_weight,
                ..Default::default()
            };

            let run = TextRun {
                len: text.len(),
                font,
                color: fg_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };

            let line = window
                .text_system()
                .shape_line(text, self.font_size, &[run], None);
            let _ = line.paint(
                point(x, y),
                self.cell_size.height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
}
