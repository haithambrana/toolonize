//! Session state machines — process vs view orthogonal.

use serde::Serialize;

/// Process/session lifecycle (PTY child / remote attach).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum ProcessSessionState {
    New,
    Starting,
    Running,
    Exited { exit_code: i32 },
    Failed { reason: String },
    Stopping,
    Closed,
    Restarting,
    // Remote-model support (no network in M3, but modeled)
    Disconnected,
    Reconnecting,
}

impl ProcessSessionState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Exited { .. } | Self::Failed { .. } | Self::Closed
        )
    }
}

/// View attachment — orthogonal to ProcessSessionState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ViewAttachmentState {
    Detached,
    Attached,
    Hidden,
}

/// Validate a process-state transition. Illegal transitions fail explicitly.
pub fn validate_transition(
    from: &ProcessSessionState,
    to: &ProcessSessionState,
) -> Result<(), String> {
    use ProcessSessionState::*;
    let allowed = match (from, to) {
        (New, Starting) => true,
        (Starting, Running) => true,
        (Starting, Failed { .. }) => true,
        (Starting, Exited { .. }) => true,
        (Running, Exited { .. }) => true,
        (Running, Failed { .. }) => true,
        (Running, Stopping) => true,
        (Running, Restarting) => true,
        (Running, Disconnected) => true,
        (Disconnected, Reconnecting) => true,
        (Reconnecting, Running) => true,
        (Reconnecting, Failed { .. }) => true,
        (Restarting, Starting) => true,
        (Stopping, Closed) => true,
        (Exited { .. }, Restarting) => true,
        (Exited { .. }, Closed) => true,
        (Failed { .. }, Restarting) => true,
        (Failed { .. }, Closed) => true,
        (Closed, Starting) => true, // explicit restart path after close
        _ => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(format!("illegal transition {from:?} -> {to:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starting_running_closed() {
        assert!(
            validate_transition(&ProcessSessionState::New, &ProcessSessionState::Starting).is_ok()
        );
        assert!(validate_transition(
            &ProcessSessionState::Starting,
            &ProcessSessionState::Running
        )
        .is_ok());
        assert!(validate_transition(
            &ProcessSessionState::Running,
            &ProcessSessionState::Stopping
        )
        .is_ok());
        assert!(
            validate_transition(&ProcessSessionState::Stopping, &ProcessSessionState::Closed)
                .is_ok()
        );
    }

    #[test]
    fn running_exited_closed() {
        assert!(validate_transition(
            &ProcessSessionState::Running,
            &ProcessSessionState::Exited { exit_code: 0 }
        )
        .is_ok());
        assert!(validate_transition(
            &ProcessSessionState::Exited { exit_code: 0 },
            &ProcessSessionState::Closed
        )
        .is_ok());
    }

    #[test]
    fn starting_failed() {
        assert!(validate_transition(
            &ProcessSessionState::Starting,
            &ProcessSessionState::Failed {
                reason: "spawn".to_string()
            }
        )
        .is_ok());
    }

    #[test]
    fn restart_path() {
        assert!(validate_transition(
            &ProcessSessionState::Running,
            &ProcessSessionState::Restarting
        )
        .is_ok());
        assert!(validate_transition(
            &ProcessSessionState::Restarting,
            &ProcessSessionState::Starting
        )
        .is_ok());
        assert!(validate_transition(
            &ProcessSessionState::Exited { exit_code: 0 },
            &ProcessSessionState::Restarting
        )
        .is_ok());
    }

    #[test]
    fn illegal_transition_rejected() {
        assert!(
            validate_transition(&ProcessSessionState::New, &ProcessSessionState::Running).is_err()
        );
        assert!(
            validate_transition(&ProcessSessionState::Running, &ProcessSessionState::New).is_err()
        );
        assert!(
            validate_transition(&ProcessSessionState::Closed, &ProcessSessionState::Running)
                .is_err()
        );
        assert!(
            validate_transition(&ProcessSessionState::Running, &ProcessSessionState::Closed)
                .is_err()
        );
    }

    #[test]
    fn view_states_orthogonal() {
        // Attach/detach/hide must not imply process transition.
        // This is a type-level invariant but we test that the enum values are distinct and clonable.
        let v1 = ViewAttachmentState::Detached;
        let v2 = ViewAttachmentState::Attached;
        let v3 = ViewAttachmentState::Hidden;
        assert_ne!(v1, v2);
        assert_ne!(v2, v3);
    }
}
