//! Production Terminal subsystem — M3.
//!
//! Architecture:
//! - `backend` — `PtyBackend` trait boundary
//! - `portable` — `PortablePtyBackend` (portable-pty 0.9.0 + mitigations)
//! - `dsr` — stateful DSR/CPR detector (ConPTY startup)
//! - `transport` — bounded lossless output transport with ack/backpressure
//! - `session` — process vs view state machines
//! - `manager` — `SessionManager` registry (Rust owns sessions)
//! - `profiles` — opaque profile ids, sanitized metadata
//! - `error` — sanitized errors

pub mod backend;
pub mod dsr;
pub mod error;
pub mod manager;
pub mod portable;
pub mod profiles;
pub mod session;
pub mod transport;

pub use manager::{global_manager, SessionManager};
pub use profiles::{available_profiles, TerminalProfile};
