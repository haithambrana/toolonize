//! PtyBackend abstraction — production boundary.

use super::error::TerminalResult;
use portable_pty::PtySize;

/// Opaque handle for a single PTY session owned by Rust.
/// Writer lifetime belongs to the handle (and thus to the session);
/// detach/reload of the WebView must not drop it.
pub trait PtyHandle: Send {
    /// Read output bytes (blocking with timeout handled by caller).
    fn read(&mut self, buf: &mut [u8]) -> TerminalResult<usize>;

    /// Write input bytes.
    fn write(&mut self, data: &[u8]) -> TerminalResult<usize>;

    /// Flush input.
    fn flush(&mut self) -> TerminalResult<()>;

    /// Resize the PTY.
    fn resize(&mut self, rows: u16, cols: u16) -> TerminalResult<()>;

    /// Current size as known by the backend.
    fn get_size(&self) -> TerminalResult<PtySize>;

    /// Non-blocking poll for exit. Returns Some(code) if exited, None if still running.
    fn try_wait(&mut self) -> TerminalResult<Option<i32>>;

    /// Terminate the child (SIGTERM / kill). Idempotent.
    fn kill(&mut self) -> TerminalResult<()>;

    /// Wait for child with blocking (used at close). Returns exit code if known.
    fn wait(&mut self) -> TerminalResult<Option<i32>>;

    /// Whether the child is still alive (best-effort).
    fn is_alive(&mut self) -> bool;
}

/// Factory / backend trait. Only `PortablePtyBackend` is wired in production.
pub trait PtyBackend: Send {
    /// Human-readable backend name (for metrics / logging — no PII).
    fn name(&self) -> &'static str;

    /// Spawn a new PTY with the given shell builder and size.
    fn spawn(
        &mut self,
        command: &str,
        args: &[String],
        rows: u16,
        cols: u16,
    ) -> TerminalResult<Box<dyn PtyHandle>>;

    /// Verify cleanup / no leak (handle count). Optional fallback uses Ok.
    fn assert_no_leak(&self) -> TerminalResult<()> {
        Ok(())
    }
}
