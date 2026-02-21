use gpui::{KeyBinding, actions};

actions!(
    termy,
    [
        Quit,
        OpenConfig,
        ToggleCommandPalette,
        NewTab,
        CloseTab,
        Copy,
        Paste,
        ZoomIn,
        ZoomOut,
        ZoomReset,
    ]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeybindAction {
    Quit,
    OpenConfig,
    ToggleCommandPalette,
    NewTab,
    CloseTab,
    Copy,
    Paste,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

impl KeybindAction {
    pub fn from_config_name(name: &str) -> Option<Self> {
        let normalized = name.trim().to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "quit" => Some(Self::Quit),
            "open_config" => Some(Self::OpenConfig),
            "toggle_command_palette" => Some(Self::ToggleCommandPalette),
            "new_tab" => Some(Self::NewTab),
            "close_tab" => Some(Self::CloseTab),
            "copy" => Some(Self::Copy),
            "paste" => Some(Self::Paste),
            "zoom_in" => Some(Self::ZoomIn),
            "zoom_out" => Some(Self::ZoomOut),
            "zoom_reset" => Some(Self::ZoomReset),
            _ => None,
        }
    }

    pub fn all_config_names() -> &'static [&'static str] {
        &[
            "quit",
            "open_config",
            "toggle_command_palette",
            "new_tab",
            "close_tab",
            "copy",
            "paste",
            "zoom_in",
            "zoom_out",
            "zoom_reset",
        ]
    }

    pub fn to_key_binding(self, trigger: &str) -> KeyBinding {
        match self {
            Self::Quit => KeyBinding::new(trigger, Quit, None),
            Self::OpenConfig => KeyBinding::new(trigger, OpenConfig, None),
            Self::ToggleCommandPalette => {
                KeyBinding::new(trigger, ToggleCommandPalette, Some("Terminal"))
            }
            Self::NewTab => KeyBinding::new(trigger, NewTab, Some("Terminal")),
            Self::CloseTab => KeyBinding::new(trigger, CloseTab, Some("Terminal")),
            Self::Copy => KeyBinding::new(trigger, Copy, Some("Terminal")),
            Self::Paste => KeyBinding::new(trigger, Paste, Some("Terminal")),
            Self::ZoomIn => KeyBinding::new(trigger, ZoomIn, Some("Terminal")),
            Self::ZoomOut => KeyBinding::new(trigger, ZoomOut, Some("Terminal")),
            Self::ZoomReset => KeyBinding::new(trigger, ZoomReset, Some("Terminal")),
        }
    }
}
