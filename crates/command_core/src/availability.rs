use crate::CommandId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandCapabilities {
    pub tmux_runtime_active: bool,
    pub install_cli_available: bool,
    pub browser_tabs_enabled: bool,
    pub browser_tabs_supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandUnavailableReason {
    RequiresTmuxRuntime,
    InstallCliAlreadyInstalled,
    BrowserTabsDisabled,
    BrowserTabsUnsupported,
    BrowserTabsUnavailableInTmux,
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

        if matches!(self, Self::NewBrowserTab) {
            if caps.tmux_runtime_active {
                return CommandAvailability {
                    enabled: false,
                    reason: Some(CommandUnavailableReason::BrowserTabsUnavailableInTmux),
                };
            }

            if !caps.browser_tabs_supported {
                return CommandAvailability {
                    enabled: false,
                    reason: Some(CommandUnavailableReason::BrowserTabsUnsupported),
                };
            }

            if !caps.browser_tabs_enabled {
                return CommandAvailability {
                    enabled: false,
                    reason: Some(CommandUnavailableReason::BrowserTabsDisabled),
                };
            }
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
            browser_tabs_supported: true,
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
            browser_tabs_supported: true,
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
    fn new_browser_tab_is_gated_on_platform_support() {
        let unsupported = CommandCapabilities {
            tmux_runtime_active: false,
            install_cli_available: true,
            browser_tabs_enabled: true,
            browser_tabs_supported: false,
        };
        let availability = CommandId::NewBrowserTab.availability(unsupported);
        assert!(!availability.enabled);
        assert_eq!(
            availability.reason,
            Some(CommandUnavailableReason::BrowserTabsUnsupported)
        );
    }

    #[test]
    fn new_browser_tab_is_gated_on_native_runtime() {
        let tmux_runtime = CommandCapabilities {
            tmux_runtime_active: true,
            install_cli_available: true,
            browser_tabs_enabled: true,
            browser_tabs_supported: true,
        };
        let availability = CommandId::NewBrowserTab.availability(tmux_runtime);
        assert!(!availability.enabled);
        assert_eq!(
            availability.reason,
            Some(CommandUnavailableReason::BrowserTabsUnavailableInTmux)
        );
    }

    #[test]
    fn command_availability_reports_install_cli_when_already_installed() {
        let caps = CommandCapabilities {
            tmux_runtime_active: true,
            install_cli_available: false,
            browser_tabs_enabled: true,
            browser_tabs_supported: true,
        };
        let availability = CommandId::InstallCli.availability(caps);
        assert!(!availability.enabled);
        assert_eq!(
            availability.reason,
            Some(CommandUnavailableReason::InstallCliAlreadyInstalled)
        );
    }
}
