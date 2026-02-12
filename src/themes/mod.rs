mod catppuccin_mocha;
mod dracula;
mod gruvbox_dark;
mod nord;
mod solarized_dark;
mod tokyo_night;

use gpui::Rgba;

#[derive(Clone, Copy, Debug)]
pub struct ThemeColors {
    pub ansi: [Rgba; 16],
    pub foreground: Rgba,
    pub background: Rgba,
    pub cursor: Rgba,
}

pub fn tokyo_night() -> ThemeColors {
    tokyo_night::theme()
}

pub fn catppuccin_mocha() -> ThemeColors {
    catppuccin_mocha::theme()
}

pub fn dracula() -> ThemeColors {
    dracula::theme()
}

pub fn gruvbox_dark() -> ThemeColors {
    gruvbox_dark::theme()
}

pub fn nord() -> ThemeColors {
    nord::theme()
}

pub fn solarized_dark() -> ThemeColors {
    solarized_dark::theme()
}

fn rgba(r: u8, g: u8, b: u8) -> Rgba {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}
