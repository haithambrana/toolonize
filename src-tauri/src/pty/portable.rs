//! `portable-pty` 0.9.0 backend with the ADR-004 mitigations.

use super::{
    ExitInfo, PtyBackend, PtyControl, PtyDims, PtyError, PtyKiller, PtyProcess, PtyWaiter,
    SpawnSpec,
};
use portable_pty::{native_pty_system, Child, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct PortableBackend;

impl PortableBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PortableBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn pty_size(dims: PtyDims) -> PtySize {
    PtySize {
        rows: dims.rows,
        cols: dims.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

impl PtyBackend for PortableBackend {
    fn name(&self) -> &'static str {
        "portable-pty-0.9.0"
    }

    fn spawn(&self, spec: &SpawnSpec, dims: PtyDims) -> Result<PtyProcess, PtyError> {
        let pair = native_pty_system().openpty(pty_size(dims))?;

        let mut builder = CommandBuilder::new(&spec.program);
        builder.args(&spec.args);
        if let Some(cwd) = &spec.cwd {
            builder.cwd(cwd);
        }
        #[cfg(unix)]
        {
            builder.env("TERM", "xterm-256color");
            builder.env("COLORTERM", "truecolor");
        }
        builder.env("TERM_PROGRAM", "ToolOnize");
        for (key, value) in &spec.env {
            builder.env(key, value);
        }

        let child = pair.slave.spawn_command(builder)?;
        // The slave end is only needed by the child. Dropping our copy lets the
        // reader observe EOF once the child (and its descendants) exit.
        drop(pair.slave);

        let pid = child.process_id();
        let exited = Arc::new(AtomicBool::new(false));
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let killer = PortableKiller {
            killer: Mutex::new(child.clone_killer()),
            pid,
            exited: exited.clone(),
        };

        Ok(PtyProcess {
            reader,
            writer,
            control: Box::new(PortableControl {
                master: Some(pair.master),
            }),
            waiter: Box::new(PortableWaiter { child, exited }),
            killer: Box::new(killer),
            pid,
        })
    }
}

struct PortableControl {
    master: Option<Box<dyn MasterPty + Send>>,
}

impl PtyControl for PortableControl {
    fn resize(&self, dims: PtyDims) -> Result<(), PtyError> {
        match &self.master {
            Some(master) => Ok(master.resize(pty_size(dims))?),
            None => Err(PtyError("pseudo terminal already closed".into())),
        }
    }

    fn size(&self) -> Result<PtyDims, PtyError> {
        match &self.master {
            Some(master) => {
                let size = master.get_size()?;
                Ok(PtyDims {
                    rows: size.rows,
                    cols: size.cols,
                })
            }
            None => Err(PtyError("pseudo terminal already closed".into())),
        }
    }

    fn close(&mut self) {
        self.master = None;
    }
}

struct PortableWaiter {
    child: Box<dyn Child + Send + Sync>,
    exited: Arc<AtomicBool>,
}

impl PtyWaiter for PortableWaiter {
    fn wait(&mut self) -> Result<ExitInfo, PtyError> {
        let status = self.child.wait();
        self.exited.store(true, Ordering::SeqCst);
        let status = status?;
        Ok(ExitInfo {
            code: status.exit_code(),
            signal: status.signal().map(str::to_string),
        })
    }
}

struct PortableKiller {
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    pid: Option<u32>,
    exited: Arc<AtomicBool>,
}

impl PtyKiller for PortableKiller {
    fn terminate(&self) -> Result<(), PtyError> {
        if self.exited.load(Ordering::SeqCst) {
            return Ok(());
        }
        #[cfg(unix)]
        if let Some(pid) = self.pid.and_then(|pid| i32::try_from(pid).ok()) {
            // The child is a session leader (setsid in portable-pty), so its
            // pid is also its process-group id: hang up the whole group, then
            // force-kill whatever ignored SIGHUP (T-TERM-04 cleanup).
            unsafe {
                libc::killpg(pid, libc::SIGHUP);
            }
            for _ in 0..30 {
                if self.exited.load(Ordering::SeqCst) {
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if !self.exited.load(Ordering::SeqCst) {
                unsafe {
                    libc::killpg(pid, libc::SIGKILL);
                }
            }
            return Ok(());
        }
        let _ = self.pid;
        self.killer
            .lock()
            .map_err(|_| PtyError("killer lock poisoned".into()))?
            .kill()
            .map_err(PtyError::from)
    }
}
