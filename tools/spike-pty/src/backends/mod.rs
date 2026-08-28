pub mod direct;
pub mod portable;

use anyhow::Result;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Read};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

pub(crate) fn count_dsr_requests(tail: &mut Vec<u8>, data: &[u8]) -> usize {
    let mut scan = tail.clone();
    scan.extend_from_slice(data);
    let requests = scan
        .windows(4)
        .filter(|window| *window == b"\x1b[6n")
        .count();
    let tail_start = scan.len().saturating_sub(3);
    tail.clear();
    tail.extend_from_slice(&scan[tail_start..]);
    requests
}

/// Trait isolating PTY backend choice (ADR-004).
pub trait PtyBackend: Send {
    fn name(&self) -> &'static str;
    fn hidden_console_evidence(&self) -> Option<&'static str> {
        None
    }
    /// Spawn a child in a PTY. Returns handle.
    fn spawn(
        &mut self,
        cmd: &str,
        args: &[&str],
        rows: u16,
        cols: u16,
    ) -> Result<Box<dyn PtyHandle>>;

    /// Spawn with invalid executable to test error handling.
    fn spawn_invalid(&mut self) -> Result<Box<dyn PtyHandle>> {
        self.spawn("/nonexistent/invalid_executable_xyz_12345", &[], 24, 80)
    }
}

pub trait PtyHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn write(&mut self, data: &[u8]) -> Result<usize>;
    fn resize(&mut self, rows: u16, cols: u16) -> Result<()>;
    fn get_size(&self) -> Result<(u16, u16)>;
    fn kill(&mut self) -> Result<()>;
    fn wait(&mut self) -> Result<Option<i32>>; // exit code if exited
    fn is_alive(&mut self) -> bool;
    fn backend_name(&self) -> &'static str;
}

enum ReadEvent {
    Data(Vec<u8>),
    Eof,
    Error(String),
}

/// Converts a blocking PTY reader into short, interruptible reads. The harness
/// can then enforce deadlines without leaking one thread per timed-out read.
pub(crate) struct ReadPump {
    receiver: Receiver<ReadEvent>,
    pending: VecDeque<u8>,
    closed: bool,
}

impl ReadPump {
    pub(crate) fn spawn(mut reader: Box<dyn Read + Send>) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = vec![0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        let _ = sender.send(ReadEvent::Eof);
                        break;
                    }
                    Ok(count) => {
                        if sender
                            .send(ReadEvent::Data(buffer[..count].to_vec()))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(ReadEvent::Error(error.to_string()));
                        break;
                    }
                }
            }
        });

        Self {
            receiver,
            pending: VecDeque::new(),
            closed: false,
        }
    }

    pub(crate) fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }

        if self.pending.is_empty() && !self.closed {
            match self.receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(ReadEvent::Data(data)) => self.pending.extend(data),
                Ok(ReadEvent::Eof) => self.closed = true,
                Ok(ReadEvent::Error(message)) => {
                    self.closed = true;
                    return Err(Error::other(message));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(Error::new(ErrorKind::WouldBlock, "PTY read timed out"));
                }
                Err(RecvTimeoutError::Disconnected) => self.closed = true,
            }
        }

        let count = output.len().min(self.pending.len());
        for slot in &mut output[..count] {
            *slot = self.pending.pop_front().expect("pending length checked");
        }
        Ok(count)
    }
}

/// Simple registry for comparative testing.
pub fn all_backends() -> Vec<Box<dyn PtyBackend>> {
    vec![
        Box::new(portable::PortableBackend::new()),
        Box::new(direct::DirectBackend::new()),
    ]
}

/// Result for a single backend + scenario.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScenarioResult {
    pub backend: String,
    pub scenario: String,
    pub status: String, // PASS, FAIL, BLOCKED, NOT_VERIFIED
    pub details: String,
    pub duration_ms: u128,
    pub extra: HashMap<String, String>,
}

#[derive(Debug, serde::Serialize)]
pub struct SpikeReport {
    pub platform: &'static str,
    pub architecture: &'static str,
    pub results: Vec<ScenarioResult>,
}

#[cfg(test)]
mod tests {
    use super::count_dsr_requests;

    #[test]
    fn detects_dsr_requests_split_across_reads_without_recounting() {
        let mut tail = Vec::new();
        assert_eq!(count_dsr_requests(&mut tail, b"prefix\x1b["), 0);
        assert_eq!(count_dsr_requests(&mut tail, b"6ntext"), 1);
        assert_eq!(count_dsr_requests(&mut tail, b"more text"), 0);
        assert_eq!(count_dsr_requests(&mut tail, b"\x1b[6n\x1b[6n"), 2);
    }
}
