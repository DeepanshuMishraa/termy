use super::*;

impl TerminalView {
    pub(super) fn sync_update_toasts(&mut self, state: Option<&UpdateState>) {
        let changed = self.last_notified_update_state.as_ref() != state;
        if !changed {
            return;
        }

        self.last_notified_update_state = state.cloned();

        match state {
            Some(UpdateState::Available { version, .. }) => {
                termy_toast::info_long(format!("Update v{} available", version));
            }
            Some(UpdateState::Downloaded { version, .. }) => {
                termy_toast::success(format!("v{} ready to install", version));
            }
            Some(UpdateState::Installing { version }) => {
                termy_toast::info(format!("Installing v{}", version));
            }
            Some(UpdateState::Installed { version }) => {
                #[cfg(target_os = "macos")]
                termy_toast::success_long(format!(
                    "v{} installed \u{2014} reopen from /Applications",
                    version
                ));
                #[cfg(target_os = "windows")]
                termy_toast::success_long(format!(
                    "v{} installed \u{2014} restart to apply",
                    version
                ));
                #[cfg(target_os = "linux")]
                termy_toast::success_long(format!(
                    "v{} installed to ~/.local/bin \u{2014} restart to apply",
                    version
                ));
                #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
                termy_toast::success_long(format!(
                    "v{} installed \u{2014} restart to apply",
                    version
                ));
            }
            Some(UpdateState::Error(message)) => {
                termy_toast::error_long(format!("Update failed: {}", message));
            }
            _ => {}
        }
    }
}
