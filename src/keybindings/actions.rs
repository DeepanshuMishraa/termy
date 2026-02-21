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

const ALL_KEYBIND_ACTIONS: [KeybindAction; 10] = [
    KeybindAction::Quit,
    KeybindAction::OpenConfig,
    KeybindAction::ToggleCommandPalette,
    KeybindAction::NewTab,
    KeybindAction::CloseTab,
    KeybindAction::Copy,
    KeybindAction::Paste,
    KeybindAction::ZoomIn,
    KeybindAction::ZoomOut,
    KeybindAction::ZoomReset,
];

impl KeybindAction {
    pub fn all() -> &'static [Self] {
        &ALL_KEYBIND_ACTIONS
    }

    pub fn config_name(self) -> &'static str {
        match self {
            Self::Quit => "quit",
            Self::OpenConfig => "open_config",
            Self::ToggleCommandPalette => "toggle_command_palette",
            Self::NewTab => "new_tab",
            Self::CloseTab => "close_tab",
            Self::Copy => "copy",
            Self::Paste => "paste",
            Self::ZoomIn => "zoom_in",
            Self::ZoomOut => "zoom_out",
            Self::ZoomReset => "zoom_reset",
        }
    }

    pub fn from_config_name(name: &str) -> Option<Self> {
        let normalized = name.trim().to_ascii_lowercase().replace('-', "_");
        Self::all()
            .iter()
            .copied()
            .find(|action| action.config_name() == normalized)
    }

    pub fn all_config_names() -> impl std::iter::ExactSizeIterator<Item = &'static str> {
        Self::all().iter().copied().map(Self::config_name)
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
