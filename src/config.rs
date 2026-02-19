use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const DEFAULT_CONFIG: &str = "# Will be comments using #\n\
theme = termy\n\
# Startup directory for new terminal sessions\n\
# working_dir = ~/Documents\n\
# Show tab bar above the terminal grid\n\
# use_tabs = true\n\
# Startup window size in pixels\n\
window_width = 1100\n\
window_height = 720\n\
# Terminal font family\n\
font_family = JetBrains Mono\n\
# Terminal font size in pixels\n\
font_size = 14\n\
# Inner terminal padding in pixels\n\
padding_x = 12\n\
padding_y = 8\n";

#[derive(Debug, Clone, Copy)]
pub enum Theme {
    Termy,
    TokyoNight,
    Catppuccin,
    Dracula,
    GruvboxDark,
    Nord,
    SolarizedDark,
    OneDark,
    Monokai,
    MaterialDark,
    Palenight,
    TomorrowNight,
    OceanicNext,
}

impl Theme {
    fn from_str(value: &str) -> Option<Self> {
        let mut normalized = value.trim().to_ascii_lowercase();
        normalized.retain(|c| c != ' ' && c != '-' && c != '_');
        match normalized.as_str() {
            "termy" | "default" => Some(Self::Termy),
            "tokyonight" => Some(Self::TokyoNight),
            "catppuccin" => Some(Self::Catppuccin),
            "dracula" => Some(Self::Dracula),
            "gruvbox" | "gruvboxdark" => Some(Self::GruvboxDark),
            "nord" => Some(Self::Nord),
            "solarizeddark" | "solarized" => Some(Self::SolarizedDark),
            "onedark" | "one" => Some(Self::OneDark),
            "monokai" => Some(Self::Monokai),
            "materialdark" | "material" => Some(Self::MaterialDark),
            "palenight" => Some(Self::Palenight),
            "tomorrownight" | "tomorrow" => Some(Self::TomorrowNight),
            "oceanicnext" | "oceanic" => Some(Self::OceanicNext),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub theme: Theme,
    pub working_dir: Option<String>,
    pub use_tabs: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub font_family: String,
    pub font_size: f32,
    pub padding_x: f32,
    pub padding_y: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Termy,
            working_dir: None,
            use_tabs: false,
            window_width: 1100.0,
            window_height: 720.0,
            font_family: "JetBrains Mono".to_string(),
            font_size: 14.0,
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
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();

            if key.eq_ignore_ascii_case("theme") {
                if let Some(theme) = Theme::from_str(value) {
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

            if key.eq_ignore_ascii_case("font_family") && !value.is_empty() {
                let trimmed = value.trim();
                let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"'))
                    || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
                {
                    &trimmed[1..trimmed.len() - 1]
                } else {
                    trimmed
                };

                if !unquoted.is_empty() {
                    config.font_family = unquoted.to_string();
                }
            }

            if key.eq_ignore_ascii_case("font_size") {
                if let Ok(font_size) = value.parse::<f32>() {
                    if font_size > 0.0 {
                        config.font_size = font_size;
                    }
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
    if let Ok(home) = env::var("HOME") {
        return Some(Path::new(&home).join(".config/termy/config.txt"));
    }

    env::current_dir()
        .ok()
        .map(|dir| dir.join(".config/termy/config.txt"))
}
