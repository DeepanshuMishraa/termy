use std::path::PathBuf;

use crate::commands::list_keybinds::KeybindDirective;

/// Returns the path to the config file
pub fn config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|p| p.join("termy").join("config.txt"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir().map(|p| p.join(".config").join("termy").join("config.txt"))
    }
}

/// Parses keybind directives from config file contents
pub fn parse_keybind_lines(contents: &str) -> Vec<KeybindDirective> {
    let mut directives = Vec::new();

    for line in contents.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Look for keybind = ...
        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key != "keybind" {
                continue;
            }

            if value == "clear" {
                directives.push(KeybindDirective::Clear);
                continue;
            }

            // Parse trigger=action format
            if let Some((trigger, action)) = value.split_once('=') {
                let trigger = trigger.trim().to_string();
                let action = action.trim().to_string();

                if action == "unbind" {
                    directives.push(KeybindDirective::Unbind { trigger });
                } else {
                    directives.push(KeybindDirective::Bind { trigger, action });
                }
            }
        }
    }

    directives
}

/// Parses the theme ID from config file contents
pub fn parse_theme_id(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Stop if we hit a section header
        if trimmed.starts_with('[') {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key == "theme" {
                return Some(value.to_string());
            }
        }
    }

    None
}
