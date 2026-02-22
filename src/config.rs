use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const DEFAULT_TAB_TITLE_FALLBACK: &str = "Terminal";
const DEFAULT_TAB_TITLE_EXPLICIT_PREFIX: &str = "termy:tab:";
const DEFAULT_TAB_TITLE_PROMPT_FORMAT: &str = "{cwd}";
const DEFAULT_TAB_TITLE_COMMAND_FORMAT: &str = "{command}";
const DEFAULT_TERM: &str = "xterm-256color";
const DEFAULT_COLORTERM: &str = "truecolor";
const DEFAULT_MOUSE_SCROLL_MULTIPLIER: f32 = 3.0;
const DEFAULT_SCROLLBACK_HISTORY: usize = 2000;
const MAX_SCROLLBACK_HISTORY: usize = 100_000;
const DEFAULT_MAX_TABS: usize = 10;
const MAX_TABS_LIMIT: usize = 100;
const DEFAULT_INACTIVE_TAB_SCROLLBACK: Option<usize> = None;
const MIN_MOUSE_SCROLL_MULTIPLIER: f32 = 0.1;
const MAX_MOUSE_SCROLL_MULTIPLIER: f32 = 1_000.0;
const DEFAULT_CURSOR_BLINK: bool = true;

const DEFAULT_CONFIG: &str = "# Main settings\n\
theme = termy\n\
# TERM value for child shells and terminal apps\n\
term = xterm-256color\n\
# Startup directory for new terminal sessions (~ supported)\n\
# working_dir = ~/Documents\n\
# Show tab bar above the terminal grid\n\
# use_tabs = true\n\
# Maximum number of tabs (lower = less memory usage)\n\
# max_tabs = 10\n\
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
# Cursor style shared by terminal and inline inputs (line|block)\n\
# cursor_style = block\n\
# Enable cursor blink for terminal and inline inputs\n\
# cursor_blink = true\n\
# Terminal background opacity (0.0 = fully transparent, 1.0 = opaque)\n\
# transparent_background_opacity = 1.0\n\
# Inner terminal padding in pixels\n\
padding_x = 12\n\
padding_y = 8\n\
# Mouse wheel scroll speed multiplier\n\
# mouse_scroll_multiplier = 3\n\
\n\
# Advanced runtime settings (usually leave these as defaults)\n\
# Preferred shell executable path\n\
# shell = /bin/zsh\n\
# Fallback startup directory when working_dir is unset: home or process\n\
# working_dir_fallback = home\n\
# Advertise 24-bit color support to child apps\n\
# colorterm = truecolor\n\
# Scrollback history lines (lower = less memory, max 100000)\n\
# scrollback_history = 2000\n\
# Scrollback for inactive tabs (saves memory with many tabs)\n\
# inactive_tab_scrollback = 500\n\
# Keybindings (Ghostty-style trigger overrides)\n\
# keybind = cmd-p=toggle_command_palette\n\
# keybind = cmd-c=copy\n\
# keybind = cmd-c=unbind\n\
# keybind = clear\n\
# Show/hide shortcut badges in command palette\n\
# command_palette_show_keybinds = true\n";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Line,
    Block,
}

impl CursorStyle {
    fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "line" | "bar" | "beam" | "ibeam" => Some(Self::Line),
            "block" | "box" => Some(Self::Block),
            _ => None,
        }
    }
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::Block
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub theme: ThemeId,
    pub working_dir: Option<String>,
    pub working_dir_fallback: WorkingDirFallback,
    pub use_tabs: bool,
    pub max_tabs: usize,
    pub tab_title: TabTitleConfig,
    pub shell: Option<String>,
    pub term: String,
    pub colorterm: Option<String>,
    pub window_width: f32,
    pub window_height: f32,
    pub font_family: String,
    pub font_size: f32,
    pub cursor_style: CursorStyle,
    pub cursor_blink: bool,
    pub transparent_background_opacity: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub mouse_scroll_multiplier: f32,
    pub scrollback_history: usize,
    pub inactive_tab_scrollback: Option<usize>,
    pub command_palette_show_keybinds: bool,
    pub keybind_lines: Vec<KeybindConfigLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindConfigLine {
    pub line_number: usize,
    pub value: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME_ID.to_string(),
            working_dir: None,
            working_dir_fallback: WorkingDirFallback::default(),
            use_tabs: true,
            max_tabs: DEFAULT_MAX_TABS,
            tab_title: TabTitleConfig::default(),
            shell: None,
            term: DEFAULT_TERM.to_string(),
            colorterm: Some(DEFAULT_COLORTERM.to_string()),
            window_width: 1280.0,
            window_height: 820.0,
            font_family: "JetBrains Mono".to_string(),
            font_size: 14.0,
            cursor_style: CursorStyle::default(),
            cursor_blink: DEFAULT_CURSOR_BLINK,
            transparent_background_opacity: 1.0,
            padding_x: 12.0,
            padding_y: 8.0,
            mouse_scroll_multiplier: DEFAULT_MOUSE_SCROLL_MULTIPLIER,
            scrollback_history: DEFAULT_SCROLLBACK_HISTORY,
            inactive_tab_scrollback: DEFAULT_INACTIVE_TAB_SCROLLBACK,
            command_palette_show_keybinds: true,
            keybind_lines: Vec::new(),
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
        for (line_number, line) in contents.lines().enumerate() {
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

            if key.eq_ignore_ascii_case("working_dir_fallback")
                || key.eq_ignore_ascii_case("default_working_dir")
            {
                if let Some(fallback) = WorkingDirFallback::from_str(value) {
                    config.working_dir_fallback = fallback;
                }
            }

            if key.eq_ignore_ascii_case("use_tabs") {
                if let Some(use_tabs) = parse_bool(value) {
                    config.use_tabs = use_tabs;
                }
            }

            if key.eq_ignore_ascii_case("max_tabs") {
                if let Ok(max_tabs) = value.parse::<usize>() {
                    config.max_tabs = max_tabs.clamp(1, MAX_TABS_LIMIT);
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

            if key.eq_ignore_ascii_case("shell") {
                config.shell = parse_optional_string_value(value);
            }

            if key.eq_ignore_ascii_case("term") {
                if let Some(term) = parse_string_value(value) {
                    config.term = term;
                }
            }

            if key.eq_ignore_ascii_case("colorterm") {
                config.colorterm = parse_optional_string_value(value);
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

            if key.eq_ignore_ascii_case("cursor_style") {
                if let Some(cursor_style) = CursorStyle::from_str(value) {
                    config.cursor_style = cursor_style;
                }
            }

            if key.eq_ignore_ascii_case("cursor_blink") {
                if let Some(cursor_blink) = parse_bool(value) {
                    config.cursor_blink = cursor_blink;
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

            if key.eq_ignore_ascii_case("mouse_scroll_multiplier") {
                if let Ok(multiplier) = value.parse::<f32>()
                    && multiplier.is_finite()
                {
                    config.mouse_scroll_multiplier =
                        multiplier.clamp(MIN_MOUSE_SCROLL_MULTIPLIER, MAX_MOUSE_SCROLL_MULTIPLIER);
                }
            }

            if key.eq_ignore_ascii_case("scrollback_history")
                || key.eq_ignore_ascii_case("scrollback")
            {
                if let Ok(history) = value.parse::<usize>() {
                    config.scrollback_history = history.min(MAX_SCROLLBACK_HISTORY);
                }
            }

            if key.eq_ignore_ascii_case("inactive_tab_scrollback") {
                if let Ok(history) = value.parse::<usize>() {
                    config.inactive_tab_scrollback = Some(history.min(MAX_SCROLLBACK_HISTORY));
                }
            }

            if key.eq_ignore_ascii_case("command_palette_show_keybinds") {
                if let Some(show) = parse_bool(value) {
                    config.command_palette_show_keybinds = show;
                }
            }

            if key.eq_ignore_ascii_case("keybind")
                && let Some(raw) = parse_string_value(value)
            {
                config.keybind_lines.push(KeybindConfigLine {
                    line_number: line_number + 1,
                    value: raw,
                });
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

fn parse_optional_string_value(value: &str) -> Option<String> {
    let parsed = parse_string_value(value)?;
    let normalized = parsed.trim().to_ascii_lowercase();
    if matches!(normalized.as_str(), "none" | "unset" | "default" | "auto") {
        return None;
    }
    Some(parsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkingDirFallback {
    Home,
    Process,
}

impl WorkingDirFallback {
    fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "home" | "user" => Some(Self::Home),
            "process" | "cwd" => Some(Self::Process),
            _ => None,
        }
    }
}

impl Default for WorkingDirFallback {
    fn default() -> Self {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            Self::Home
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Self::Process
        }
    }
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
    use super::{AppConfig, CursorStyle, TabTitleMode, TabTitleSource, WorkingDirFallback};

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

    #[test]
    fn runtime_env_options_parse() {
        let config = AppConfig::from_contents(
            "term = screen-256color\n\
             shell = /bin/zsh\n\
             working_dir_fallback = process\n\
             colorterm = none\n",
        );

        assert_eq!(config.term, "screen-256color");
        assert_eq!(config.shell.as_deref(), Some("/bin/zsh"));
        assert_eq!(config.working_dir_fallback, WorkingDirFallback::Process);
        assert!(config.colorterm.is_none());
    }

    #[test]
    fn keybind_lines_are_collected_in_order_with_line_numbers() {
        let config = AppConfig::from_contents(
            "# ignore comments\n\
             keybind = cmd-p=toggle_command_palette\n\
             keybind = cmd-c=copy\n\
             keybind = cmd-c=unbind\n\
             keybind = clear\n",
        );

        assert_eq!(config.keybind_lines.len(), 4);
        assert_eq!(config.keybind_lines[0].line_number, 2);
        assert_eq!(
            config.keybind_lines[0].value,
            "cmd-p=toggle_command_palette"
        );
        assert_eq!(config.keybind_lines[1].line_number, 3);
        assert_eq!(config.keybind_lines[1].value, "cmd-c=copy");
        assert_eq!(config.keybind_lines[2].line_number, 4);
        assert_eq!(config.keybind_lines[2].value, "cmd-c=unbind");
        assert_eq!(config.keybind_lines[3].line_number, 5);
        assert_eq!(config.keybind_lines[3].value, "clear");
    }

    #[test]
    fn command_palette_show_keybinds_parses_and_defaults() {
        let defaults = AppConfig::from_contents("");
        assert!(defaults.command_palette_show_keybinds);

        let disabled = AppConfig::from_contents("command_palette_show_keybinds = false\n");
        assert!(!disabled.command_palette_show_keybinds);
    }

    #[test]
    fn mouse_scroll_multiplier_parses_and_clamps() {
        let defaults = AppConfig::from_contents("");
        assert_eq!(defaults.mouse_scroll_multiplier, 3.0);

        let custom = AppConfig::from_contents("mouse_scroll_multiplier = 2.5\n");
        assert_eq!(custom.mouse_scroll_multiplier, 2.5);

        let clamped_low = AppConfig::from_contents("mouse_scroll_multiplier = -1\n");
        assert_eq!(clamped_low.mouse_scroll_multiplier, 0.1);

        let clamped_high = AppConfig::from_contents("mouse_scroll_multiplier = 20000\n");
        assert_eq!(clamped_high.mouse_scroll_multiplier, 1_000.0);
    }

    #[test]
    fn cursor_style_and_blink_parse_and_default() {
        let defaults = AppConfig::from_contents("");
        assert_eq!(defaults.cursor_style, CursorStyle::Block);
        assert!(defaults.cursor_blink);

        let line = AppConfig::from_contents("cursor_style = line\n");
        assert_eq!(line.cursor_style, CursorStyle::Line);

        let line_alias = AppConfig::from_contents("cursor_style = bar\n");
        assert_eq!(line_alias.cursor_style, CursorStyle::Line);

        let block = AppConfig::from_contents("cursor_style = block\n");
        assert_eq!(block.cursor_style, CursorStyle::Block);

        let blink_disabled = AppConfig::from_contents("cursor_blink = false\n");
        assert!(!blink_disabled.cursor_blink);
    }

    #[test]
    fn scrollback_history_parses_and_clamps() {
        let defaults = AppConfig::from_contents("");
        assert_eq!(defaults.scrollback_history, 2000);

        let custom = AppConfig::from_contents("scrollback_history = 5000\n");
        assert_eq!(custom.scrollback_history, 5000);

        let alias = AppConfig::from_contents("scrollback = 3000\n");
        assert_eq!(alias.scrollback_history, 3000);

        let clamped_high = AppConfig::from_contents("scrollback_history = 200000\n");
        assert_eq!(clamped_high.scrollback_history, 100_000);
    }

    #[test]
    fn max_tabs_parses_and_clamps() {
        let defaults = AppConfig::from_contents("");
        assert_eq!(defaults.max_tabs, 10);

        let custom = AppConfig::from_contents("max_tabs = 5\n");
        assert_eq!(custom.max_tabs, 5);

        let clamped_low = AppConfig::from_contents("max_tabs = 0\n");
        assert_eq!(clamped_low.max_tabs, 1);

        let clamped_high = AppConfig::from_contents("max_tabs = 500\n");
        assert_eq!(clamped_high.max_tabs, 100);
    }
}
