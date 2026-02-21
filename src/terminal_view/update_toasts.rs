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
                termy_toast::info(format!("Update v{} available", version));
            }
            Some(UpdateState::Downloaded { version, .. }) => {
                termy_toast::success(format!("Update v{} downloaded", version));
            }
            Some(UpdateState::Installing { version }) => {
                termy_toast::info(format!("Installing v{}...", version));
            }
            Some(UpdateState::Installed { version }) => {
                #[cfg(target_os = "macos")]
                termy_toast::success(format!(
                    "v{} installed. Reopen Termy from /Applications to use the new version",
                    version
                ));
                #[cfg(target_os = "windows")]
                termy_toast::success(format!(
                    "v{} installed. Restart Termy to use the new version",
                    version
                ));
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                termy_toast::success(format!("v{} installed. Restart Termy to use the new version", version));
            }
            Some(UpdateState::Error(message)) => {
                termy_toast::error(format!("Update failed: {}", message));
            }
            _ => {}
        }
    }
}
