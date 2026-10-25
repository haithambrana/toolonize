//! The two orthogonal session state machines (PRD FR-031, DOMAIN_MODEL §3).
//!
//! `ProcessState` describes the child process / remote attach lifecycle and
//! only changes in response to [`ProcessEvent`]s. `ViewState` describes how
//! the UI currently presents the session and only changes in response to
//! [`ViewEvent`]s. The types share no events, so a layout or view operation
//! cannot be expressed as a process transition (T-STATE-001..004).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProcessState {
    New,
    Starting,
    Running,
    Stopping,
    Closed,
    Exited { code: u32, signal: Option<String> },
    Failed { reason: String },
    Restarting,
    Disconnected { code: u32 },
    Reconnecting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessEvent {
    Start,
    Spawned,
    SpawnFailed(String),
    ChildExited {
        code: u32,
        signal: Option<String>,
        remote: bool,
    },
    Stop,
    Restart,
    Reconnect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: ProcessState,
    pub event: ProcessEvent,
}

impl std::fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid transition {:?} on {:?}", self.from, self.event)
    }
}

/// ssh reports connection-level failures (unreachable host, dropped link)
/// with exit status 255. Only remote members interpret it as a disconnect.
pub const SSH_CONNECTION_FAILURE: u32 = 255;

impl ProcessState {
    pub fn is_live(&self) -> bool {
        matches!(
            self,
            ProcessState::Starting
                | ProcessState::Running
                | ProcessState::Stopping
                | ProcessState::Restarting
                | ProcessState::Reconnecting
        )
    }

    pub fn accepts_input(&self) -> bool {
        matches!(self, ProcessState::Running)
    }

    pub fn is_terminal_state(&self) -> bool {
        matches!(
            self,
            ProcessState::Closed
                | ProcessState::Exited { .. }
                | ProcessState::Failed { .. }
                | ProcessState::Disconnected { .. }
        )
    }

    pub fn next(&self, event: ProcessEvent) -> Result<ProcessState, InvalidTransition> {
        use ProcessEvent as E;
        use ProcessState as S;
        let next = match (self, &event) {
            (S::New, E::Start) => S::Starting,
            (S::Restarting | S::Reconnecting, E::Start) => S::Starting,
            (S::Starting, E::Spawned) => S::Running,
            (S::Starting, E::SpawnFailed(reason)) => S::Failed {
                reason: reason.clone(),
            },
            (
                S::Running | S::Starting,
                E::ChildExited {
                    code,
                    signal,
                    remote,
                },
            ) => {
                if *remote && *code == SSH_CONNECTION_FAILURE {
                    S::Disconnected { code: *code }
                } else {
                    S::Exited {
                        code: *code,
                        signal: signal.clone(),
                    }
                }
            }
            // A child that exits while we are stopping it is simply closed.
            (S::Stopping, E::ChildExited { .. }) => S::Closed,
            // Restart/reconnect tear the old child down first; its exit is
            // expected and does not change the in-progress transition.
            (S::Restarting, E::ChildExited { .. }) => S::Restarting,
            (S::Reconnecting, E::ChildExited { .. }) => S::Reconnecting,
            (S::Running | S::Starting, E::Stop) => S::Stopping,
            (S::New | S::Exited { .. } | S::Failed { .. } | S::Disconnected { .. }, E::Stop) => {
                S::Closed
            }
            (
                S::Running | S::Exited { .. } | S::Failed { .. } | S::Disconnected { .. },
                E::Restart,
            ) => S::Restarting,
            (S::Disconnected { .. } | S::Exited { .. } | S::Running, E::Reconnect) => {
                S::Reconnecting
            }
            _ => {
                return Err(InvalidTransition {
                    from: self.clone(),
                    event,
                })
            }
        };
        Ok(next)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewState {
    Detached,
    Attached,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewEvent {
    Attach,
    Detach,
    Hide,
    Show,
}

impl ViewState {
    pub fn next(self, event: ViewEvent) -> ViewState {
        match (self, event) {
            (_, ViewEvent::Detach) => ViewState::Detached,
            (_, ViewEvent::Attach) => ViewState::Attached,
            (ViewState::Attached, ViewEvent::Hide) => ViewState::Hidden,
            (ViewState::Hidden, ViewEvent::Show) => ViewState::Attached,
            (state, _) => state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exited(code: u32, remote: bool) -> ProcessEvent {
        ProcessEvent::ChildExited {
            code,
            signal: None,
            remote,
        }
    }

    #[test]
    fn happy_path_local() {
        let s = ProcessState::New;
        let s = s.next(ProcessEvent::Start).unwrap();
        assert_eq!(s, ProcessState::Starting);
        let s = s.next(ProcessEvent::Spawned).unwrap();
        assert_eq!(s, ProcessState::Running);
        let s = s.next(exited(3, false)).unwrap();
        assert_eq!(
            s,
            ProcessState::Exited {
                code: 3,
                signal: None
            }
        );
    }

    #[test]
    fn spawn_failure_is_failed() {
        let s = ProcessState::Starting
            .next(ProcessEvent::SpawnFailed("no such file".into()))
            .unwrap();
        assert!(matches!(s, ProcessState::Failed { .. }));
        assert_eq!(
            s.next(ProcessEvent::Restart).unwrap(),
            ProcessState::Restarting
        );
    }

    #[test]
    fn remote_255_is_disconnected_and_reconnects() {
        let s = ProcessState::Running.next(exited(255, true)).unwrap();
        assert_eq!(s, ProcessState::Disconnected { code: 255 });
        let s = s.next(ProcessEvent::Reconnect).unwrap();
        assert_eq!(s, ProcessState::Reconnecting);
        assert_eq!(s.next(ProcessEvent::Start).unwrap(), ProcessState::Starting);
    }

    #[test]
    fn local_255_is_plain_exit() {
        let s = ProcessState::Running.next(exited(255, false)).unwrap();
        assert!(matches!(s, ProcessState::Exited { code: 255, .. }));
    }

    #[test]
    fn stop_then_exit_closes() {
        let s = ProcessState::Running.next(ProcessEvent::Stop).unwrap();
        assert_eq!(s, ProcessState::Stopping);
        assert_eq!(s.next(exited(0, false)).unwrap(), ProcessState::Closed);
    }

    #[test]
    fn invalid_transitions_rejected() {
        assert!(ProcessState::Closed.next(ProcessEvent::Spawned).is_err());
        assert!(ProcessState::New.next(ProcessEvent::Spawned).is_err());
        assert!(ProcessState::Closed.next(ProcessEvent::Restart).is_err());
    }

    #[test]
    fn view_events_never_touch_process_state() {
        // T-STATE-001: every view operation leaves the process state as-is.
        // The view machine is a separate type; this test documents that a
        // full sequence of layout-driven view transitions is representable
        // without a single ProcessEvent.
        let process = ProcessState::Running;
        let mut view = ViewState::Detached;
        for event in [
            ViewEvent::Attach,
            ViewEvent::Hide,
            ViewEvent::Show,
            ViewEvent::Detach,
            ViewEvent::Attach,
        ] {
            view = view.next(event);
        }
        assert_eq!(view, ViewState::Attached);
        assert_eq!(process, ProcessState::Running);
    }

    #[test]
    fn view_transitions() {
        assert_eq!(
            ViewState::Detached.next(ViewEvent::Hide),
            ViewState::Detached
        );
        assert_eq!(ViewState::Attached.next(ViewEvent::Hide), ViewState::Hidden);
        assert_eq!(ViewState::Hidden.next(ViewEvent::Show), ViewState::Attached);
        assert_eq!(
            ViewState::Hidden.next(ViewEvent::Detach),
            ViewState::Detached
        );
    }
}
