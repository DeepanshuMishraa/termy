use gpui::{
    App, Bounds, Element, Font, FontWeight, Hsla, IntoElement, Pixels, SharedString, Size,
    TextAlign, TextRun, UnderlineStyle, Window, point, px, quad,
};

/// Info needed to render a single cell.
#[derive(Clone)]
pub struct CellRenderInfo {
    pub col: usize,
    pub row: usize,
    pub char: char,
    pub fg: Hsla,
    pub bg: Hsla,
    pub bold: bool,
    pub render_text: bool,
    pub is_cursor: bool,
    pub selected: bool,
    /// Part of the current (focused) search match
    pub search_current: bool,
    /// Part of any search match (but not current)
    pub search_match: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalCursorStyle {
    Line,
    Block,
}

/// Custom element for rendering the terminal grid.
pub struct TerminalGrid {
    pub cells: Vec<CellRenderInfo>,
    pub cell_size: Size<Pixels>,
    pub cols: usize,
    pub rows: usize,
    pub default_bg: Hsla,
    pub cursor_color: Hsla,
    pub selection_bg: Hsla,
    pub selection_fg: Hsla,
    pub search_match_bg: Hsla,
    pub search_current_bg: Hsla,
    pub hovered_link_range: Option<(usize, usize, usize)>,
    pub font_family: SharedString,
    pub font_size: Pixels,
    pub cursor_style: TerminalCursorStyle,
}

impl IntoElement for TerminalGrid {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Check if two HSLA colors are approximately equal.
/// This is used to avoid painting cell backgrounds that match the terminal's default background,
/// which can cause visual artifacts due to slight color differences between ANSI colors.
fn colors_approximately_equal(a: &Hsla, b: &Hsla) -> bool {
    const EPSILON: f32 = 0.02;
    (a.h - b.h).abs() < EPSILON
        && (a.s - b.s).abs() < EPSILON
        && (a.l - b.l).abs() < EPSILON
        && (a.a - b.a).abs() < EPSILON
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
        let grid_bounds = Bounds {
            origin,
            size: bounds.size,
        };

        // Always clear the full terminal surface first to avoid ghosting artifacts
        // when scrolled content reveals previously untouched cells.
        window.paint_quad(quad(
            grid_bounds,
            px(0.0),
            self.default_bg,
            gpui::Edges::default(),
            Hsla::transparent_black(),
            gpui::BorderStyle::default(),
        ));

        // Paint background colors and cursor first.
        for cell in &self.cells {
            let x = origin.x + self.cell_size.width * cell.col as f32;
            let y = origin.y + self.cell_size.height * cell.row as f32;

            let cell_bounds = Bounds {
                origin: point(x, y),
                size: self.cell_size,
            };

            if cell.selected {
                window.paint_quad(quad(
                    cell_bounds,
                    px(0.0),
                    self.selection_bg,
                    gpui::Edges::default(),
                    Hsla::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            } else if cell.search_current {
                window.paint_quad(quad(
                    cell_bounds,
                    px(0.0),
                    self.search_current_bg,
                    gpui::Edges::default(),
                    Hsla::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            } else if cell.search_match {
                window.paint_quad(quad(
                    cell_bounds,
                    px(0.0),
                    self.search_match_bg,
                    gpui::Edges::default(),
                    Hsla::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            } else if cell.bg.a > 0.01
                && !colors_approximately_equal(&cell.bg, &self.default_bg)
            {
                window.paint_quad(quad(
                    cell_bounds,
                    px(0.0),
                    cell.bg,
                    gpui::Edges::default(),
                    Hsla::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            }

            if cell.is_cursor {
                let cursor_bounds = match self.cursor_style {
                    TerminalCursorStyle::Block => cell_bounds,
                    TerminalCursorStyle::Line => {
                        let cell_width: f32 = self.cell_size.width.into();
                        let cursor_width = px(cell_width.clamp(1.0, 2.0));
                        Bounds::new(
                            cell_bounds.origin,
                            Size {
                                width: cursor_width,
                                height: cell_bounds.size.height,
                            },
                        )
                    }
                };

                window.paint_quad(quad(
                    cursor_bounds,
                    px(0.0),
                    self.cursor_color,
                    gpui::Edges::default(),
                    Hsla::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            }
        }

        // Pre-create font structs to avoid cloning font_family for every cell
        let font_normal = Font {
            family: self.font_family.clone(),
            weight: FontWeight::NORMAL,
            ..Default::default()
        };
        let font_bold = Font {
            family: self.font_family.clone(),
            weight: FontWeight::BOLD,
            ..Default::default()
        };

        // Pre-compute cursor foreground color (black on cursor block)
        let cursor_fg = Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 1.0,
        };

        for cell in &self.cells {
            if !cell.render_text || cell.char == ' ' || cell.char == '\0' || cell.char.is_control()
            {
                continue;
            }

            let x = origin.x + self.cell_size.width * cell.col as f32;
            let y = origin.y + self.cell_size.height * cell.row as f32;

            let fg_color = if cell.is_cursor && self.cursor_style == TerminalCursorStyle::Block {
                cursor_fg
            } else if cell.selected {
                self.selection_fg
            } else {
                cell.fg
            };

            let text: SharedString = cell.char.to_string().into();
            let font = if cell.bold { &font_bold } else { &font_normal };

            let run = TextRun {
                len: text.len(),
                font: font.clone(),
                color: fg_color,
                background_color: None,
                underline: self
                    .hovered_link_range
                    .and_then(|(row, start_col, end_col)| {
                        if cell.row == row && cell.col >= start_col && cell.col <= end_col {
                            Some(UnderlineStyle {
                                thickness: px(1.0),
                                color: Some(fg_color),
                                wavy: false,
                            })
                        } else {
                            None
                        }
                    }),
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
