use crate::colors::TerminalColors;
use crate::config::AppConfig;
use crate::terminal::{Terminal, TerminalEvent, TerminalSize, keystroke_to_input};
use alacritty_terminal::term::cell::Flags;
use gpui::{
    App, AsyncApp, Bounds, ClipboardItem, Context, Element, FocusHandle, Focusable, Font,
    FontWeight, Hsla, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, SharedString, Size, Styled,
    TextAlign, TextRun, WeakEntity, Window, WindowControlArea, div, point, px, quad,
};
use std::time::Duration;

const MIN_FONT_SIZE: f32 = 8.0;
const MAX_FONT_SIZE: f32 = 40.0;
const ZOOM_STEP: f32 = 1.0;
const TITLEBAR_HEIGHT: f32 = 34.0;
const SELECTION_BG_ALPHA: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellPos {
    col: usize,
    row: usize,
}

/// The main terminal view component
pub struct TerminalView {
    terminal: Terminal,
    focus_handle: FocusHandle,
    colors: TerminalColors,
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

        // Focus the terminal immediately
        focus_handle.focus(window, cx);

        // Start a timer to poll for terminal events
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                smol::Timer::after(Duration::from_millis(16)).await;
                let result = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        let events = view.terminal.process_events();
                        let should_redraw =
                            events.iter().any(|e| matches!(e, TerminalEvent::Wakeup));
                        if should_redraw {
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

        Self {
            terminal: Terminal::new(TerminalSize::default(), config.working_dir.as_deref())
                .expect("Failed to create terminal"),
            focus_handle,
            colors,
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

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_head = None;
        self.selection_dragging = false;
        self.selection_moved = false;
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
        let size = self.terminal.size();
        if size.cols == 0 || size.rows == 0 {
            return None;
        }

        let mut x: f32 = position.x.into();
        let mut y: f32 = position.y.into();
        x -= self.padding_x;
        y -= TITLEBAR_HEIGHT + self.padding_y;

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
        let size = self.terminal.size();
        let cols = size.cols as usize;
        let rows = size.rows as usize;
        if cols == 0 || rows == 0 {
            return None;
        }

        let mut grid = vec![vec![' '; cols]; rows];
        self.terminal.with_term(|term| {
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
            (viewport_height - TITLEBAR_HEIGHT - (self.padding_y * 2.0)).max(cell_height);
        let cols = (terminal_width / cell_width).floor().max(2.0) as u16;
        let rows = (terminal_height / cell_height).floor().max(1.0) as u16;

        let current = self.terminal.size();
        if current.cols != cols || current.rows != rows {
            self.terminal.resize(TerminalSize {
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

        if Self::is_copy_shortcut(key, modifiers) {
            if let Some(selected) = self.selected_text() {
                cx.write_to_clipboard(ClipboardItem::new_string(selected));
            }
            return;
        }

        if Self::is_paste_shortcut(key, modifiers) {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.terminal.write(text.as_bytes());
                self.clear_selection();
                cx.notify();
            }
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

        if let Some(input) = keystroke_to_input(key, modifiers) {
            self.terminal.write(&input);
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

    fn handle_titlebar_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
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
        let (cursor_col, cursor_row) = self.terminal.cursor_position();

        self.terminal.with_term(|term| {
            let content = term.renderable_content();
            for cell in content.display_iter {
                let point = cell.point;
                let cell_content = &cell.cell;

                // Get foreground and background colors
                let fg = colors.convert(cell_content.fg);
                let bg = colors.convert(cell_content.bg);

                let c = cell_content.c;
                let is_cursor = point.column.0 == cursor_col && point.line.0 as usize == cursor_row;
                let selected = self.cell_is_selected(point.column.0, point.line.0 as usize);

                cells_to_render.push(CellRenderInfo {
                    col: point.column.0,
                    row: point.line.0 as usize,
                    char: c,
                    fg: fg.into(),
                    bg: bg.into(),
                    bold: cell_content.flags.contains(Flags::BOLD),
                    render_text: !cell_content
                        .flags
                        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER),
                    _italic: cell_content.flags.contains(Flags::ITALIC),
                    is_cursor,
                    selected,
                });
            }
        });

        let terminal_size = self.terminal.size();
        let focus_handle = self.focus_handle.clone();
        let mut titlebar_bg = colors.background;
        titlebar_bg.a = 0.96;
        let mut titlebar_border = colors.cursor;
        titlebar_border.a = 0.18;
        let mut titlebar_text = colors.foreground;
        titlebar_text.a = 0.82;
        let mut selection_bg = colors.cursor;
        selection_bg.a = SELECTION_BG_ALPHA;
        let selection_fg = colors.background;

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
                    .justify_center()
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(Self::handle_titlebar_mouse_down),
                    )
                    .bg(titlebar_bg)
                    .border_b(px(1.0))
                    .border_color(titlebar_border)
                    .text_color(titlebar_text)
                    .text_size(px(12.0))
                    .child("Termy"),
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
