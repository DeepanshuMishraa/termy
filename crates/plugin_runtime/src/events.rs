use serde::{Deserialize, Serialize};

use crate::PluginAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum PluginEventKind {
    #[serde(rename = "terminal.ready")]
    TerminalReady,
    #[serde(rename = "tab.activated")]
    TabActivated,
    #[serde(rename = "workingDirectory.changed")]
    WorkingDirectoryChanged,
    #[serde(rename = "command.finished")]
    CommandFinished,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum PluginEvent {
    #[serde(rename = "terminal.ready")]
    TerminalReady,
    #[serde(rename = "tab.activated")]
    TabActivated {
        #[serde(rename = "previousTabIndex", skip_serializing_if = "Option::is_none")]
        previous_tab_index: Option<usize>,
    },
    #[serde(rename = "workingDirectory.changed")]
    WorkingDirectoryChanged {
        #[serde(
            rename = "previousWorkingDirectory",
            skip_serializing_if = "Option::is_none"
        )]
        previous_working_directory: Option<String>,
        #[serde(rename = "workingDirectory", skip_serializing_if = "Option::is_none")]
        working_directory: Option<String>,
    },
    #[serde(rename = "command.finished")]
    CommandFinished {
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        #[serde(rename = "exitCode", skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        #[serde(rename = "durationMs", skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
}

impl PluginEvent {
    pub fn kind(&self) -> PluginEventKind {
        match self {
            Self::TerminalReady => PluginEventKind::TerminalReady,
            Self::TabActivated { .. } => PluginEventKind::TabActivated,
            Self::WorkingDirectoryChanged { .. } => PluginEventKind::WorkingDirectoryChanged,
            Self::CommandFinished { .. } => PluginEventKind::CommandFinished,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginEventDispatch {
    pub actions: Vec<PluginAction>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginEventSubscriptionDescriptor {
    pub plugin_id: String,
    pub event: PluginEventKind,
    #[serde(default = "super::default_invoke_timeout_ms")]
    pub timeout_ms: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct RegisteredPluginEvent {
    pub plugin_id: String,
    pub event: PluginEventKind,
    pub timeout_ms: u64,
    pub revision: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn event_payloads_use_the_typescript_wire_format() {
        let cases = [
            (
                PluginEvent::TerminalReady,
                json!({ "type": "terminal.ready" }),
            ),
            (
                PluginEvent::TabActivated {
                    previous_tab_index: Some(2),
                },
                json!({ "type": "tab.activated", "previousTabIndex": 2 }),
            ),
            (
                PluginEvent::WorkingDirectoryChanged {
                    previous_working_directory: Some("/old".to_string()),
                    working_directory: Some("/new".to_string()),
                },
                json!({
                    "type": "workingDirectory.changed",
                    "previousWorkingDirectory": "/old",
                    "workingDirectory": "/new",
                }),
            ),
            (
                PluginEvent::CommandFinished {
                    command: Some("cargo test".to_string()),
                    exit_code: Some(0),
                    duration_ms: Some(42),
                },
                json!({
                    "type": "command.finished",
                    "command": "cargo test",
                    "exitCode": 0,
                    "durationMs": 42,
                }),
            ),
        ];

        for (event, expected) in cases {
            assert_eq!(
                serde_json::to_value(event).expect("serialize event"),
                expected
            );
        }
    }

    #[test]
    fn unavailable_event_fields_are_omitted() {
        assert_eq!(
            serde_json::to_value(PluginEvent::CommandFinished {
                command: None,
                exit_code: None,
                duration_ms: None,
            })
            .expect("serialize event"),
            json!({ "type": "command.finished" })
        );
    }
}
