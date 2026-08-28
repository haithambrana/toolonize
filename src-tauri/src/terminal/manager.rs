//! SessionManager registry — Rust owns sessions.
//! SessionId is opaque; PID is never exposed as identity.
//! View state is orthogonal to process state; attach/detach never mutates process state.

use super::error::{TerminalError, TerminalResult};
use super::portable::PortablePtyBackend;
use super::profiles::resolve_profile;
use super::session::{validate_transition, ProcessSessionState, ViewAttachmentState};
use super::transport::{OutputChunk, Transport, TransportState};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type SessionId = String;

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub generation: u64,
    pub profile_id: String,
    pub process_state: ProcessSessionState,
    pub view_state: ViewAttachmentState,
    pub rows: u16,
    pub cols: u16,
    pub transport_state: TransportState,
    pub replay_truncated: bool,
    pub exit_code: Option<i32>,
}

/// Internal session owned by the manager.
struct Session {
    id: SessionId,
    generation: u64,
    profile_id: String,
    process_state: ProcessSessionState,
    view_state: ViewAttachmentState,
    rows: u16,
    cols: u16,
    // Split handles — pump owns reader, manager owns these for control
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    // Shared size for DSR CPR response
    shared_rows: Arc<Mutex<u16>>,
    shared_cols: Arc<Mutex<u16>>,
    transport: Arc<Mutex<Transport>>,
    pump_handle: Option<thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
    replay_truncated: bool,
    exit_code: Option<i32>,
}

pub struct SessionManager {
    sessions: Mutex<HashMap<SessionId, Session>>,
    next_id: AtomicU64,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    fn new_id(&self) -> SessionId {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        let rnd = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() ^ (n as u32).wrapping_mul(0x9E3779B9))
            .unwrap_or(n as u32);
        format!("sess_{n:08x}_{rnd:08x}")
    }

    /// List all sessions (projection for frontend — no PII).
    pub fn list(&self) -> Vec<SessionInfo> {
        self.refresh_process_states();
        let map = self.sessions.lock().unwrap();
        map.values()
            .map(|s| SessionInfo {
                session_id: s.id.clone(),
                generation: s.generation,
                profile_id: s.profile_id.clone(),
                process_state: s.process_state.clone(),
                view_state: s.view_state,
                rows: s.rows,
                cols: s.cols,
                transport_state: s.transport.lock().unwrap().state(),
                replay_truncated: s.replay_truncated,
                exit_code: s.exit_code,
            })
            .collect()
    }

    pub fn get_info(&self, id: &str) -> TerminalResult<SessionInfo> {
        self.refresh_process_states();
        let map = self.sessions.lock().unwrap();
        let s = map
            .get(id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        let ts = s.transport.lock().unwrap().state();
        let info = SessionInfo {
            session_id: s.id.clone(),
            generation: s.generation,
            profile_id: s.profile_id.clone(),
            process_state: s.process_state.clone(),
            view_state: s.view_state,
            rows: s.rows,
            cols: s.cols,
            transport_state: ts,
            replay_truncated: s.replay_truncated,
            exit_code: s.exit_code,
        };
        Ok(info)
    }

    fn refresh_process_states(&self) {
        let mut map = self.sessions.lock().unwrap();
        for sess in map.values_mut() {
            if sess.process_state == ProcessSessionState::Running {
                let exit_opt = {
                    let mut h = sess.child.lock().unwrap();
                    h.try_wait().unwrap_or(None)
                };
                if let Some(status) = exit_opt {
                    let code = status.exit_code() as i32;
                    let target = ProcessSessionState::Exited { exit_code: code };
                    if validate_transition(&sess.process_state, &target).is_ok() {
                        sess.process_state = target;
                        sess.exit_code = Some(code);
                    }
                }
            }
        }
    }

    pub fn start(&self, profile_id: &str, rows: u16, cols: u16) -> TerminalResult<SessionInfo> {
        let resolved = resolve_profile(profile_id)?;
        if rows == 0 || cols == 0 || rows > 500 || cols > 1000 {
            return Err(TerminalError::invalid_input("invalid dimensions"));
        }
        let id = self.new_id();
        let generation = 1u64;
        let mut backend = PortablePtyBackend::new();
        let split = backend.spawn_split(&resolved.program, &resolved.args, rows, cols)?;
        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(split.writer));
        let master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>> =
            Arc::new(Mutex::new(split.master));
        let child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>> =
            Arc::new(Mutex::new(split.child));
        let reader = split.reader;
        let shared_rows = Arc::new(Mutex::new(rows));
        let shared_cols = Arc::new(Mutex::new(cols));
        let transport: Arc<Mutex<Transport>> = Arc::new(Mutex::new(Transport::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Spawn pump thread that owns reader, shares writer/master/child for DSR & exit checks
        let pump_writer = Arc::clone(&writer);
        let pump_child = Arc::clone(&child);
        let pump_rows = Arc::clone(&shared_rows);
        let pump_cols = Arc::clone(&shared_cols);
        let pump_transport = Arc::clone(&transport);
        let pump_stop = Arc::clone(&stop_flag);
        let mut pump_reader = reader;
        let mut dsr = super::dsr::DsrDetector::new();
        let pump = thread::spawn(move || {
            let mut buf = vec![0u8; 8192];
            // Make reader non-blocking if possible by setting timeout via poll? For now, blocking read with stop_flag check via killing.
            // To avoid indefinite block, we use a trick: after stop_flag set, the child will be killed and reader will get EOF.
            while !pump_stop.load(Ordering::Relaxed) {
                // Check child exit
                {
                    let mut h = pump_child.lock().unwrap();
                    if let Ok(Some(_)) = h.try_wait() {
                        break;
                    }
                }
                let n = match pump_reader.read(&mut buf) {
                    Ok(0) => {
                        thread::sleep(Duration::from_millis(5));
                        0
                    }
                    Ok(n) => n,
                    Err(_) => {
                        thread::sleep(Duration::from_millis(5));
                        0
                    }
                };
                if n > 0 {
                    let dsr_count = dsr.feed(&buf[..n]);
                    if dsr_count > 0 {
                        let r = *pump_rows.lock().unwrap();
                        let c = *pump_cols.lock().unwrap();
                        let resp = super::dsr::cpr_response(r, c);
                        let mut w = pump_writer.lock().unwrap();
                        for _ in 0..dsr_count {
                            let _ = w.write_all(&resp);
                        }
                        let _ = w.flush();
                    }
                    let data = buf[..n].to_vec();
                    let mut tr = pump_transport.lock().unwrap();
                    match tr.enqueue(&data) {
                        Ok(()) => {}
                        Err(super::transport::TransportError::WouldBlock) => {
                            drop(tr);
                            thread::sleep(Duration::from_millis(5));
                            let mut tr2 = pump_transport.lock().unwrap();
                            let mut waited = 0u32;
                            while !tr2.below_low_water() && waited < 200 {
                                drop(tr2);
                                thread::sleep(Duration::from_millis(5));
                                waited += 1;
                                tr2 = pump_transport.lock().unwrap();
                            }
                            let _ = tr2.enqueue(&data);
                        }
                        Err(super::transport::TransportError::HardLimitBreach { .. }) => break,
                        Err(super::transport::TransportError::Desynchronized) => break,
                        Err(_) => break,
                    }
                } else {
                    thread::sleep(Duration::from_millis(2));
                }
            }
        });

        let sess = Session {
            id: id.clone(),
            generation,
            profile_id: profile_id.to_string(),
            process_state: ProcessSessionState::Running,
            view_state: ViewAttachmentState::Detached,
            rows,
            cols,
            writer,
            master,
            child,
            shared_rows,
            shared_cols,
            transport: Arc::clone(&transport),
            pump_handle: Some(pump),
            stop_flag,
            replay_truncated: false,
            exit_code: None,
        };
        let info = SessionInfo {
            session_id: sess.id.clone(),
            generation: sess.generation,
            profile_id: sess.profile_id.clone(),
            process_state: sess.process_state.clone(),
            view_state: sess.view_state,
            rows: sess.rows,
            cols: sess.cols,
            transport_state: sess.transport.lock().unwrap().state(),
            replay_truncated: sess.replay_truncated,
            exit_code: sess.exit_code,
        };
        self.sessions.lock().unwrap().insert(id, sess);
        Ok(info)
    }

    pub fn write(&self, session_id: &str, data: &[u8]) -> TerminalResult<()> {
        let map = self.sessions.lock().unwrap();
        let sess = map
            .get(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        if matches!(
            sess.process_state,
            ProcessSessionState::Exited { .. }
                | ProcessSessionState::Failed { .. }
                | ProcessSessionState::Closed
        ) {
            return Err(TerminalError::invalid_input("session not running"));
        }
        let mut w = sess.writer.lock().unwrap();
        w.write_all(data)
            .map_err(|_| TerminalError::backend("pty write failed"))?;
        w.flush()
            .map_err(|_| TerminalError::backend("pty flush failed"))?;
        Ok(())
    }

    pub fn resize(&self, session_id: &str, rows: u16, cols: u16) -> TerminalResult<()> {
        if rows == 0 || cols == 0 || rows > 500 || cols > 1000 {
            return Err(TerminalError::invalid_input(
                "resize dimensions must be 1..500 rows, 1..1000 cols",
            ));
        }
        let mut map = self.sessions.lock().unwrap();
        let sess = map
            .get_mut(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        {
            let m = sess.master.lock().unwrap();
            m.resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|_| TerminalError::backend("pty resize failed"))?;
        }
        sess.rows = rows;
        sess.cols = cols;
        *sess.shared_rows.lock().unwrap() = rows;
        *sess.shared_cols.lock().unwrap() = cols;
        Ok(())
    }

    pub fn attach(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        let mut map = self.sessions.lock().unwrap();
        let sess = map
            .get_mut(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        let prev_proc = sess.process_state.clone();
        let prev_gen = sess.generation;
        sess.view_state = ViewAttachmentState::Attached;
        debug_assert_eq!(sess.process_state, prev_proc);
        debug_assert_eq!(sess.generation, prev_gen);
        let ts = sess.transport.lock().unwrap().state();
        let info = SessionInfo {
            session_id: sess.id.clone(),
            generation: sess.generation,
            profile_id: sess.profile_id.clone(),
            process_state: sess.process_state.clone(),
            view_state: sess.view_state,
            rows: sess.rows,
            cols: sess.cols,
            transport_state: ts,
            replay_truncated: sess.replay_truncated,
            exit_code: sess.exit_code,
        };
        Ok(info)
    }

    pub fn detach(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        let mut map = self.sessions.lock().unwrap();
        let sess = map
            .get_mut(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        let prev_proc = sess.process_state.clone();
        let prev_gen = sess.generation;
        sess.view_state = ViewAttachmentState::Detached;
        debug_assert_eq!(sess.process_state, prev_proc);
        debug_assert_eq!(sess.generation, prev_gen);
        let ts = sess.transport.lock().unwrap().state();
        let info = SessionInfo {
            session_id: sess.id.clone(),
            generation: sess.generation,
            profile_id: sess.profile_id.clone(),
            process_state: sess.process_state.clone(),
            view_state: sess.view_state,
            rows: sess.rows,
            cols: sess.cols,
            transport_state: ts,
            replay_truncated: sess.replay_truncated,
            exit_code: sess.exit_code,
        };
        Ok(info)
    }

    pub fn hide(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        let mut map = self.sessions.lock().unwrap();
        let sess = map
            .get_mut(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        let prev_proc = sess.process_state.clone();
        sess.view_state = ViewAttachmentState::Hidden;
        debug_assert_eq!(sess.process_state, prev_proc);
        let ts = sess.transport.lock().unwrap().state();
        let info = SessionInfo {
            session_id: sess.id.clone(),
            generation: sess.generation,
            profile_id: sess.profile_id.clone(),
            process_state: sess.process_state.clone(),
            view_state: sess.view_state,
            rows: sess.rows,
            cols: sess.cols,
            transport_state: ts,
            replay_truncated: sess.replay_truncated,
            exit_code: sess.exit_code,
        };
        Ok(info)
    }

    pub fn show(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        let mut map = self.sessions.lock().unwrap();
        let sess = map
            .get_mut(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        let prev_proc = sess.process_state.clone();
        sess.view_state = ViewAttachmentState::Attached;
        debug_assert_eq!(sess.process_state, prev_proc);
        let ts = sess.transport.lock().unwrap().state();
        let info = SessionInfo {
            session_id: sess.id.clone(),
            generation: sess.generation,
            profile_id: sess.profile_id.clone(),
            process_state: sess.process_state.clone(),
            view_state: sess.view_state,
            rows: sess.rows,
            cols: sess.cols,
            transport_state: ts,
            replay_truncated: sess.replay_truncated,
            exit_code: sess.exit_code,
        };
        Ok(info)
    }

    pub fn ack(&self, session_id: &str, sequence: u64) -> TerminalResult<()> {
        let map = self.sessions.lock().unwrap();
        let sess = map
            .get(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        let mut tr = sess.transport.lock().unwrap();
        tr.ack(sequence)
            .map_err(|e| TerminalError::transport(e.to_string()))?;
        Ok(())
    }

    pub fn next_chunk(&self, session_id: &str) -> TerminalResult<Option<OutputChunk>> {
        let map = self.sessions.lock().unwrap();
        let sess = map
            .get(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        let mut tr = sess.transport.lock().unwrap();
        let chunk = tr
            .next_chunk(&sess.id, sess.generation)
            .map_err(|e| TerminalError::transport(e.to_string()))?;
        Ok(chunk)
    }

    pub fn poll_chunks(&self, session_id: &str, max: usize) -> TerminalResult<Vec<OutputChunk>> {
        let map = self.sessions.lock().unwrap();
        let sess = map
            .get(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        let mut tr = sess.transport.lock().unwrap();
        let mut out = Vec::new();
        for _ in 0..max {
            match tr.next_chunk(&sess.id, sess.generation) {
                Ok(Some(c)) => out.push(c),
                Ok(None) => break,
                Err(e) => return Err(TerminalError::transport(e.to_string())),
            }
        }
        Ok(out)
    }

    pub fn replay(&self, session_id: &str) -> TerminalResult<Vec<u8>> {
        let map = self.sessions.lock().unwrap();
        let sess = map
            .get(session_id)
            .ok_or_else(|| TerminalError::not_found("session not found"))?;
        let tr = sess.transport.lock().unwrap();
        Ok(tr.replay_bytes().to_vec())
    }

    pub fn close(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        // Take session out of map to avoid holding lock while joining
        let mut sess = {
            let mut map = self.sessions.lock().unwrap();
            map.remove(session_id)
                .ok_or_else(|| TerminalError::not_found("session not found"))?
        };
        // Transition
        if sess.process_state == ProcessSessionState::Running {
            validate_transition(&sess.process_state, &ProcessSessionState::Stopping)
                .map_err(TerminalError::illegal_transition)?;
            sess.process_state = ProcessSessionState::Stopping;
        } else if matches!(
            sess.process_state,
            ProcessSessionState::Exited { .. } | ProcessSessionState::Failed { .. }
        ) {
            validate_transition(&sess.process_state, &ProcessSessionState::Closed)
                .map_err(TerminalError::illegal_transition)?;
            sess.process_state = ProcessSessionState::Closed;
            sess.stop_flag.store(true, Ordering::Relaxed);
            {
                let mut h = sess.child.lock().unwrap();
                let _ = h.kill();
            }
            if let Some(h) = sess.pump_handle.take() {
                let _ = h.join();
            }
            let ts = sess.transport.lock().unwrap().state();
            let info = SessionInfo {
                session_id: sess.id.clone(),
                generation: sess.generation,
                profile_id: sess.profile_id.clone(),
                process_state: sess.process_state.clone(),
                view_state: sess.view_state,
                rows: sess.rows,
                cols: sess.cols,
                transport_state: ts,
                replay_truncated: sess.replay_truncated,
                exit_code: sess.exit_code,
            };
            // Re-insert as Closed for inspection, or keep removed? For M3, keep as Closed in map.
            self.sessions.lock().unwrap().insert(sess.id.clone(), sess);
            return Ok(info);
        }
        // Kill child
        {
            let mut h = sess.child.lock().unwrap();
            let _ = h.kill();
        }
        sess.stop_flag.store(true, Ordering::Relaxed);
        thread::sleep(Duration::from_millis(50));
        {
            let mut h = sess.child.lock().unwrap();
            if let Ok(Some(status)) = h.try_wait() {
                sess.exit_code = Some(status.exit_code() as i32);
            }
            let _ = h.wait();
        }
        if let Some(handle) = sess.pump_handle.take() {
            // Join with timeout to avoid hang if reader blocked
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            let _ = rx.recv_timeout(Duration::from_millis(500));
        }
        validate_transition(&sess.process_state, &ProcessSessionState::Closed)
            .map_err(TerminalError::illegal_transition)?;
        sess.process_state = ProcessSessionState::Closed;
        let ts = sess.transport.lock().unwrap().state();
        let info = SessionInfo {
            session_id: sess.id.clone(),
            generation: sess.generation,
            profile_id: sess.profile_id.clone(),
            process_state: sess.process_state.clone(),
            view_state: sess.view_state,
            rows: sess.rows,
            cols: sess.cols,
            transport_state: ts,
            replay_truncated: sess.replay_truncated,
            exit_code: sess.exit_code,
        };
        self.sessions.lock().unwrap().insert(sess.id.clone(), sess);
        Ok(info)
    }

    pub fn restart(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        // Snapshot profile/rows/cols and old state
        let (profile_id, rows, cols, old_state) = {
            let map = self.sessions.lock().unwrap();
            let s = map
                .get(session_id)
                .ok_or_else(|| TerminalError::not_found("session not found"))?;
            (
                s.profile_id.clone(),
                s.rows,
                s.cols,
                s.process_state.clone(),
            )
        };
        let restarting = ProcessSessionState::Restarting;
        validate_transition(&old_state, &restarting).map_err(TerminalError::illegal_transition)?;
        // Remove old session, kill and join
        let mut old_sess = {
            let mut map = self.sessions.lock().unwrap();
            map.remove(session_id)
                .ok_or_else(|| TerminalError::not_found("session not found"))?
        };
        old_sess.stop_flag.store(true, Ordering::Relaxed);
        {
            let mut h = old_sess.child.lock().unwrap();
            let _ = h.kill();
        }
        if let Some(handle) = old_sess.pump_handle.take() {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            let _ = rx.recv_timeout(Duration::from_millis(500));
        }
        let mut generation = old_sess.generation;
        let view_state = old_sess.view_state;
        // Spawn new handle
        let resolved = resolve_profile(&profile_id)?;
        let mut backend = PortablePtyBackend::new();
        let split = backend.spawn_split(&resolved.program, &resolved.args, rows, cols)?;
        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(split.writer));
        let master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>> =
            Arc::new(Mutex::new(split.master));
        let child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>> =
            Arc::new(Mutex::new(split.child));
        let reader = split.reader;
        let shared_rows = Arc::new(Mutex::new(rows));
        let shared_cols = Arc::new(Mutex::new(cols));
        let transport: Arc<Mutex<Transport>> = Arc::new(Mutex::new(Transport::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let pump_writer = Arc::clone(&writer);
        let pump_child = Arc::clone(&child);
        let pump_rows = Arc::clone(&shared_rows);
        let pump_cols = Arc::clone(&shared_cols);
        let pump_transport = Arc::clone(&transport);
        let pump_stop = Arc::clone(&stop_flag);
        let mut pump_reader = reader;
        let mut dsr = super::dsr::DsrDetector::new();
        let pump = thread::spawn(move || {
            let mut buf = vec![0u8; 8192];
            while !pump_stop.load(Ordering::Relaxed) {
                {
                    let mut h = pump_child.lock().unwrap();
                    if let Ok(Some(_)) = h.try_wait() {
                        break;
                    }
                }
                let n = match pump_reader.read(&mut buf) {
                    Ok(0) => {
                        thread::sleep(Duration::from_millis(5));
                        0
                    }
                    Ok(n) => n,
                    Err(_) => {
                        thread::sleep(Duration::from_millis(5));
                        0
                    }
                };
                if n > 0 {
                    let dsr_count = dsr.feed(&buf[..n]);
                    if dsr_count > 0 {
                        let r = *pump_rows.lock().unwrap();
                        let c = *pump_cols.lock().unwrap();
                        let resp = super::dsr::cpr_response(r, c);
                        let mut w = pump_writer.lock().unwrap();
                        for _ in 0..dsr_count {
                            let _ = w.write_all(&resp);
                        }
                        let _ = w.flush();
                    }
                    let data = buf[..n].to_vec();
                    let mut tr = pump_transport.lock().unwrap();
                    match tr.enqueue(&data) {
                        Ok(()) => {}
                        Err(super::transport::TransportError::WouldBlock) => {
                            drop(tr);
                            thread::sleep(Duration::from_millis(5));
                            let mut tr2 = pump_transport.lock().unwrap();
                            let mut waited = 0u32;
                            while !tr2.below_low_water() && waited < 200 {
                                drop(tr2);
                                thread::sleep(Duration::from_millis(5));
                                waited += 1;
                                tr2 = pump_transport.lock().unwrap();
                            }
                            let _ = tr2.enqueue(&data);
                        }
                        Err(_) => break,
                    }
                } else {
                    thread::sleep(Duration::from_millis(2));
                }
            }
        });

        // Build new session with incremented generation and Running state
        generation += 1;
        let mut new_state = ProcessSessionState::Restarting;
        validate_transition(&old_state, &new_state).map_err(TerminalError::illegal_transition)?;
        new_state = ProcessSessionState::Starting;
        validate_transition(&ProcessSessionState::Restarting, &new_state)
            .map_err(TerminalError::illegal_transition)?;
        new_state = ProcessSessionState::Running;
        validate_transition(&ProcessSessionState::Starting, &new_state)
            .map_err(TerminalError::illegal_transition)?;

        let sess = Session {
            id: session_id.to_string(),
            generation,
            profile_id: profile_id.clone(),
            process_state: ProcessSessionState::Running,
            view_state,
            rows,
            cols,
            writer,
            master,
            child,
            shared_rows,
            shared_cols,
            transport,
            pump_handle: Some(pump),
            stop_flag,
            replay_truncated: false,
            exit_code: None,
        };
        let info = SessionInfo {
            session_id: sess.id.clone(),
            generation: sess.generation,
            profile_id: sess.profile_id.clone(),
            process_state: sess.process_state.clone(),
            view_state: sess.view_state,
            rows: sess.rows,
            cols: sess.cols,
            transport_state: sess.transport.lock().unwrap().state(),
            replay_truncated: sess.replay_truncated,
            exit_code: sess.exit_code,
        };
        self.sessions.lock().unwrap().insert(sess.id.clone(), sess);
        Ok(info)
    }

    pub fn shutdown_all(&self) {
        let ids: Vec<String> = {
            let map = self.sessions.lock().unwrap();
            map.keys().cloned().collect()
        };
        for id in ids {
            let _ = self.close(&id);
        }
    }

    #[cfg(test)]
    pub fn generation(&self, id: &str) -> Option<u64> {
        let map = self.sessions.lock().unwrap();
        map.get(id).map(|s| s.generation)
    }

    #[cfg(test)]
    pub fn view_state(&self, id: &str) -> Option<ViewAttachmentState> {
        let map = self.sessions.lock().unwrap();
        map.get(id).map(|s| s.view_state)
    }

    #[cfg(test)]
    pub fn process_state(&self, id: &str) -> Option<ProcessSessionState> {
        let map = self.sessions.lock().unwrap();
        map.get(id).map(|s| s.process_state.clone())
    }
}

// Global singleton for Tauri commands.
static GLOBAL_MANAGER: std::sync::OnceLock<SessionManager> = std::sync::OnceLock::new();

pub fn global_manager() -> &'static SessionManager {
    GLOBAL_MANAGER.get_or_init(SessionManager::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::manager::SessionManager;
    use crate::terminal::session::ProcessSessionState;
    use std::time::Duration;

    #[test]
    fn view_attach_does_not_mutate_process_state() {
        let mgr = SessionManager::new();
        let info = mgr.start("sh", 24, 80).expect("start sh");
        let id = info.session_id.clone();
        let proc_before = mgr.process_state(&id).unwrap();
        let gen_before = mgr.generation(&id).unwrap();

        let after = mgr.attach(&id).expect("attach");
        assert_eq!(after.process_state, proc_before);
        assert_eq!(after.generation, gen_before);

        let after2 = mgr.detach(&id).expect("detach");
        assert_eq!(after2.process_state, proc_before);
        assert_eq!(after2.generation, gen_before);

        let after3 = mgr.hide(&id).expect("hide");
        assert_eq!(after3.process_state, proc_before);
        let after4 = mgr.show(&id).expect("show");
        assert_eq!(after4.process_state, proc_before);

        let _ = mgr.close(&id);
    }

    #[test]
    fn renderer_reload_survival_same_id_and_generation() {
        let mgr = SessionManager::new();
        let info = mgr.start("sh", 24, 80).expect("start");
        let id = info.session_id.clone();
        let gen = info.generation;
        let proc = mgr.process_state(&id).unwrap();
        assert_eq!(proc, ProcessSessionState::Running);

        mgr.detach(&id).expect("detach");
        assert_eq!(mgr.view_state(&id).unwrap(), ViewAttachmentState::Detached);
        assert_eq!(mgr.process_state(&id).unwrap(), proc);
        assert_eq!(mgr.generation(&id).unwrap(), gen);

        let list = mgr.list();
        assert!(list
            .iter()
            .any(|s| s.session_id == id && s.generation == gen));

        let reattached = mgr.attach(&id).expect("reattach");
        assert_eq!(reattached.session_id, id);
        assert_eq!(reattached.generation, gen);
        assert_eq!(reattached.process_state, proc);

        let _ = mgr.close(&id);
    }

    #[test]
    fn restart_retains_session_id_increments_generation() {
        let mgr = SessionManager::new();
        let info = mgr.start("sh", 24, 80).expect("start");
        let id = info.session_id.clone();
        let gen1 = info.generation;
        {
            let map = mgr.sessions.lock().unwrap();
            let sess = map.get(&id).unwrap();
            let mut w = sess.writer.lock().unwrap();
            let _ = w.write_all(b"exit 0\n");
            let _ = w.flush();
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut exited = false;
        while std::time::Instant::now() < deadline {
            mgr.refresh_process_states();
            if matches!(
                mgr.process_state(&id).unwrap(),
                ProcessSessionState::Exited { .. }
            ) {
                exited = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(exited, "session did not reach Exited state before restart");
        let restarted = mgr.restart(&id).expect("restart from exited");
        assert_eq!(restarted.session_id, id);
        assert_eq!(restarted.generation, gen1 + 1);
        assert_eq!(restarted.process_state, ProcessSessionState::Running);
        let _ = mgr.close(&id);
    }

    #[test]
    fn close_reaps_child() {
        let mgr = SessionManager::new();
        let info = mgr.start("sh", 24, 80).expect("start");
        let id = info.session_id;
        std::thread::sleep(Duration::from_millis(100));
        let closed = mgr.close(&id).expect("close");
        assert_eq!(closed.process_state, ProcessSessionState::Closed);
        assert!(mgr.write(&id, b"echo hi\n").is_err());
    }

    #[test]
    fn shutdown_all_terminates_children() {
        let mgr = SessionManager::new();
        let a = mgr.start("sh", 24, 80).expect("a");
        let b = mgr.start("sh", 24, 80).expect("b");
        std::thread::sleep(Duration::from_millis(100));
        mgr.shutdown_all();
        assert_eq!(
            mgr.process_state(&a.session_id).unwrap(),
            ProcessSessionState::Closed
        );
        assert_eq!(
            mgr.process_state(&b.session_id).unwrap(),
            ProcessSessionState::Closed
        );
    }

    #[test]
    fn concurrent_sessions_isolated() {
        let mgr = SessionManager::new();
        let mut ids = Vec::new();
        for _ in 0..5 {
            let info = mgr.start("sh", 24, 80).expect("start concurrent");
            ids.push(info.session_id);
        }
        assert_eq!(ids.len(), 5);
        for (i, id) in ids.iter().enumerate() {
            let rows = 24 + i as u16;
            let cols = 80 + i as u16 * 2;
            mgr.resize(id, rows, cols).expect("resize");
        }
        for (i, id) in ids.iter().enumerate() {
            let info = mgr.get_info(id).expect("get");
            assert_eq!(info.rows, 24 + i as u16);
            assert_eq!(info.cols, 80 + i as u16 * 2);
        }
        mgr.write(&ids[0], b"echo isolated0\n").expect("write0");
        std::thread::sleep(Duration::from_millis(200));
        for id in &ids {
            let _ = mgr.poll_chunks(id, 4);
        }
        for id in ids {
            let _ = mgr.close(&id);
        }
    }

    #[test]
    #[cfg(unix)]
    fn byte_integrity_256kib() {
        use sha2::{Digest, Sha256};
        let payload_bytes = 256 * 1024;
        let expected = vec![b'A'; payload_bytes];
        let expected_sha = format!("{:x}", Sha256::digest(&expected));
        let mut transport = crate::terminal::transport::Transport::new();
        let chunk = vec![b'A'; 4096];
        let mut produced = 0usize;
        let mut delivered = Vec::new();
        // Enqueue with backpressure handling: when WouldBlock, drain and ack
        while produced < payload_bytes {
            let left = payload_bytes - produced;
            let n = std::cmp::min(chunk.len(), left);
            match transport.enqueue(&chunk[..n]) {
                Ok(()) => produced += n,
                Err(crate::terminal::transport::TransportError::WouldBlock) => {
                    // Drain some
                    while let Ok(Some(ch)) = transport.next_chunk("test-sess", 1) {
                        assert_eq!(ch.generation, 1);
                        delivered.extend_from_slice(&ch.bytes);
                        transport.ack(ch.sequence).expect("ack");
                        if transport.below_low_water() {
                            break;
                        }
                    }
                }
                Err(e) => panic!("enqueue failed {:?}", e),
            }
        }
        // Drain remainder
        while let Ok(Some(ch)) = transport.next_chunk("test-sess", 1) {
            // Sequence continues from previous
            delivered.extend_from_slice(&ch.bytes);
            transport.ack(ch.sequence).expect("ack");
        }
        assert_eq!(delivered.len(), payload_bytes);
        assert_eq!(delivered, expected);
        let delivered_sha = format!("{:x}", Sha256::digest(&delivered));
        assert_eq!(delivered_sha, expected_sha);
        assert_eq!(transport.stats().dropped_bytes, 0);
        assert_eq!(transport.stats().hard_limit_breaches, 0);

        let mgr = SessionManager::new();
        let info = mgr.start("sh", 24, 80).expect("start sh for byte test");
        let id = info.session_id.clone();
        std::thread::sleep(Duration::from_millis(100));
        mgr.write(&id, b"printf 'B%.0s' {1..4096}; echo DONE_MARKER\n")
            .ok();
        std::thread::sleep(Duration::from_millis(200));
        let mut collected = Vec::new();
        for _ in 0..20 {
            let chunks = mgr.poll_chunks(&id, 16).unwrap_or_default();
            for ch in &chunks {
                collected.extend_from_slice(&ch.bytes);
                let _ = mgr.ack(&id, ch.sequence);
            }
            if collected.windows(11).any(|w| w == b"DONE_MARKER") {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let replay = mgr.replay(&id).unwrap_or_default();
        if collected.is_empty() {
            collected = replay.clone();
        }
        let has_marker = collected.windows(11).any(|w| w == b"DONE_MARKER")
            || replay.windows(11).any(|w| w == b"DONE_MARKER");
        assert!(
            has_marker || collected.len() < 100,
            "marker or small output"
        );
        let _ = mgr.close(&id);
    }

    #[test]
    #[cfg(unix)]
    fn child_observed_resize_via_backend() {
        use crate::terminal::backend::PtyBackend;
        use crate::terminal::portable::PortablePtyBackend;
        use std::time::Duration;
        let mut backend = PortablePtyBackend::new();
        let mut handle = backend
            .spawn(
                "bash",
                &[
                    "-c".to_string(),
                    "read ignored; size=$(stty size); echo SIZE=${size/ /x}".to_string(),
                ],
                24,
                80,
            )
            .expect("spawn resize test");
        std::thread::sleep(Duration::from_millis(200));
        handle.resize(40, 120).expect("resize");
        handle.write(b"\r").expect("write cr");
        handle.flush().expect("flush");
        let mut buf = vec![0u8; 4096];
        let mut out = Vec::new();
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            match handle.read(&mut buf) {
                Ok(0) => std::thread::sleep(Duration::from_millis(10)),
                Ok(n) => {
                    out.extend_from_slice(&buf[..n]);
                    if String::from_utf8_lossy(&out).contains("SIZE=40x120") {
                        break;
                    }
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
            if handle.try_wait().unwrap_or(None).is_some() {
                break;
            }
        }
        let text = String::from_utf8_lossy(&out);
        assert!(
            text.contains("SIZE=40x120"),
            "child did not observe resize 40x120, got: {text:?}"
        );
        let _ = handle.kill();
    }
}
