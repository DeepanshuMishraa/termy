use crate::config::KeybindConfigLine;
use gpui::Keystroke;

use super::actions::KeybindAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindDirective {
    Clear,
    Bind {
        trigger: String,
        action: KeybindAction,
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

        let Some((trigger_raw, action_raw)) = value.split_once('=') else {
            warnings.push(KeybindWarning {
                line_number: line.line_number,
                message: "expected `keybind = <trigger>=<action>` or `keybind = clear`".to_string(),
            });
            continue;
        };

        let trigger_raw = trigger_raw.trim();
        let action_raw = action_raw.trim();
        if trigger_raw.is_empty() || action_raw.is_empty() {
            warnings.push(KeybindWarning {
                line_number: line.line_number,
                message: "keybind trigger and action must both be non-empty".to_string(),
            });
            continue;
        }

        let trigger = match canonicalize_trigger(trigger_raw) {
            Ok(trigger) => trigger,
            Err(message) => {
                warnings.push(KeybindWarning {
                    line_number: line.line_number,
                    message,
                });
                continue;
            }
        };

        if action_raw.eq_ignore_ascii_case("unbind") {
            directives.push(KeybindDirective::Unbind { trigger });
            continue;
        }

        let Some(action) = KeybindAction::from_config_name(action_raw) else {
            warnings.push(KeybindWarning {
                line_number: line.line_number,
                message: format!(
                    "unknown keybind action `{}`; expected one of: {}",
                    action_raw,
                    KeybindAction::all_config_names().join(", ")
                ),
            });
            continue;
        };

        directives.push(KeybindDirective::Bind { trigger, action });
    }

    (directives, warnings)
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
    use super::{KeybindDirective, KeybindWarning, parse_keybind_directives};
    use crate::config::KeybindConfigLine;
    use crate::keybindings::actions::KeybindAction;

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
                    action: KeybindAction::ToggleCommandPalette
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
                assert_eq!(*action, KeybindAction::ToggleCommandPalette);
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
}
