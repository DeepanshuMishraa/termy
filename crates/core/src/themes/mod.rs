mod catppuccin_mocha;
mod dracula;
mod gruvbox_dark;
mod material_dark;
mod monokai;
mod nord;
mod oceanic_next;
mod one_dark;
mod palenight;
mod solarized_dark;
mod termy;
mod termy_light;
mod tokyo_night;
mod tomorrow_night;

pub use crate::theme_core::{
    Rgb8, THEME_REGISTRY_CACHE_VERSION, ThemeColors, ThemeRegistryCache, ThemeRegistryEntry,
    ThemeRegistryIndex, ThemeStoreTheme, normalize_theme_id, parse_theme_colors_json,
    registry_file_url, theme_colors_json_pretty,
};

/// Themes every Termy host ships offline: core config, libtermy, the CLI and
/// the desktop app. Other palettes below are bundled only for embedders that
/// list them themselves (the web package); apps install them from the theme
/// store.
pub const BUILTIN_THEME_IDS: &[&str] = &["termy", "termy-light"];

/// The ids in [`BUILTIN_THEME_IDS`], kept for existing callers.
pub fn available_theme_ids() -> Vec<&'static str> {
    BUILTIN_THEME_IDS.to_vec()
}

/// Same as [`builtin_theme`], kept for existing callers.
pub fn resolve_theme(theme_id: &str) -> Option<ThemeColors> {
    builtin_theme(theme_id)
}

/// Colors of a [`BUILTIN_THEME_IDS`] theme; `termylight` is accepted for
/// `termy-light`.
pub fn builtin_theme(theme_id: &str) -> Option<ThemeColors> {
    match normalize_theme_id(theme_id).as_str() {
        "termy" => Some(termy()),
        "termy-light" | "termylight" => Some(termy_light()),
        _ => None,
    }
}

pub fn tokyo_night() -> ThemeColors {
    tokyo_night::theme()
}

pub fn termy() -> ThemeColors {
    termy::theme()
}

pub fn termy_light() -> ThemeColors {
    termy_light::theme()
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

pub fn one_dark() -> ThemeColors {
    one_dark::theme()
}

pub fn monokai() -> ThemeColors {
    monokai::theme()
}

pub fn material_dark() -> ThemeColors {
    material_dark::theme()
}

pub fn palenight() -> ThemeColors {
    palenight::theme()
}

pub fn tomorrow_night() -> ThemeColors {
    tomorrow_night::theme()
}

pub fn oceanic_next() -> ThemeColors {
    oceanic_next::theme()
}

fn rgba(r: u8, g: u8, b: u8) -> Rgb8 {
    Rgb8::new(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_theme_id_resolves() {
        for id in BUILTIN_THEME_IDS {
            assert!(builtin_theme(id).is_some(), "{id}");
            assert!(resolve_theme(id).is_some(), "{id}");
        }
        assert_eq!(available_theme_ids(), BUILTIN_THEME_IDS);
        assert!(builtin_theme("dracula").is_none());
    }

    #[test]
    fn termy_light_is_an_offline_builtin_with_a_light_background() {
        let colors = builtin_theme("termy-light").expect("termy-light builtin");

        assert_eq!(colors.foreground, Rgb8::new(0x09, 0x09, 0x0B));
        assert_eq!(colors.background, Rgb8::new(0xFA, 0xFA, 0xF9));
        assert_eq!(colors.cursor, Rgb8::new(0x4F, 0xA8, 0x4A));
    }
}
