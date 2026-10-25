//! Terminal sessions: process lifecycle independent of any view (FR-031).

mod manager;
pub mod output;
pub mod state;

pub use manager::{
    AttachInfo, Listener, SessionError, SessionEvent, SessionInfo, SessionManager, SessionPlan,
    SessionSink, MAX_WRITE,
};
pub use output::FlowLimits;
pub use state::{ProcessState, ViewEvent, ViewState};
