use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Rgb as AnsiRgb};
use gpui::Rgba;
use termy_themes as themes;

/// Default terminal color palette (based on typical terminal colors)
#[derive(Clone)]
pub struct TerminalColors {
    /// Standard 16 ANSI colors
    pub ansi: [Rgba; 16],
    /// Default foreground color
    pub foreground: Rgba,
    /// Default background color
    pub background: Rgba,
    /// Cursor color
    pub cursor: Rgba,
}

impl Default for TerminalColors {
    fn default() -> Self {
        Self {
            ansi: [
                // Normal colors (0-7)
                rgba(0x00, 0x00, 0x00), // Black
                rgba(0xCD, 0x00, 0x00), // Red
                rgba(0x00, 0xCD, 0x00), // Green
                rgba(0xCD, 0xCD, 0x00), // Yellow
                rgba(0x00, 0x00, 0xEE), // Blue
                rgba(0xCD, 0x00, 0xCD), // Magenta
                rgba(0x00, 0xCD, 0xCD), // Cyan
                rgba(0xE5, 0xE5, 0xE5), // White
                // Bright colors (8-15)
                rgba(0x7F, 0x7F, 0x7F), // Bright Black (Gray)
                rgba(0xFF, 0x00, 0x00), // Bright Red
                rgba(0x00, 0xFF, 0x00), // Bright Green
                rgba(0xFF, 0xFF, 0x00), // Bright Yellow
                rgba(0x5C, 0x5C, 0xFF), // Bright Blue
                rgba(0xFF, 0x00, 0xFF), // Bright Magenta
                rgba(0x00, 0xFF, 0xFF), // Bright Cyan
                rgba(0xFF, 0xFF, 0xFF), // Bright White
            ],
            foreground: rgba(0xE5, 0xE5, 0xE5),
            background: rgba(0x1E, 0x1E, 0x1E),
            cursor: rgba(0xFF, 0xFF, 0xFF),
        }
    }
}

impl TerminalColors {
    pub fn from_theme(theme: &str) -> Self {
        let theme_colors = themes::resolve_theme(theme).unwrap_or_else(themes::termy);

        Self::from_theme_colors(theme_colors)
    }

    fn from_theme_colors(theme: themes::ThemeColors) -> Self {
        Self {
            ansi: theme.ansi,
            foreground: theme.foreground,
            background: theme.background,
            cursor: theme.cursor,
        }
    }

    /// Convert an alacritty ANSI color to a GPUI Rgba
    pub fn convert(&self, color: AnsiColor) -> Rgba {
        match color {
            AnsiColor::Named(named) => self.named_color(named),
            AnsiColor::Spec(AnsiRgb { r, g, b }) => rgba(r, g, b),
            AnsiColor::Indexed(idx) => self.indexed_color(idx),
        }
    }

    fn named_color(&self, color: NamedColor) -> Rgba {
        match color {
            NamedColor::Black => self.ansi[0],
            NamedColor::Red => self.ansi[1],
            NamedColor::Green => self.ansi[2],
            NamedColor::Yellow => self.ansi[3],
            NamedColor::Blue => self.ansi[4],
            NamedColor::Magenta => self.ansi[5],
            NamedColor::Cyan => self.ansi[6],
            NamedColor::White => self.ansi[7],
            NamedColor::BrightBlack => self.ansi[8],
            NamedColor::BrightRed => self.ansi[9],
            NamedColor::BrightGreen => self.ansi[10],
            NamedColor::BrightYellow => self.ansi[11],
            NamedColor::BrightBlue => self.ansi[12],
            NamedColor::BrightMagenta => self.ansi[13],
            NamedColor::BrightCyan => self.ansi[14],
            NamedColor::BrightWhite => self.ansi[15],
            NamedColor::Foreground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            _ => self.foreground,
        }
    }

    fn indexed_color(&self, idx: u8) -> Rgba {
        match idx {
            // Standard ANSI colors
            0..=15 => self.ansi[idx as usize],
            // 216 color cube (6x6x6)
            16..=231 => {
                let idx = idx - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                let to_component = |c: u8| if c == 0 { 0 } else { 55 + c * 40 };
                rgba(to_component(r), to_component(g), to_component(b))
            }
            // Grayscale (24 shades)
            232..=255 => {
                let gray = 8 + (idx - 232) * 10;
                rgba(gray, gray, gray)
            }
        }
    }
}

/// Helper to create Rgba from u8 components
fn rgba(r: u8, g: u8, b: u8) -> Rgba {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}
