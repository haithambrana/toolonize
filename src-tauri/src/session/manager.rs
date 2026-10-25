//! Terminal Session Manager: Rust-owned PTY sessions that outlive any view.
//!
//! Each session owns one PTY child at a time (a *generation*). Per generation
//! there is a reader thread (PTY -> output log, with backpressure), a writer
//! thread (input queue -> PTY; it also owns the writer for the full child
//! lifetime) and a waiter thread (exit detection). One pump thread per session
//! moves bytes from the output log to whichever view is attached.

use super::output::{encode_frame, FlowLimits, OutputLog};
use super::state::{ProcessEvent, ProcessState, ViewEvent, ViewState};
use crate::pty::{
    is_cursor_report, DsrScanner, PtyBackend, PtyControl, PtyDims, PtyKiller, SpawnSpec,
};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Maximum bytes accepted by one `write` call (a large paste).
pub const MAX_WRITE: usize = 1024 * 1024;
/// Maximum queued, not yet written input per session.
pub const MAX_INPUT_QUEUE: usize = 4 * 1024 * 1024;
/// A view that stops acknowledging for this long while the reader is paused
/// is detached with an explicit desynchronization event.
pub const STALL_TIMEOUT: Duration = Duration::from_secs(20);
const CURSOR_REPLY: &[u8] = b"\x1b[1;1R";
const STARTUP_DSR_WINDOW: u64 = 4096;

/// Receives output frames and events for one attached view.
pub trait SessionSink: Send + Sync {
    /// Returns false when the receiving view is gone.
    fn output(&self, frame: Vec<u8>) -> bool;
    fn event(&self, event: &SessionEvent) -> bool;
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    State { info: SessionInfo },
    Desync { reason: String, end_offset: u64 },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SessionInfo {
    pub session_id: String,
    pub workspace_id: String,
    pub member_id: String,
    pub title: String,
    pub process_state: ProcessState,
    pub view_state: ViewState,
    pub generation: u32,
    pub pid: Option<u32>,
    pub remote: bool,
    pub detach_expected: bool,
    pub rows: u16,
    pub cols: u16,
    pub base_offset: u64,
    pub end_offset: u64,
    pub attached: bool,
    pub backend: String,
    pub started_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AttachInfo {
    pub attachment_id: u64,
    pub start_offset: u64,
    pub end_offset: u64,
    /// True when the requested offset had already left the replay window.
    pub trimmed: bool,
    pub info: SessionInfo,
}

#[derive(Debug, Clone)]
pub struct SessionPlan {
    pub workspace_id: String,
    pub member_id: String,
    pub title: String,
    pub spec: SpawnSpec,
    pub remote: bool,
    pub detach_expected: bool,
    pub dims: PtyDims,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    NotFound,
    InvalidState(String),
    InputTooLarge,
    InputQueueFull,
    Spawn(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::NotFound => f.write_str("session not found"),
            SessionError::InvalidState(s) => write!(f, "invalid session state: {s}"),
            SessionError::InputTooLarge => f.write_str("input exceeds the per-write limit"),
            SessionError::InputQueueFull => f.write_str("terminal input queue is full"),
            SessionError::Spawn(s) => write!(f, "failed to start: {s}"),
        }
    }
}

pub type Listener = Arc<dyn Fn(&SessionInfo) + Send + Sync>;

struct Attachment {
    id: u64,
    sink: Arc<dyn SessionSink>,
    sent: u64,
    acked: u64,
    last_progress: Instant,
}

struct Shared {
    workspace_id: String,
    member_id: String,
    title: String,
    remote: bool,
    detach_expected: bool,
    spec: SpawnSpec,
    process_state: ProcessState,
    view_state: ViewState,
    generation: u32,
    alive_generation: Option<u32>,
    reader_done_generation: u32,
    pid: Option<u32>,
    dims: PtyDims,
    log: OutputLog,
    attachment: Option<Attachment>,
    paused: bool,
    backpressure_events: u64,
    input: VecDeque<Vec<u8>>,
    input_bytes: usize,
    dsr_credits: u32,
    generation_bytes: u64,
    control: Option<Box<dyn PtyControl>>,
    killer: Option<Arc<dyn PtyKiller>>,
    closed: bool,
    started_at_ms: u64,
}

pub struct Session {
    id: String,
    shared: Mutex<Shared>,
    cond: Condvar,
    backend: Arc<dyn PtyBackend>,
    listener: Listener,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Session {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn info_locked(&self, s: &Shared) -> SessionInfo {
        SessionInfo {
            session_id: self.id.clone(),
            workspace_id: s.workspace_id.clone(),
            member_id: s.member_id.clone(),
            title: s.title.clone(),
            process_state: s.process_state.clone(),
            view_state: s.view_state,
            generation: s.generation,
            pid: s.pid,
            remote: s.remote,
            detach_expected: s.detach_expected,
            rows: s.dims.rows,
            cols: s.dims.cols,
            base_offset: s.log.base(),
            end_offset: s.log.end(),
            attached: s.attachment.is_some(),
            backend: self.backend.name().to_string(),
            started_at_ms: s.started_at_ms,
        }
    }

    pub fn info(&self) -> SessionInfo {
        let s = self.lock();
        self.info_locked(&s)
    }

    /// Broadcast the current state to the attached view and the listener.
    fn publish(&self) {
        let (info, sink) = {
            let s = self.lock();
            (
                self.info_locked(&s),
                s.attachment.as_ref().map(|a| a.sink.clone()),
            )
        };
        if let Some(sink) = sink {
            sink.event(&SessionEvent::State { info: info.clone() });
        }
        (self.listener)(&info);
    }

    fn transition(&self, s: &mut Shared, event: ProcessEvent) -> Result<(), SessionError> {
        s.process_state = s
            .process_state
            .next(event)
            .map_err(|e| SessionError::InvalidState(e.to_string()))?;
        Ok(())
    }

    /// Spawn a new generation. Caller must have moved the state to a value
    /// that accepts `ProcessEvent::Start`.
    fn spawn_generation(self: &Arc<Self>) -> Result<(), SessionError> {
        let (spec, dims, generation) = {
            let mut s = self.lock();
            self.transition(&mut s, ProcessEvent::Start)?;
            s.generation += 1;
            s.input.clear();
            s.input_bytes = 0;
            s.dsr_credits = 0;
            s.generation_bytes = 0;
            (s.spec.clone(), s.dims, s.generation)
        };
        self.publish();

        match self.backend.spawn(&spec, dims) {
            Err(error) => {
                {
                    let mut s = self.lock();
                    let _ = self.transition(&mut s, ProcessEvent::SpawnFailed(error.0.clone()));
                    s.pid = None;
                }
                self.cond.notify_all();
                self.publish();
                Err(SessionError::Spawn(error.0))
            }
            Ok(process) => {
                let killer: Arc<dyn PtyKiller> = Arc::from(process.killer);
                {
                    let mut s = self.lock();
                    s.pid = process.pid;
                    s.control = Some(process.control);
                    s.killer = Some(killer);
                    s.alive_generation = Some(generation);
                    s.started_at_ms = now_ms();
                    let _ = self.transition(&mut s, ProcessEvent::Spawned);
                }
                self.spawn_reader(generation, process.reader);
                self.spawn_writer(generation, process.writer);
                self.spawn_waiter(generation, process.waiter);
                self.cond.notify_all();
                self.publish();
                Ok(())
            }
        }
    }

    fn spawn_reader(self: &Arc<Self>, generation: u32, mut reader: Box<dyn Read + Send>) {
        let session = self.clone();
        std::thread::Builder::new()
            .name(format!("pty-read-{}", self.id))
            .spawn(move || {
                let mut buffer = vec![0u8; 64 * 1024];
                let mut scanner = DsrScanner::new();
                loop {
                    let count = match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => count,
                    };
                    let chunk = &buffer[..count];
                    let requests = scanner.scan(chunk);
                    if !session.accept_output(generation, chunk, requests) {
                        break;
                    }
                }
                let mut s = session.lock();
                if s.reader_done_generation < generation {
                    s.reader_done_generation = generation;
                }
                drop(s);
                session.cond.notify_all();
            })
            .expect("spawn PTY reader thread");
    }

    /// Append PTY output, blocking while an attached view is behind.
    /// Returns false when the session is closed.
    fn accept_output(&self, generation: u32, chunk: &[u8], dsr_requests: usize) -> bool {
        let mut desync: Option<Arc<dyn SessionSink>> = None;
        let mut s = self.lock();
        if dsr_requests > 0 {
            // Windows ConPTY blocks at startup until its request is answered;
            // without a view nobody else can answer. Every reply we send here
            // earns a credit used to drop the duplicate reply xterm.js sends.
            let startup = cfg!(windows) && s.generation_bytes < STARTUP_DSR_WINDOW;
            if startup || s.attachment.is_none() {
                for _ in 0..dsr_requests {
                    s.input.push_front(CURSOR_REPLY.to_vec());
                    s.dsr_credits += 1;
                }
            }
        }
        let limits = s.log.limits();
        loop {
            if s.closed {
                return false;
            }
            let Some((acked, last_progress)) =
                s.attachment.as_ref().map(|a| (a.acked, a.last_progress))
            else {
                s.paused = false;
                break;
            };
            let unacked = s.log.end().saturating_sub(acked) as usize;
            if s.paused {
                if unacked <= limits.low_water {
                    s.paused = false;
                    break;
                }
            } else if unacked >= limits.high_water {
                s.paused = true;
                s.backpressure_events += 1;
            } else {
                break;
            }
            if last_progress.elapsed() > STALL_TIMEOUT {
                // Hard limit: never drop bytes, detach the stalled view.
                desync = s.attachment.take().map(|a| a.sink);
                s.view_state = ViewState::Detached;
                s.paused = false;
                break;
            }
            let (guard, _) = self
                .cond
                .wait_timeout(s, Duration::from_millis(100))
                .unwrap_or_else(|poison| poison.into_inner());
            s = guard;
        }
        if s.generation == generation {
            s.generation_bytes += chunk.len() as u64;
        }
        s.log.append(chunk);
        let protect = s.attachment.as_ref().map(|a| a.acked);
        s.log.trim(protect);
        let end_offset = s.log.end();
        drop(s);
        self.cond.notify_all();
        if let Some(sink) = desync {
            sink.event(&SessionEvent::Desync {
                reason: "view stopped acknowledging output".into(),
                end_offset,
            });
            self.publish();
        }
        true
    }

    fn spawn_writer(self: &Arc<Self>, generation: u32, mut writer: Box<dyn Write + Send>) {
        let session = self.clone();
        std::thread::Builder::new()
            .name(format!("pty-write-{}", self.id))
            .spawn(move || loop {
                let chunk = {
                    let mut s = session.lock();
                    loop {
                        if s.closed || s.alive_generation != Some(generation) {
                            return;
                        }
                        if let Some(chunk) = s.input.pop_front() {
                            s.input_bytes = s.input_bytes.saturating_sub(chunk.len());
                            break chunk;
                        }
                        let (guard, _) = session
                            .cond
                            .wait_timeout(s, Duration::from_millis(250))
                            .unwrap_or_else(|poison| poison.into_inner());
                        s = guard;
                    }
                };
                if writer
                    .write_all(&chunk)
                    .and_then(|_| writer.flush())
                    .is_err()
                {
                    return;
                }
            })
            .expect("spawn PTY writer thread");
    }

    fn spawn_waiter(self: &Arc<Self>, generation: u32, mut waiter: Box<dyn crate::pty::PtyWaiter>) {
        let session = self.clone();
        std::thread::Builder::new()
            .name(format!("pty-wait-{}", self.id))
            .spawn(move || {
                let exit = waiter.wait();
                // Capture all output before reporting the exit: wait for the
                // reader to observe EOF; if it does not (ConPTY keeps the pipe
                // open, or a daemon kept the slave), close the master.
                // ConPTY never reports EOF while the pseudoconsole is open, so
                // Windows closes it right away. Closing happens outside the
                // session lock: ClosePseudoConsole can wait for the reader.
                let reader_done = |s: &Shared| s.reader_done_generation >= generation;
                if cfg!(windows) || !session.wait_until(Duration::from_secs(2), reader_done) {
                    session.close_control(generation);
                }
                session.wait_until(Duration::from_secs(2), reader_done);
                {
                    let mut s = session.lock();
                    if s.alive_generation == Some(generation) {
                        s.alive_generation = None;
                        let (code, signal) = match exit {
                            Ok(info) => (info.code, info.signal),
                            Err(_) => (1, Some("wait failed".into())),
                        };
                        let remote = s.remote;
                        let _ = session.transition(
                            &mut s,
                            ProcessEvent::ChildExited {
                                code,
                                signal,
                                remote,
                            },
                        );
                        s.killer = None;
                        s.pid = None;
                    }
                }
                session.close_control(generation);
                session.cond.notify_all();
                session.publish();
            })
            .expect("spawn PTY waiter thread");
    }

    /// Release the PTY master of `generation` (if still installed) without
    /// holding the session lock while it closes.
    fn close_control(&self, generation: u32) {
        let control = {
            let mut s = self.lock();
            if s.generation == generation {
                s.control.take()
            } else {
                None
            }
        };
        if let Some(mut control) = control {
            control.close();
        }
    }

    fn wait_until(&self, timeout: Duration, condition: impl Fn(&Shared) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        let mut s = self.lock();
        loop {
            if condition(&s) {
                return true;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (guard, _) = self
                .cond
                .wait_timeout(s, remaining.min(Duration::from_millis(100)))
                .unwrap_or_else(|poison| poison.into_inner());
            s = guard;
        }
    }

    fn spawn_pump(self: &Arc<Self>) {
        let session = self.clone();
        std::thread::Builder::new()
            .name(format!("pty-pump-{}", self.id))
            .spawn(move || loop {
                let (sink, attachment_id, frame) = {
                    let mut s = session.lock();
                    loop {
                        if s.closed {
                            return;
                        }
                        let limits = s.log.limits();
                        let end = s.log.end();
                        if let Some(a) = s.attachment.as_ref() {
                            let in_flight = (a.sent - a.acked) as usize;
                            if a.sent < end && in_flight < limits.window {
                                break;
                            }
                        }
                        let (guard, _) = session
                            .cond
                            .wait_timeout(s, Duration::from_millis(500))
                            .unwrap_or_else(|poison| poison.into_inner());
                        s = guard;
                    }
                    let limits = s.log.limits();
                    let (sent, acked) = {
                        let a = s.attachment.as_ref().expect("checked above");
                        (a.sent, a.acked)
                    };
                    let room = limits.window - (sent - acked) as usize;
                    let (start, data) = s.log.read(sent, limits.batch.min(room));
                    let a = s.attachment.as_mut().expect("checked above");
                    a.sent = start + data.len() as u64;
                    (a.sink.clone(), a.id, encode_frame(start, &data))
                };
                if !sink.output(frame) {
                    let detached = {
                        let mut s = session.lock();
                        if s.attachment.as_ref().map(|a| a.id) == Some(attachment_id) {
                            s.attachment = None;
                            s.view_state = ViewState::Detached;
                            s.paused = false;
                            true
                        } else {
                            false
                        }
                    };
                    session.cond.notify_all();
                    if detached {
                        session.publish();
                    }
                }
            })
            .expect("spawn PTY pump thread");
    }

    fn terminate_current(&self) {
        let killer = self.lock().killer.clone();
        if let Some(killer) = killer {
            let _ = killer.terminate();
        }
    }
}

pub struct SessionManager {
    backend: Arc<dyn PtyBackend>,
    limits: Mutex<FlowLimits>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    listener: Listener,
    next_id: AtomicU64,
}

impl SessionManager {
    pub fn new(backend: Arc<dyn PtyBackend>, listener: Listener) -> Self {
        Self {
            backend,
            limits: Mutex::new(FlowLimits::default()),
            sessions: Mutex::new(HashMap::new()),
            listener,
            next_id: AtomicU64::new(1),
        }
    }

    pub fn with_limits(self, limits: FlowLimits) -> Self {
        assert!(limits.validate(), "inconsistent flow limits");
        *self.limits.lock().unwrap() = limits;
        self
    }

    /// Change the replay retention for sessions started from now on.
    pub fn set_retention(&self, bytes: usize) {
        let mut limits = self.limits.lock().unwrap();
        let candidate = FlowLimits {
            retention: bytes.max(limits.high_water),
            ..*limits
        };
        if candidate.validate() {
            *limits = candidate;
        }
    }

    fn get(&self, id: &str) -> Result<Arc<Session>, SessionError> {
        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or(SessionError::NotFound)
    }

    pub fn find_by_member(&self, workspace_id: &str, member_id: &str) -> Option<SessionInfo> {
        self.list()
            .into_iter()
            .find(|i| i.workspace_id == workspace_id && i.member_id == member_id)
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        let sessions: Vec<_> = self.sessions.lock().unwrap().values().cloned().collect();
        let mut out: Vec<_> = sessions.iter().map(|s| s.info()).collect();
        out.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        out
    }

    pub fn info(&self, id: &str) -> Result<SessionInfo, SessionError> {
        Ok(self.get(id)?.info())
    }

    /// Number of times the reader paused for a slow view (diagnostics/tests).
    pub fn backpressure_events(&self, id: &str) -> Result<u64, SessionError> {
        Ok(self.get(id)?.lock().backpressure_events)
    }

    /// Start a session for a member. One live session per member: an
    /// existing session for the same member is returned unchanged.
    pub fn start(&self, plan: SessionPlan) -> Result<SessionInfo, SessionError> {
        if let Some(existing) = self.find_by_member(&plan.workspace_id, &plan.member_id) {
            return Ok(existing);
        }
        let id = format!(
            "s{}-{:x}",
            self.next_id.fetch_add(1, Ordering::SeqCst),
            now_ms()
        );
        let limits = *self.limits.lock().unwrap();
        let session = Arc::new(Session {
            id: id.clone(),
            shared: Mutex::new(Shared {
                workspace_id: plan.workspace_id,
                member_id: plan.member_id,
                title: plan.title,
                remote: plan.remote,
                detach_expected: plan.detach_expected,
                spec: plan.spec,
                process_state: ProcessState::New,
                view_state: ViewState::Detached,
                generation: 0,
                alive_generation: None,
                reader_done_generation: 0,
                pid: None,
                dims: plan.dims,
                log: OutputLog::new(limits),
                attachment: None,
                paused: false,
                backpressure_events: 0,
                input: VecDeque::new(),
                input_bytes: 0,
                dsr_credits: 0,
                generation_bytes: 0,
                control: None,
                killer: None,
                closed: false,
                started_at_ms: now_ms(),
            }),
            cond: Condvar::new(),
            backend: self.backend.clone(),
            listener: self.listener.clone(),
        });
        self.sessions
            .lock()
            .unwrap()
            .insert(id.clone(), session.clone());
        session.spawn_pump();
        // A spawn failure is a session state (Failed + retry), not an error
        // for the caller: other members are unaffected (FS-02).
        let _ = session.spawn_generation();
        Ok(session.info())
    }

    pub fn write(&self, id: &str, data: &[u8]) -> Result<(), SessionError> {
        if data.len() > MAX_WRITE {
            return Err(SessionError::InputTooLarge);
        }
        if data.is_empty() {
            return Ok(());
        }
        let session = self.get(id)?;
        let mut s = session.lock();
        if !s.process_state.accepts_input() {
            return Err(SessionError::InvalidState(
                "the process is not running".into(),
            ));
        }
        if s.dsr_credits > 0 && is_cursor_report(data) {
            s.dsr_credits -= 1;
            return Ok(());
        }
        if s.input_bytes + data.len() > MAX_INPUT_QUEUE {
            return Err(SessionError::InputQueueFull);
        }
        s.input_bytes += data.len();
        s.input.push_back(data.to_vec());
        drop(s);
        session.cond.notify_all();
        Ok(())
    }

    pub fn resize(&self, id: &str, rows: u16, cols: u16) -> Result<SessionInfo, SessionError> {
        let session = self.get(id)?;
        let dims = PtyDims::clamped(rows, cols);
        {
            let mut s = session.lock();
            if s.dims != dims {
                s.dims = dims;
                if let Some(control) = s.control.as_ref() {
                    control
                        .resize(dims)
                        .map_err(|e| SessionError::InvalidState(e.0))?;
                }
            }
        }
        Ok(session.info())
    }

    /// Report a view transition. Only the view machine changes.
    pub fn view_event(&self, id: &str, event: ViewEvent) -> Result<SessionInfo, SessionError> {
        let session = self.get(id)?;
        {
            let mut s = session.lock();
            s.view_state = s.view_state.next(event);
        }
        Ok(session.info())
    }

    pub fn attach(
        &self,
        id: &str,
        from_offset: u64,
        sink: Arc<dyn SessionSink>,
    ) -> Result<AttachInfo, SessionError> {
        let session = self.get(id)?;
        let attachment_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (start, end, trimmed) = {
            let mut s = session.lock();
            let base = s.log.base();
            let end = s.log.end();
            let trimmed = from_offset < base;
            let start = from_offset.clamp(base, end);
            s.attachment = Some(Attachment {
                id: attachment_id,
                sink,
                sent: start,
                acked: start,
                last_progress: Instant::now(),
            });
            s.view_state = ViewState::Attached;
            s.paused = false;
            (start, end, trimmed)
        };
        session.cond.notify_all();
        Ok(AttachInfo {
            attachment_id,
            start_offset: start,
            end_offset: end,
            trimmed,
            info: session.info(),
        })
    }

    pub fn detach(&self, id: &str, attachment_id: u64) -> Result<SessionInfo, SessionError> {
        let session = self.get(id)?;
        {
            let mut s = session.lock();
            if s.attachment.as_ref().map(|a| a.id) == Some(attachment_id) {
                s.attachment = None;
                s.view_state = ViewState::Detached;
                s.paused = false;
            }
        }
        session.cond.notify_all();
        Ok(session.info())
    }

    pub fn ack(&self, id: &str, attachment_id: u64, offset: u64) -> Result<(), SessionError> {
        let session = self.get(id)?;
        {
            let mut s = session.lock();
            let mut progressed = false;
            if let Some(a) = s.attachment.as_mut() {
                if a.id == attachment_id && offset > a.acked && offset <= a.sent {
                    a.acked = offset;
                    a.last_progress = Instant::now();
                    progressed = true;
                }
            }
            if progressed {
                let protect = s.attachment.as_ref().map(|a| a.acked);
                s.log.trim(protect);
            }
        }
        session.cond.notify_all();
        Ok(())
    }

    /// Replace the process with a fresh one built from `spec` (which the
    /// execution policy re-authorized). Output history is kept.
    pub fn restart(&self, id: &str, spec: SpawnSpec) -> Result<SessionInfo, SessionError> {
        self.replace_process(id, spec, ProcessEvent::Restart)
    }

    /// Remote members only: start a NEW remote session via the user's own
    /// tooling. Plain SSH does not resume the previous remote shell.
    pub fn reconnect(&self, id: &str, spec: SpawnSpec) -> Result<SessionInfo, SessionError> {
        let session = self.get(id)?;
        if !session.lock().remote {
            return Err(SessionError::InvalidState(
                "reconnect is only available for remote members".into(),
            ));
        }
        self.replace_process(id, spec, ProcessEvent::Reconnect)
    }

    fn replace_process(
        &self,
        id: &str,
        spec: SpawnSpec,
        event: ProcessEvent,
    ) -> Result<SessionInfo, SessionError> {
        let session = self.get(id)?;
        let old_generation = {
            let mut s = session.lock();
            session.transition(&mut s, event)?;
            s.spec = spec;
            s.alive_generation
        };
        session.publish();
        if let Some(old) = old_generation {
            session.terminate_current();
            session.wait_until(Duration::from_secs(6), |s| s.alive_generation != Some(old));
        }
        let _ = session.spawn_generation();
        Ok(session.info())
    }

    /// Terminate the process; the session record stays (Closed) until
    /// [`SessionManager::close`] removes it.
    pub fn stop(&self, id: &str) -> Result<SessionInfo, SessionError> {
        let session = self.get(id)?;
        let live = {
            let mut s = session.lock();
            session.transition(&mut s, ProcessEvent::Stop)?;
            s.alive_generation.is_some()
        };
        session.publish();
        if live {
            session.terminate_current();
            session.wait_until(Duration::from_secs(5), |s| s.alive_generation.is_none());
        }
        Ok(session.info())
    }

    /// Stop (if needed) and forget the session.
    pub fn close(&self, id: &str) -> Result<(), SessionError> {
        let session = self.get(id)?;
        if session.lock().process_state.is_live() {
            let _ = self.stop(id);
        }
        {
            let mut s = session.lock();
            s.closed = true;
            s.process_state = ProcessState::Closed;
        }
        session.cond.notify_all();
        session.publish();
        self.sessions.lock().unwrap().remove(id);
        Ok(())
    }

    pub fn close_workspace(&self, workspace_id: &str) {
        for info in self.list() {
            if info.workspace_id == workspace_id {
                let _ = self.close(&info.session_id);
            }
        }
    }

    /// App exit: terminate every local child (FR-037). Persistence of
    /// `detach_expected` members comes from tmux/screen, not from us.
    pub fn shutdown(&self) {
        let sessions: Vec<_> = self.sessions.lock().unwrap().values().cloned().collect();
        let handles: Vec<_> = sessions
            .into_iter()
            .map(|session| {
                std::thread::spawn(move || {
                    session.terminate_current();
                    session.wait_until(Duration::from_secs(2), |s| s.alive_generation.is_none());
                    session.lock().closed = true;
                    session.cond.notify_all();
                })
            })
            .collect();
        for handle in handles {
            let _ = handle.join();
        }
        self.sessions.lock().unwrap().clear();
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
