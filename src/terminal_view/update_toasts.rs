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
                termy_toast::success(format!("v{} installed. Restart to finish update", version));
            }
            Some(UpdateState::Error(message)) => {
                termy_toast::error(format!("Update failed: {}", message));
            }
            _ => {}
        }
    }
}
