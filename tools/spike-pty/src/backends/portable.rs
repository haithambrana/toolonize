use super::{PtyBackend, PtyHandle, ReadPump};
use anyhow::Result;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::Write;

/// Portable-pty 0.9.0 backend with mitigations per ADR-004 / turborepo#11816:
/// - Respond to DSR (ESC[6n) to avoid ConPTY hang (WIN)
/// - Guard stdin drop on Windows (don't drop stdin unconditionally)
pub struct PortableBackend {
    pty_system: Box<dyn portable_pty::PtySystem + Send>,
}

pub struct PortableHandle {
    master: Box<dyn MasterPty + Send>,
    reader: ReadPump,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send>,
    backend_name: &'static str,
}

impl PortableBackend {
    pub fn new() -> Self {
        Self {
            pty_system: native_pty_system(),
        }
    }
}

impl Default for PortableBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyBackend for PortableBackend {
    fn name(&self) -> &'static str {
        "portable-pty-0.9.0"
    }

    fn hidden_console_evidence(&self) -> Option<&'static str> {
        #[cfg(windows)]
        {
            Some("portable-pty native_pty_system selects its Windows ConPTY pseudoconsole path; the harness requests no console-window creation API")
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    fn spawn(
        &mut self,
        cmd: &str,
        args: &[&str],
        rows: u16,
        cols: u16,
    ) -> Result<Box<dyn PtyHandle>> {
        let pair = self.pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut builder = CommandBuilder::new(cmd);
        for a in args {
            builder.arg(*a);
        }
        // Ensure we have a sane env
        // DSR mitigation is handled by not deadlocking on Windows; portable-pty 0.9.0
        // itself has the INHERIT_CURSOR flag issue, but we mitigate by ensuring
        // the reader handles ESC[6n if it appears (we respond with ESC[24;80R as generic).
        // The harness will also set a read timeout to detect hang.
        let child = pair.slave.spawn_command(builder)?;

        // Drop slave explicitly - on Windows this is where the stdin-drop bug
        // manifested in turborepo#11816. We do NOT drop stdin unconditionally
        // on Windows; portable-pty's Child owns handles, we just keep master.
        // Mitigation: keep master alive, don't force close slave FD prematurely.
        // (portable-pty already handles this, but we document it)

        let master = pair.master;
        let reader = ReadPump::spawn(master.try_clone_reader()?);
        let writer = master.take_writer()?;

        Ok(Box::new(PortableHandle {
            master,
            reader,
            writer,
            child,
            backend_name: "portable-pty-0.9.0",
        }))
    }
}

impl PtyHandle for PortableHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = self.reader.read(buf)?;
        if buf[..n].windows(4).any(|w| w == [0x1b, b'[', b'6', b'n']) {
            // Respond to DSR
            let _ = self.writer.write_all(b"\x1b[24;80R");
            let _ = self.writer.flush();
        }
        Ok(n)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize> {
        Ok(self.writer.write(data)?)
    }

    fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    fn get_size(&self) -> Result<(u16, u16)> {
        let size = self.master.get_size()?;
        Ok((size.rows, size.cols))
    }

    fn kill(&mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
    }

    fn wait(&mut self) -> Result<Option<i32>> {
        let status = self.child.try_wait()?;
        Ok(status.map(|s| s.exit_code() as i32))
    }

    fn is_alive(&mut self) -> bool {
        self.child.try_wait().map(|s| s.is_none()).unwrap_or(false)
    }

    fn backend_name(&self) -> &'static str {
        self.backend_name
    }
}
