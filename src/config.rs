use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const DEFAULT_TAB_TITLE_FALLBACK: &str = "Terminal";
const DEFAULT_TAB_TITLE_EXPLICIT_PREFIX: &str = "termy:tab:";
const DEFAULT_TAB_TITLE_PROMPT_FORMAT: &str = "{cwd}";
const DEFAULT_TAB_TITLE_COMMAND_FORMAT: &str = "{command}";

const DEFAULT_CONFIG: &str = "# Will be comments using #\n\
theme = termy\n\
# Startup directory for new terminal sessions\n\
# On Windows, leaving this unset defaults to your user home directory.\n\
# working_dir = ~/Documents\n\
# Show tab bar above the terminal grid\n\
# use_tabs = true\n\
# Tab title mode. Supported values: smart, shell, explicit, static\n\
# smart = manual rename > explicit title > shell/app title > fallback\n\
tab_title_mode = smart\n\
# Export TERMY_* env vars for optional shell tab-title integration\n\
tab_title_shell_integration = true\n\
# Optional: static fallback tab title\n\
# tab_title_fallback = Terminal\n\
# Advanced tab-title options are documented in docs/configuration.md:\n\
# tab_title_priority = manual, explicit, shell, fallback\n\
# tab_title_explicit_prefix = termy:tab:\n\
# tab_title_prompt_format = {cwd}\n\
# tab_title_command_format = {command}\n\
# Startup window size in pixels\n\
window_width = 1280\n\
window_height = 820\n\
# Terminal font family\n\
font_family = JetBrains Mono\n\
# Terminal font size in pixels\n\
font_size = 14\n\
# Terminal background opacity (0.0 = fully transparent, 1.0 = opaque)\n\
# transparent_background_opacity = 1.0\n\
# Inner terminal padding in pixels\n\
padding_x = 12\n\
padding_y = 8\n";

pub type ThemeId = String;

const DEFAULT_THEME_ID: &str = "termy";

fn parse_theme_id(value: &str) -> Option<ThemeId> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Some(canonical) = termy_themes::canonical_builtin_theme_id(value) {
        return Some(canonical.to_string());
    }

    let normalized = termy_themes::normalize_theme_id(value);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabTitleSource {
    Manual,
    Explicit,
    Shell,
    Fallback,
}

impl TabTitleSource {
    fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "manual" => Some(Self::Manual),
            "explicit" => Some(Self::Explicit),
            "shell" | "app" | "terminal" => Some(Self::Shell),
            "fallback" | "default" => Some(Self::Fallback),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabTitleMode {
    Smart,
    Shell,
    Explicit,
    Static,
}

impl TabTitleMode {
    fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "smart" => Some(Self::Smart),
            "shell" => Some(Self::Shell),
            "explicit" => Some(Self::Explicit),
            "static" => Some(Self::Static),
            _ => None,
        }
    }

    fn default_priority(self) -> Vec<TabTitleSource> {
        match self {
            Self::Smart => vec![
                TabTitleSource::Manual,
                TabTitleSource::Explicit,
                TabTitleSource::Shell,
                TabTitleSource::Fallback,
            ],
            Self::Shell => vec![
                TabTitleSource::Manual,
                TabTitleSource::Shell,
                TabTitleSource::Fallback,
            ],
            Self::Explicit => vec![
                TabTitleSource::Manual,
                TabTitleSource::Explicit,
                TabTitleSource::Fallback,
            ],
            Self::Static => vec![TabTitleSource::Manual, TabTitleSource::Fallback],
        }
    }
}

#[derive(Debug, Clone)]
pub struct TabTitleConfig {
    pub mode: TabTitleMode,
    pub priority: Vec<TabTitleSource>,
    pub fallback: String,
    pub explicit_prefix: String,
    pub shell_integration: bool,
    pub prompt_format: String,
    pub command_format: String,
}

impl Default for TabTitleConfig {
    fn default() -> Self {
        Self {
            mode: TabTitleMode::Smart,
            priority: TabTitleMode::Smart.default_priority(),
            fallback: DEFAULT_TAB_TITLE_FALLBACK.to_string(),
            explicit_prefix: DEFAULT_TAB_TITLE_EXPLICIT_PREFIX.to_string(),
            shell_integration: true,
            prompt_format: DEFAULT_TAB_TITLE_PROMPT_FORMAT.to_string(),
            command_format: DEFAULT_TAB_TITLE_COMMAND_FORMAT.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub theme: ThemeId,
    pub working_dir: Option<String>,
    pub use_tabs: bool,
    pub tab_title: TabTitleConfig,
    pub window_width: f32,
    pub window_height: f32,
    pub font_family: String,
    pub font_size: f32,
    pub transparent_background_opacity: f32,
    pub padding_x: f32,
    pub padding_y: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME_ID.to_string(),
            working_dir: None,
            use_tabs: false,
            tab_title: TabTitleConfig::default(),
            window_width: 1280.0,
            window_height: 820.0,
            font_family: "JetBrains Mono".to_string(),
            font_size: 14.0,
            transparent_background_opacity: 1.0,
            padding_x: 12.0,
            padding_y: 8.0,
        }
    }
}

impl AppConfig {
    pub fn load_or_create() -> Self {
        let mut config = Self::default();
        let Some(path) = ensure_config_file() else {
            return config;
        };

        if let Ok(contents) = fs::read_to_string(&path) {
            config = Self::from_contents(&contents);
        }

        config
    }

    fn from_contents(contents: &str) -> Self {
        let mut config = Self::default();
        let mut tab_title_priority_overridden = false;
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();

            if key.eq_ignore_ascii_case("theme") {
                if let Some(theme) = parse_theme_id(value) {
                    config.theme = theme;
                }
            }

            if key.eq_ignore_ascii_case("working_dir") && !value.is_empty() {
                config.working_dir = Some(value.to_string());
            }

            if key.eq_ignore_ascii_case("use_tabs") {
                if let Some(use_tabs) = parse_bool(value) {
                    config.use_tabs = use_tabs;
                }
            }

            if key.eq_ignore_ascii_case("tab_title_priority") {
                if let Some(priority) = parse_tab_title_priority(value) {
                    config.tab_title.priority = priority;
                    tab_title_priority_overridden = true;
                }
            }

            if key.eq_ignore_ascii_case("tab_title_mode") {
                if let Some(mode) = TabTitleMode::from_str(value) {
                    config.tab_title.mode = mode;
                }
            }

            if key.eq_ignore_ascii_case("tab_title_fallback") {
                if let Some(fallback) = parse_string_value(value) {
                    config.tab_title.fallback = fallback;
                }
            }

            if key.eq_ignore_ascii_case("tab_title_explicit_prefix") {
                if let Some(prefix) = parse_string_value(value) {
                    config.tab_title.explicit_prefix = prefix;
                }
            }

            if key.eq_ignore_ascii_case("tab_title_shell_integration") {
                if let Some(enabled) = parse_bool(value) {
                    config.tab_title.shell_integration = enabled;
                }
            }

            if key.eq_ignore_ascii_case("tab_title_prompt_format") {
                if let Some(format) = parse_string_value(value) {
                    config.tab_title.prompt_format = format;
                }
            }

            if key.eq_ignore_ascii_case("tab_title_command_format") {
                if let Some(format) = parse_string_value(value) {
                    config.tab_title.command_format = format;
                }
            }

            if key.eq_ignore_ascii_case("window_width") {
                if let Ok(window_width) = value.parse::<f32>() {
                    if window_width > 0.0 {
                        config.window_width = window_width;
                    }
                }
            }

            if key.eq_ignore_ascii_case("window_height") {
                if let Ok(window_height) = value.parse::<f32>() {
                    if window_height > 0.0 {
                        config.window_height = window_height;
                    }
                }
            }

            if key.eq_ignore_ascii_case("font_family") {
                if let Some(font_family) = parse_string_value(value) {
                    config.font_family = font_family;
                }
            }

            if key.eq_ignore_ascii_case("font_size") {
                if let Ok(font_size) = value.parse::<f32>() {
                    if font_size > 0.0 {
                        config.font_size = font_size;
                    }
                }
            }

            if key.eq_ignore_ascii_case("transparent_background_opacity")
                || key.eq_ignore_ascii_case("transparent_background_opccaity")
            {
                if let Ok(opacity) = value.parse::<f32>() {
                    config.transparent_background_opacity = opacity.clamp(0.0, 1.0);
                }
            }

            if key.eq_ignore_ascii_case("padding_x") {
                if let Ok(padding_x) = value.parse::<f32>() {
                    if padding_x >= 0.0 {
                        config.padding_x = padding_x;
                    }
                }
            }

            if key.eq_ignore_ascii_case("padding_y") {
                if let Ok(padding_y) = value.parse::<f32>() {
                    if padding_y >= 0.0 {
                        config.padding_y = padding_y;
                    }
                }
            }
        }

        if !tab_title_priority_overridden {
            config.tab_title.priority = config.tab_title.mode.default_priority();
        }

        config
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_string_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    let unquoted = unquoted.trim();
    if unquoted.is_empty() {
        return None;
    }

    Some(unquoted.to_string())
}

fn parse_tab_title_priority(value: &str) -> Option<Vec<TabTitleSource>> {
    let mut priority = Vec::new();
    for token in value.split(',') {
        let Some(source) = TabTitleSource::from_str(token) else {
            continue;
        };

        if !priority.contains(&source) {
            priority.push(source);
        }
    }

    if priority.is_empty() {
        return None;
    }

    Some(priority)
}

pub fn ensure_config_file() -> Option<PathBuf> {
    let path = config_path()?;
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, DEFAULT_CONFIG);
    }
    Some(path)
}

pub fn open_config_file() {
    let Some(path) = ensure_config_file() else {
        return;
    };

    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(&path).status();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(&path).status();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd")
            .args(["/C", "start", "", path.to_string_lossy().as_ref()])
            .status();
    }
}

fn config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = env::var("APPDATA")
            && !app_data.trim().is_empty()
        {
            return Some(Path::new(&app_data).join("termy").join("config.txt"));
        }

        if let Ok(user_profile) = env::var("USERPROFILE")
            && !user_profile.trim().is_empty()
        {
            return Some(Path::new(&user_profile).join(".config/termy/config.txt"));
        }
    }

    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME")
        && !xdg_config_home.trim().is_empty()
    {
        return Some(Path::new(&xdg_config_home).join("termy/config.txt"));
    }

    if let Ok(home) = env::var("HOME")
        && !home.trim().is_empty()
    {
        return Some(Path::new(&home).join(".config/termy/config.txt"));
    }

    env::current_dir()
        .ok()
        .map(|dir| dir.join(".config/termy/config.txt"))
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, TabTitleMode, TabTitleSource};

    #[test]
    fn tab_title_mode_sets_default_priority() {
        let config = AppConfig::from_contents(
            "tab_title_mode = static\n\
             tab_title_fallback = Session\n",
        );

        assert_eq!(config.tab_title.mode, TabTitleMode::Static);
        assert_eq!(
            config.tab_title.priority,
            vec![TabTitleSource::Manual, TabTitleSource::Fallback]
        );
        assert_eq!(config.tab_title.fallback, "Session");
    }

    #[test]
    fn tab_title_priority_overrides_mode() {
        let config = AppConfig::from_contents(
            "tab_title_mode = static\n\
             tab_title_priority = shell, explicit, fallback\n\
             tab_title_fallback = Session\n\
             tab_title_explicit_prefix = termy:custom:\n\
             tab_title_shell_integration = false\n\
             tab_title_prompt_format = cwd:{cwd}\n\
             tab_title_command_format = run:{command}\n",
        );

        assert_eq!(config.tab_title.mode, TabTitleMode::Static);
        assert_eq!(
            config.tab_title.priority,
            vec![
                TabTitleSource::Shell,
                TabTitleSource::Explicit,
                TabTitleSource::Fallback
            ]
        );
        assert_eq!(config.tab_title.fallback, "Session");
        assert_eq!(config.tab_title.explicit_prefix, "termy:custom:");
        assert!(!config.tab_title.shell_integration);
        assert_eq!(config.tab_title.prompt_format, "cwd:{cwd}");
        assert_eq!(config.tab_title.command_format, "run:{command}");
    }
}
