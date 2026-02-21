use gpui::{KeyBinding, actions};

const GLOBAL_CONTEXT: Option<&str> = None;
const TERMINAL_CONTEXT: Option<&str> = Some("Terminal");

macro_rules! define_command_actions {
    ($(($variant:ident, $config_name:literal, $context:expr)),+ $(,)?) => {
        actions!(
            termy,
            [$( $variant, )+]
        );

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum CommandAction {
            $( $variant, )+
        }

        impl CommandAction {
            pub fn all() -> &'static [Self] {
                const ALL: &[CommandAction] = &[
                    $(CommandAction::$variant,)+
                ];
                ALL
            }

            pub fn config_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $config_name,)+
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
                    $(Self::$variant => KeyBinding::new(trigger, $variant, $context),)+
                }
            }
        }
    };
}

define_command_actions!(
    (Quit, "quit", GLOBAL_CONTEXT),
    (OpenConfig, "open_config", GLOBAL_CONTEXT),
    (AppInfo, "app_info", TERMINAL_CONTEXT),
    (RestartApp, "restart_app", TERMINAL_CONTEXT),
    (RenameTab, "rename_tab", TERMINAL_CONTEXT),
    (CheckForUpdates, "check_for_updates", TERMINAL_CONTEXT),
    (
        ToggleCommandPalette,
        "toggle_command_palette",
        TERMINAL_CONTEXT
    ),
    (NewTab, "new_tab", TERMINAL_CONTEXT),
    (CloseTab, "close_tab", TERMINAL_CONTEXT),
    (Copy, "copy", TERMINAL_CONTEXT),
    (Paste, "paste", TERMINAL_CONTEXT),
    (ZoomIn, "zoom_in", TERMINAL_CONTEXT),
    (ZoomOut, "zoom_out", TERMINAL_CONTEXT),
    (ZoomReset, "zoom_reset", TERMINAL_CONTEXT),
);
