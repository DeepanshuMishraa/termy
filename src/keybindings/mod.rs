pub mod actions;
mod config;
mod defaults;

use crate::config::AppConfig;
use gpui::App;
use log::warn;

use self::actions::KeybindAction;
use self::config::{KeybindDirective, canonicalize_trigger, parse_keybind_directives};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedKeybind {
    trigger: String,
    action: KeybindAction,
}

pub fn install_keybindings(cx: &mut App, config: &AppConfig) {
    let (directives, warnings) = parse_keybind_directives(&config.keybind_lines);
    if !warnings.is_empty() {
        for warning in &warnings {
            warn!(
                "Ignoring invalid keybind at config line {}: {}",
                warning.line_number, warning.message
            );
        }
        let noun = if warnings.len() == 1 { "line" } else { "lines" };
        termy_toast::warning(format!(
            "Ignored {} invalid keybind {}",
            warnings.len(),
            noun
        ));
    }

    let default_bindings = defaults::default_keybinds()
        .into_iter()
        .filter_map(|binding| match canonicalize_trigger(binding.trigger) {
            Ok(trigger) => Some(ResolvedKeybind {
                trigger,
                action: binding.action,
            }),
            Err(error) => {
                warn!(
                    "Skipping invalid built-in keybind `{}`: {}",
                    binding.trigger, error
                );
                None
            }
        })
        .collect::<Vec<_>>();

    let resolved = resolve_keybinds(default_bindings, &directives);

    cx.clear_key_bindings();
    cx.bind_keys(
        resolved
            .iter()
            .map(|binding| binding.action.to_key_binding(&binding.trigger)),
    );
    cx.set_menus(vec![crate::app_menu()]);
}

fn resolve_keybinds(
    mut bindings: Vec<ResolvedKeybind>,
    directives: &[KeybindDirective],
) -> Vec<ResolvedKeybind> {
    for directive in directives {
        match directive {
            KeybindDirective::Clear => bindings.clear(),
            KeybindDirective::Unbind { trigger } => {
                bindings.retain(|binding| binding.trigger != *trigger);
            }
            KeybindDirective::Bind { trigger, action } => {
                bindings.retain(|binding| binding.trigger != *trigger);
                bindings.push(ResolvedKeybind {
                    trigger: trigger.clone(),
                    action: *action,
                });
            }
        }
    }

    bindings
}

#[cfg(test)]
mod tests {
    use super::{ResolvedKeybind, resolve_keybinds};
    use crate::keybindings::actions::KeybindAction;
    use crate::keybindings::config::KeybindDirective;

    fn resolved(trigger: &str, action: KeybindAction) -> ResolvedKeybind {
        ResolvedKeybind {
            trigger: trigger.to_string(),
            action,
        }
    }

    #[test]
    fn defaults_only_stay_unchanged() {
        let defaults = vec![
            resolved("cmd-p", KeybindAction::ToggleCommandPalette),
            resolved("cmd-c", KeybindAction::Copy),
        ];

        let result = resolve_keybinds(defaults.clone(), &[]);
        assert_eq!(result, defaults);
    }

    #[test]
    fn bind_overrides_same_trigger() {
        let defaults = vec![
            resolved("cmd-p", KeybindAction::ToggleCommandPalette),
            resolved("cmd-c", KeybindAction::Copy),
        ];
        let directives = vec![KeybindDirective::Bind {
            trigger: "cmd-p".to_string(),
            action: KeybindAction::NewTab,
        }];

        let result = resolve_keybinds(defaults, &directives);
        assert_eq!(
            result,
            vec![
                resolved("cmd-c", KeybindAction::Copy),
                resolved("cmd-p", KeybindAction::NewTab)
            ]
        );
    }

    #[test]
    fn unbind_removes_matching_trigger() {
        let defaults = vec![
            resolved("cmd-p", KeybindAction::ToggleCommandPalette),
            resolved("cmd-c", KeybindAction::Copy),
        ];
        let directives = vec![KeybindDirective::Unbind {
            trigger: "cmd-c".to_string(),
        }];

        let result = resolve_keybinds(defaults, &directives);
        assert_eq!(
            result,
            vec![resolved("cmd-p", KeybindAction::ToggleCommandPalette)]
        );
    }

    #[test]
    fn clear_resets_defaults_before_subsequent_binds() {
        let defaults = vec![
            resolved("cmd-p", KeybindAction::ToggleCommandPalette),
            resolved("cmd-c", KeybindAction::Copy),
        ];
        let directives = vec![
            KeybindDirective::Clear,
            KeybindDirective::Bind {
                trigger: "ctrl-k".to_string(),
                action: KeybindAction::OpenConfig,
            },
        ];

        let result = resolve_keybinds(defaults, &directives);
        assert_eq!(result, vec![resolved("ctrl-k", KeybindAction::OpenConfig)]);
    }

    #[test]
    fn directive_order_is_deterministic() {
        let defaults = vec![
            resolved("cmd-c", KeybindAction::Copy),
            resolved("cmd-v", KeybindAction::Paste),
        ];
        let directives = vec![
            KeybindDirective::Bind {
                trigger: "cmd-x".to_string(),
                action: KeybindAction::CloseTab,
            },
            KeybindDirective::Bind {
                trigger: "cmd-c".to_string(),
                action: KeybindAction::Quit,
            },
            KeybindDirective::Unbind {
                trigger: "cmd-v".to_string(),
            },
            KeybindDirective::Bind {
                trigger: "cmd-x".to_string(),
                action: KeybindAction::ZoomIn,
            },
        ];

        let result = resolve_keybinds(defaults, &directives);
        assert_eq!(
            result,
            vec![
                resolved("cmd-c", KeybindAction::Quit),
                resolved("cmd-x", KeybindAction::ZoomIn)
            ]
        );
    }
}
