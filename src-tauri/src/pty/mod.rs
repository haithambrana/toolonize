//! PTY backend boundary (ADR-004).
//!
//! The production backend is `portable-pty` 0.9.0 plus ToolOnize-owned
//! mitigations. Everything platform-specific about pseudo terminals stays
//! below this module; the session manager only sees [`PtyBackend`] and
//! [`PtyProcess`].

mod dsr;
mod portable;

pub use dsr::{is_cursor_report, DsrScanner};
pub use portable::PortableBackend;

use std::io::{Read, Write};
use std::path::PathBuf;

/// Fully constructed, policy-approved spawn request. Built only by the
/// execution policy from a reviewed descriptor; never from frontend text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtyDims {
    pub rows: u16,
    pub cols: u16,
}

impl PtyDims {
    pub const MIN: u16 = 2;
    pub const MAX: u16 = 1000;

    /// Clamp untrusted frontend geometry into a sane range.
    pub fn clamped(rows: u16, cols: u16) -> Self {
        Self {
            rows: rows.clamp(Self::MIN, Self::MAX),
            cols: cols.clamp(Self::MIN, Self::MAX),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitInfo {
    pub code: u32,
    pub signal: Option<String>,
}

#[derive(Debug)]
pub struct PtyError(pub String);

impl std::fmt::Display for PtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PtyError {}

impl From<anyhow::Error> for PtyError {
    fn from(value: anyhow::Error) -> Self {
        PtyError(value.to_string())
    }
}

impl From<std::io::Error> for PtyError {
    fn from(value: std::io::Error) -> Self {
        PtyError(value.to_string())
    }
}

/// Kills a running PTY child (and on Unix its process group) from any thread.
pub trait PtyKiller: Send + Sync {
    fn terminate(&self) -> Result<(), PtyError>;
}

/// A spawned PTY child. Ownership is split so independent threads can read,
/// write, wait, and kill without contending on one lock.
pub struct PtyProcess {
    pub reader: Box<dyn Read + Send>,
    /// Mitigation 2 (ADR-004): the writer is owned for the whole session
    /// lifetime. Dropping it early can terminate a Windows ConPTY child.
    pub writer: Box<dyn Write + Send>,
    pub control: Box<dyn PtyControl>,
    pub waiter: Box<dyn PtyWaiter>,
    pub killer: Box<dyn PtyKiller>,
    pub pid: Option<u32>,
}

pub trait PtyControl: Send {
    fn resize(&self, dims: PtyDims) -> Result<(), PtyError>;
    fn size(&self) -> Result<PtyDims, PtyError>;
    /// Release the pseudo terminal master. On Windows this closes the
    /// pseudoconsole so a pending reader observes EOF after child exit.
    fn close(&mut self);
}

pub trait PtyWaiter: Send {
    fn wait(&mut self) -> Result<ExitInfo, PtyError>;
}

pub trait PtyBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn spawn(&self, spec: &SpawnSpec, dims: PtyDims) -> Result<PtyProcess, PtyError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dims_are_clamped() {
        assert_eq!(PtyDims::clamped(0, 0), PtyDims { rows: 2, cols: 2 });
        assert_eq!(
            PtyDims::clamped(u16::MAX, 5000),
            PtyDims {
                rows: 1000,
                cols: 1000
            }
        );
        assert_eq!(
            PtyDims::clamped(40, 120),
            PtyDims {
                rows: 40,
                cols: 120
            }
        );
    }
}
