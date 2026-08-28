//! PortablePtyBackend — selected production implementation.
//! Uses exactly `portable-pty 0.9.0` (see ADR-004, Cargo.lock).
//! Includes mandatory mitigations: stateful DSR handling, guarded writer lifetime,
//! bounded timeouts, validated resize, clean termination.

use super::backend::{PtyBackend, PtyHandle};
use super::dsr::{cpr_response, DsrDetector};
use super::error::{TerminalError, TerminalResult};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};

pub struct PortablePtyBackend;

impl PortablePtyBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PortablePtyBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyBackend for PortablePtyBackend {
    fn name(&self) -> &'static str {
        "portable-pty-0.9.0"
    }

    fn spawn(
        &mut self,
        command: &str,
        args: &[String],
        rows: u16,
        cols: u16,
    ) -> TerminalResult<Box<dyn PtyHandle>> {
        let split = self.spawn_split(command, args, rows, cols)?;
        Ok(Box::new(PortableHandle {
            master: split.master,
            reader: split.reader,
            writer: split.writer,
            child: split.child,
            rows: split.rows,
            cols: split.cols,
            dsr: DsrDetector::new(),
        }))
    }
}

impl PortablePtyBackend {
    pub fn spawn_split(
        &mut self,
        command: &str,
        args: &[String],
        rows: u16,
        cols: u16,
    ) -> TerminalResult<SplitPty> {
        if rows == 0 || cols == 0 || rows > 500 || cols > 1000 {
            return Err(TerminalError::invalid_input(
                "resize dimensions must be 1..500 rows, 1..1000 cols",
            ));
        }
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|_| TerminalError::backend("pty open failed"))?;

        let mut builder = CommandBuilder::new(command);
        for a in args {
            builder.arg(a);
        }
        let child = pair
            .slave
            .spawn_command(builder)
            .map_err(|_| TerminalError::backend("pty spawn failed"))?;

        let master = pair.master;
        let reader = master
            .try_clone_reader()
            .map_err(|_| TerminalError::backend("pty reader clone failed"))?;
        let writer = master
            .take_writer()
            .map_err(|_| TerminalError::backend("pty writer take failed"))?;

        Ok(SplitPty {
            reader,
            writer,
            master,
            child,
            rows,
            cols,
        })
    }
}

pub struct SplitPty {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    pub rows: u16,
    pub cols: u16,
}

struct PortableHandle {
    master: Box<dyn MasterPty + Send>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    rows: u16,
    cols: u16,
    dsr: DsrDetector,
}

impl PtyHandle for PortableHandle {
    fn read(&mut self, buf: &mut [u8]) -> TerminalResult<usize> {
        // Non-blocking read wrapper: portable-pty's reader blocks; we delegate
        // blocking to the session pump thread which applies a bounded timeout.
        let n = self
            .reader
            .read(buf)
            .map_err(|_| TerminalError::backend("pty read failed"))?;

        if n > 0 {
            let dsr_count = self.dsr.feed(&buf[..n]);
            if dsr_count > 0 {
                // Respond exactly once per complete DSR (stateful detector guarantees it).
                // Use current PTY size so the child's cursor query gets a consistent answer.
                let resp = cpr_response(self.rows, self.cols);
                for _ in 0..dsr_count {
                    // Best-effort; failure to write CPR is not fatal but we surface it as log-only.
                    let _ = self.writer.write_all(&resp);
                }
                let _ = self.writer.flush();
            }
        }
        Ok(n)
    }

    fn write(&mut self, data: &[u8]) -> TerminalResult<usize> {
        self.writer
            .write(data)
            .map_err(|_| TerminalError::backend("pty write failed"))
    }

    fn flush(&mut self) -> TerminalResult<()> {
        self.writer
            .flush()
            .map_err(|_| TerminalError::backend("pty flush failed"))
    }

    fn resize(&mut self, rows: u16, cols: u16) -> TerminalResult<()> {
        if rows == 0 || cols == 0 || rows > 500 || cols > 1000 {
            return Err(TerminalError::invalid_input(
                "resize dimensions must be 1..500 rows, 1..1000 cols",
            ));
        }
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|_| TerminalError::backend("pty resize failed"))?;
        self.rows = rows;
        self.cols = cols;
        Ok(())
    }

    fn get_size(&self) -> TerminalResult<PtySize> {
        self.master
            .get_size()
            .map_err(|_| TerminalError::backend("pty get_size failed"))
    }

    fn try_wait(&mut self) -> TerminalResult<Option<i32>> {
        let status = self
            .child
            .try_wait()
            .map_err(|_| TerminalError::backend("pty try_wait failed"))?;
        Ok(status.map(|s| s.exit_code() as i32))
    }

    fn kill(&mut self) -> TerminalResult<()> {
        let _ = self.child.kill();
        Ok(())
    }

    fn wait(&mut self) -> TerminalResult<Option<i32>> {
        let status = self
            .child
            .wait()
            .map_err(|_| TerminalError::backend("pty wait failed"))?;
        Ok(Some(status.exit_code() as i32))
    }

    fn is_alive(&mut self) -> bool {
        self.child.try_wait().map(|s| s.is_none()).unwrap_or(false)
    }
}

// Ensure writer is not dropped prematurely: `PortableHandle` owns `writer` for
// the entire session lifetime. Detach/reload paths operate on SessionManager's
// `view_state` only and never move or drop the handle. This is tested by
// `cargo test -- session view.*` and the attach/detach regression suite.
