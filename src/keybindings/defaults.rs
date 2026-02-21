use super::actions::KeybindAction;

#[derive(Debug, Clone, Copy)]
pub struct DefaultKeybind {
    pub trigger: &'static str,
    pub action: KeybindAction,
}

pub fn default_keybinds() -> Vec<DefaultKeybind> {
    let mut bindings = vec![
        DefaultKeybind {
            trigger: "secondary-q",
            action: KeybindAction::Quit,
        },
        DefaultKeybind {
            trigger: "secondary-,",
            action: KeybindAction::OpenConfig,
        },
        DefaultKeybind {
            trigger: "secondary-p",
            action: KeybindAction::ToggleCommandPalette,
        },
        DefaultKeybind {
            trigger: "secondary-t",
            action: KeybindAction::NewTab,
        },
        DefaultKeybind {
            trigger: "secondary-w",
            action: KeybindAction::CloseTab,
        },
        DefaultKeybind {
            trigger: "secondary-=",
            action: KeybindAction::ZoomIn,
        },
        DefaultKeybind {
            trigger: "secondary-+",
            action: KeybindAction::ZoomIn,
        },
        DefaultKeybind {
            trigger: "secondary--",
            action: KeybindAction::ZoomOut,
        },
        DefaultKeybind {
            trigger: "secondary-0",
            action: KeybindAction::ZoomReset,
        },
    ];

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        bindings.push(DefaultKeybind {
            trigger: "secondary-c",
            action: KeybindAction::Copy,
        });
        bindings.push(DefaultKeybind {
            trigger: "secondary-v",
            action: KeybindAction::Paste,
        });
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        bindings.push(DefaultKeybind {
            trigger: "ctrl-shift-c",
            action: KeybindAction::Copy,
        });
        bindings.push(DefaultKeybind {
            trigger: "ctrl-shift-v",
            action: KeybindAction::Paste,
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
            .filter(|binding| binding.action == KeybindAction::ZoomIn)
            .map(|binding| binding.trigger)
            .collect::<Vec<_>>();

        assert!(zoom_in_triggers.contains(&"secondary-="));
        assert!(zoom_in_triggers.contains(&"secondary-+"));
    }
}
