use crate::commands::CommandAction;

#[derive(Debug, Clone, Copy)]
pub struct DefaultKeybind {
    pub trigger: &'static str,
    pub action: CommandAction,
}

pub fn default_keybinds() -> Vec<DefaultKeybind> {
    let mut bindings = vec![
        DefaultKeybind {
            trigger: "secondary-q",
            action: CommandAction::Quit,
        },
        DefaultKeybind {
            trigger: "secondary-,",
            action: CommandAction::OpenSettings,
        },
        DefaultKeybind {
            trigger: "secondary-p",
            action: CommandAction::ToggleCommandPalette,
        },
        DefaultKeybind {
            trigger: "secondary-t",
            action: CommandAction::NewTab,
        },
        DefaultKeybind {
            trigger: "secondary-w",
            action: CommandAction::CloseTab,
        },
        #[cfg(target_os = "macos")]
        DefaultKeybind {
            trigger: "secondary-m",
            action: CommandAction::MinimizeWindow,
        },
        DefaultKeybind {
            trigger: "secondary-=",
            action: CommandAction::ZoomIn,
        },
        DefaultKeybind {
            trigger: "secondary-+",
            action: CommandAction::ZoomIn,
        },
        DefaultKeybind {
            trigger: "secondary--",
            action: CommandAction::ZoomOut,
        },
        DefaultKeybind {
            trigger: "secondary-0",
            action: CommandAction::ZoomReset,
        },
        // Search
        DefaultKeybind {
            trigger: "secondary-f",
            action: CommandAction::OpenSearch,
        },
        DefaultKeybind {
            trigger: "secondary-g",
            action: CommandAction::SearchNext,
        },
        DefaultKeybind {
            trigger: "secondary-shift-g",
            action: CommandAction::SearchPrevious,
        },
    ];

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        bindings.push(DefaultKeybind {
            trigger: "secondary-c",
            action: CommandAction::Copy,
        });
        bindings.push(DefaultKeybind {
            trigger: "secondary-v",
            action: CommandAction::Paste,
        });
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        bindings.push(DefaultKeybind {
            trigger: "ctrl-shift-c",
            action: CommandAction::Copy,
        });
        bindings.push(DefaultKeybind {
            trigger: "ctrl-shift-v",
            action: CommandAction::Paste,
        });
    }

    bindings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_in_has_equal_and_plus_defaults() {
        let zoom_in_triggers = default_keybinds()
            .into_iter()
            .filter(|binding| binding.action == CommandAction::ZoomIn)
            .map(|binding| binding.trigger)
            .collect::<Vec<_>>();

        assert!(zoom_in_triggers.contains(&"secondary-="));
        assert!(zoom_in_triggers.contains(&"secondary-+"));
    }

    #[test]
    fn advanced_palette_actions_are_unbound_by_default() {
        let defaults = default_keybinds();
        assert!(
            defaults
                .iter()
                .all(|binding| binding.action != CommandAction::AppInfo)
        );
        assert!(
            defaults
                .iter()
                .all(|binding| binding.action != CommandAction::RestartApp)
        );
        assert!(
            defaults
                .iter()
                .all(|binding| binding.action != CommandAction::RenameTab)
        );
        assert!(
            defaults
                .iter()
                .all(|binding| binding.action != CommandAction::CheckForUpdates)
        );
        assert!(
            defaults
                .iter()
                .all(|binding| binding.action != CommandAction::SwitchTheme)
        );
        assert!(
            defaults
                .iter()
                .all(|binding| binding.action != CommandAction::NativeSdkExample)
        );
    }
}
