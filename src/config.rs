use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const DEFAULT_CONFIG: &str = "# Will be comments using #\n\
theme = tokyonight\n";

#[derive(Debug, Clone, Copy)]
pub enum Theme {
    TokyoNight,
    Catppuccin,
    Dracula,
    GruvboxDark,
    Nord,
    SolarizedDark,
}

impl Theme {
    fn from_str(value: &str) -> Option<Self> {
        let mut normalized = value.trim().to_ascii_lowercase();
        normalized.retain(|c| c != ' ' && c != '-' && c != '_');
        match normalized.as_str() {
            "tokyonight" => Some(Self::TokyoNight),
            "catppuccin" => Some(Self::Catppuccin),
            "dracula" => Some(Self::Dracula),
            "gruvbox" | "gruvboxdark" => Some(Self::GruvboxDark),
            "nord" => Some(Self::Nord),
            "solarizeddark" | "solarized" => Some(Self::SolarizedDark),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AppConfig {
    pub theme: Theme,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: Theme::TokyoNight,
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
        }

        config
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
