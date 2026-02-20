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
}

/// Custom element for rendering the terminal grid.
pub struct TerminalGrid {
    pub cells: Vec<CellRenderInfo>,
    pub cell_size: Size<Pixels>,
    pub cols: usize,
    pub rows: usize,
    pub cursor_color: Hsla,
    pub selection_bg: Hsla,
    pub selection_fg: Hsla,
    pub hovered_link_range: Option<(usize, usize, usize)>,
    pub font_family: SharedString,
    pub font_size: Pixels,
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

        // Paint background colors and cursor first.
        for cell in &self.cells {
            let x = origin.x + self.cell_size.width * cell.col as f32;
            let y = origin.y + self.cell_size.height * cell.row as f32;

            let cell_bounds = Bounds {
                origin: point(x, y),
                size: self.cell_size,
            };

            if cell.is_cursor {
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

        for cell in &self.cells {
            if !cell.render_text || cell.char == ' ' || cell.char == '\0' || cell.char.is_control()
            {
                continue;
            }

            let x = origin.x + self.cell_size.width * cell.col as f32;
            let y = origin.y + self.cell_size.height * cell.row as f32;

            let fg_color = if cell.is_cursor {
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
