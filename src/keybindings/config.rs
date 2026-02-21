use crate::config::KeybindConfigLine;
use gpui::Keystroke;

use crate::commands::CommandAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindDirective {
    Clear,
    Bind {
        trigger: String,
        action: CommandAction,
    },
    Unbind {
        trigger: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindWarning {
    pub line_number: usize,
    pub message: String,
}

pub fn parse_keybind_directives(
    lines: &[KeybindConfigLine],
) -> (Vec<KeybindDirective>, Vec<KeybindWarning>) {
    let mut directives = Vec::new();
    let mut warnings = Vec::new();

    for line in lines {
        let value = line.value.trim();
        if value.is_empty() {
            warnings.push(KeybindWarning {
                line_number: line.line_number,
                message: "empty keybind value".to_string(),
            });
            continue;
        }

        if value.eq_ignore_ascii_case("clear") {
            directives.push(KeybindDirective::Clear);
            continue;
        }

        let Some((trigger_raw, action_raw)) = value.rsplit_once('=') else {
            warnings.push(KeybindWarning {
                line_number: line.line_number,
                message: "expected `keybind = <trigger>=<action>` or `keybind = clear`".to_string(),
            });
            continue;
        };

        let mut trigger_raw = trigger_raw.trim().to_string();
        let action_raw = action_raw.trim();
        if trigger_raw.is_empty() || action_raw.is_empty() {
            warnings.push(KeybindWarning {
                line_number: line.line_number,
                message: "keybind trigger and action must both be non-empty".to_string(),
            });
            continue;
        }

        if action_raw.eq_ignore_ascii_case("unbind") {
            if should_treat_trailing_dash_as_equal_key(&trigger_raw) {
                trigger_raw.push('=');
            }
            let trigger = match canonicalize_trigger(&trigger_raw) {
                Ok(trigger) => trigger,
                Err(message) => {
                    warnings.push(KeybindWarning {
                        line_number: line.line_number,
                        message,
                    });
                    continue;
                }
            };
            directives.push(KeybindDirective::Unbind { trigger });
            continue;
        }

        let Some(action) = CommandAction::from_config_name(action_raw) else {
            warnings.push(KeybindWarning {
                line_number: line.line_number,
                message: format!(
                    "unknown keybind action `{}`; expected one of: {}",
                    action_raw,
                    CommandAction::all_config_names()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
            continue;
        };

        if should_treat_trailing_dash_as_equal_key(&trigger_raw) {
            trigger_raw.push('=');
        }
        let trigger = match canonicalize_trigger(&trigger_raw) {
            Ok(trigger) => trigger,
            Err(message) => {
                warnings.push(KeybindWarning {
                    line_number: line.line_number,
                    message,
                });
                continue;
            }
        };

        directives.push(KeybindDirective::Bind { trigger, action });
    }

    (directives, warnings)
}

fn should_treat_trailing_dash_as_equal_key(trigger: &str) -> bool {
    // `keybind = <trigger>=<action>` uses `=` as the directive separator, so
    // users often write `cmd-=zoom_in` for the equals key. Interpret a trailing
    // single dash as an implicit equals key in that case; `cmd--` remains minus.
    trigger.ends_with('-') && !trigger.ends_with("--")
}

pub(crate) fn canonicalize_trigger(trigger: &str) -> Result<String, String> {
    let mut normalized_parts = Vec::new();
    for component in trigger.split_whitespace() {
        let keystroke = Keystroke::parse(component).map_err(|error| {
            format!(
                "invalid keybind trigger component `{}`: {}",
                component, error
            )
        })?;
        normalized_parts.push(keystroke.unparse());
    }

    if normalized_parts.is_empty() {
        return Err("empty keybind trigger".to_string());
    }

    Ok(normalized_parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::{KeybindDirective, KeybindWarning, canonicalize_trigger, parse_keybind_directives};
    use crate::commands::CommandAction;
    use crate::config::KeybindConfigLine;

    #[test]
    fn parses_clear_bind_and_unbind_in_order() {
        let lines = vec![
            KeybindConfigLine {
                line_number: 10,
                value: "clear".to_string(),
            },
            KeybindConfigLine {
                line_number: 11,
                value: "cmd-p=toggle_command_palette".to_string(),
            },
            KeybindConfigLine {
                line_number: 12,
                value: "cmd-p=unbind".to_string(),
            },
        ];

        let (directives, warnings) = parse_keybind_directives(&lines);

        assert!(warnings.is_empty());
        assert_eq!(
            directives,
            vec![
                KeybindDirective::Clear,
                KeybindDirective::Bind {
                    trigger: "cmd-p".to_string(),
                    action: CommandAction::ToggleCommandPalette
                },
                KeybindDirective::Unbind {
                    trigger: "cmd-p".to_string()
                }
            ]
        );
    }

    #[test]
    fn accepts_secondary_alias() {
        let lines = vec![KeybindConfigLine {
            line_number: 3,
            value: "secondary-p=toggle_command_palette".to_string(),
        }];

        let (directives, warnings) = parse_keybind_directives(&lines);

        assert!(warnings.is_empty());
        assert_eq!(directives.len(), 1);
        match &directives[0] {
            KeybindDirective::Bind { action, .. } => {
                assert_eq!(*action, CommandAction::ToggleCommandPalette);
            }
            _ => panic!("expected bind directive"),
        }
    }

    #[test]
    fn reports_invalid_lines() {
        let lines = vec![
            KeybindConfigLine {
                line_number: 2,
                value: "cmd-p".to_string(),
            },
            KeybindConfigLine {
                line_number: 3,
                value: "=toggle_command_palette".to_string(),
            },
            KeybindConfigLine {
                line_number: 4,
                value: "cmd-p=unknown_action".to_string(),
            },
        ];

        let (_directives, warnings) = parse_keybind_directives(&lines);

        assert_eq!(warnings.len(), 3);
        assert_eq!(
            warnings.iter().map(|w| w.line_number).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
        assert!(
            warnings
                .iter()
                .all(|warning: &KeybindWarning| { !warning.message.trim().is_empty() })
        );
    }

    #[test]
    fn parses_equal_key_shortcut_forms() {
        let lines = vec![
            KeybindConfigLine {
                line_number: 2,
                value: "cmd-=zoom_in".to_string(),
            },
            KeybindConfigLine {
                line_number: 3,
                value: "cmd-==zoom_out".to_string(),
            },
            KeybindConfigLine {
                line_number: 4,
                value: "cmd-=unbind".to_string(),
            },
        ];

        let (directives, warnings) = parse_keybind_directives(&lines);

        assert!(warnings.is_empty());
        assert_eq!(
            directives,
            vec![
                KeybindDirective::Bind {
                    trigger: "cmd-=".to_string(),
                    action: CommandAction::ZoomIn
                },
                KeybindDirective::Bind {
                    trigger: "cmd-=".to_string(),
                    action: CommandAction::ZoomOut
                },
                KeybindDirective::Unbind {
                    trigger: "cmd-=".to_string()
                }
            ]
        );
    }

    #[test]
    fn keeps_minus_key_syntax_unchanged() {
        let lines = vec![KeybindConfigLine {
            line_number: 2,
            value: "cmd--=zoom_out".to_string(),
        }];

        let (directives, warnings) = parse_keybind_directives(&lines);
        assert!(warnings.is_empty());

        let expected = canonicalize_trigger("cmd--").expect("valid minus trigger");
        assert_eq!(
            directives,
            vec![KeybindDirective::Bind {
                trigger: expected,
                action: CommandAction::ZoomOut
            }]
        );
    }

    #[test]
    fn parses_unbound_by_default_actions() {
        let lines = vec![
            KeybindConfigLine {
                line_number: 2,
                value: "secondary-i=app_info".to_string(),
            },
            KeybindConfigLine {
                line_number: 3,
                value: "secondary-r=restart_app".to_string(),
            },
            KeybindConfigLine {
                line_number: 4,
                value: "secondary-e=rename_tab".to_string(),
            },
        ];

        let (directives, warnings) = parse_keybind_directives(&lines);
        assert!(warnings.is_empty());
        let app_info_trigger = canonicalize_trigger("secondary-i").expect("valid app_info trigger");
        let restart_trigger =
            canonicalize_trigger("secondary-r").expect("valid restart_app trigger");
        let rename_trigger = canonicalize_trigger("secondary-e").expect("valid rename_tab trigger");
        assert_eq!(
            directives,
            vec![
                KeybindDirective::Bind {
                    trigger: app_info_trigger,
                    action: CommandAction::AppInfo
                },
                KeybindDirective::Bind {
                    trigger: restart_trigger,
                    action: CommandAction::RestartApp
                },
                KeybindDirective::Bind {
                    trigger: rename_trigger,
                    action: CommandAction::RenameTab
                },
            ]
        );
    }
}
