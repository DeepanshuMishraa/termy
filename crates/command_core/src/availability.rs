use crate::CommandId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandCapabilities {
    pub tmux_runtime_active: bool,
    pub install_cli_available: bool,
    pub browser_tabs_enabled: bool,
    pub git_panel_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandUnavailableReason {
    RequiresTmuxRuntime,
    InstallCliAlreadyInstalled,
    BrowserTabsDisabled,
    GitPanelDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandAvailability {
    pub enabled: bool,
    pub reason: Option<CommandUnavailableReason>,
}

impl CommandId {
    pub const fn availability(self, caps: CommandCapabilities) -> CommandAvailability {
        if self.is_tmux_only() && !caps.tmux_runtime_active {
            return CommandAvailability {
                enabled: false,
                reason: Some(CommandUnavailableReason::RequiresTmuxRuntime),
            };
        }

        if matches!(self, Self::InstallCli) && !caps.install_cli_available {
            return CommandAvailability {
                enabled: false,
                reason: Some(CommandUnavailableReason::InstallCliAlreadyInstalled),
            };
        }

        if matches!(self, Self::NewBrowserTab) && !caps.browser_tabs_enabled {
            return CommandAvailability {
                enabled: false,
                reason: Some(CommandUnavailableReason::BrowserTabsDisabled),
            };
        }

        if matches!(self, Self::ToggleGitPanel) && !caps.git_panel_enabled {
            return CommandAvailability {
                enabled: false,
                reason: Some(CommandUnavailableReason::GitPanelDisabled),
            };
        }

        CommandAvailability {
            enabled: true,
            reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandCapabilities, CommandUnavailableReason};
    use crate::CommandId;

    #[test]
    fn resize_commands_are_available_without_tmux_runtime() {
        let caps = CommandCapabilities {
            tmux_runtime_active: false,
            install_cli_available: true,
            browser_tabs_enabled: true,
            git_panel_enabled: true,
        };
        let availability = CommandId::ResizePaneLeft.availability(caps);
        assert!(availability.enabled);
        assert_eq!(availability.reason, None);
    }

    #[test]
    fn new_browser_tab_is_gated_on_browser_tabs_setting() {
        let disabled = CommandCapabilities {
            tmux_runtime_active: false,
            install_cli_available: true,
            browser_tabs_enabled: false,
            git_panel_enabled: true,
        };
        let availability = CommandId::NewBrowserTab.availability(disabled);
        assert!(!availability.enabled);
        assert_eq!(
            availability.reason,
            Some(CommandUnavailableReason::BrowserTabsDisabled)
        );

        let enabled = CommandCapabilities {
            browser_tabs_enabled: true,
            ..disabled
        };
        assert!(CommandId::NewBrowserTab.availability(enabled).enabled);
    }

    #[test]
    fn toggle_git_panel_is_gated_on_git_panel_setting() {
        let disabled = CommandCapabilities {
            tmux_runtime_active: false,
            install_cli_available: true,
            browser_tabs_enabled: true,
            git_panel_enabled: false,
        };
        let availability = CommandId::ToggleGitPanel.availability(disabled);
        assert!(!availability.enabled);
        assert_eq!(
            availability.reason,
            Some(CommandUnavailableReason::GitPanelDisabled)
        );

        let enabled = CommandCapabilities {
            git_panel_enabled: true,
            ..disabled
        };
        assert!(CommandId::ToggleGitPanel.availability(enabled).enabled);
    }

    #[test]
    fn command_availability_reports_install_cli_when_already_installed() {
        let caps = CommandCapabilities {
            tmux_runtime_active: true,
            install_cli_available: false,
            browser_tabs_enabled: true,
            git_panel_enabled: true,
        };
        let availability = CommandId::InstallCli.availability(caps);
        assert!(!availability.enabled);
        assert_eq!(
            availability.reason,
            Some(CommandUnavailableReason::InstallCliAlreadyInstalled)
        );
    }
}
