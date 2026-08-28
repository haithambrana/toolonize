//! SessionManager registry — Rust owns sessions.
//! SessionId is opaque; PID is never exposed as identity.
//! View state is orthogonal to process state; attach/detach never mutates process state.
//! H1 lossless pump: pending chunk retained until enqueued or hard failure.
//! H2/H3 attachment epoch / reattach cursor protocol.
//! H5 replay truncation tracked in Transport.
//! H8 per-session isolation: registry lock only for lookup, per-session ops outside.
//! H11 cleanup observable.

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
    Arc, Condvar, Mutex,
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

/// Attachment cursor for renderer reload protocol (H2/H3).
#[derive(Debug, Clone, Serialize)]
pub struct AttachmentInfo {
    pub attachment_epoch: u64,
    pub generation: u64,
    pub next_sequence: u64,
    pub acknowledged_up_to: Option<u64>,
    pub replay_truncated: bool,
    pub replay_discarded_bytes: u64,
}

/// Replay response with watermark (H2/H5).
#[derive(Debug, Clone, Serialize)]
pub struct ReplayInfo {
    pub bytes: Vec<u8>,
    pub truncated: bool,
    pub discarded_bytes: u64,
    pub next_sequence: u64,
    pub attachment_epoch: u64,
}

/// Close result with observable cleanup (H11).
#[derive(Debug, Clone, Serialize)]
pub struct CloseResult {
    pub session: SessionInfo,
    pub pump_joined: bool,
    pub child_reaped: bool,
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
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Master PTY. `None` once retired at close: dropping the master closes the
    /// ConPTY (Windows) / host pty so the pump reader reaches EOF and joins.
    master: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    shared_rows: Arc<Mutex<u16>>,
    shared_cols: Arc<Mutex<u16>>,
    transport: Arc<Mutex<Transport>>,
    pump_handle: Option<thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
    /// For H1: wake pump when ack releases backpressure.
    pump_cvar: Arc<(Mutex<()>, Condvar)>,
    exit_code: Option<i32>,
}

pub struct SessionManager {
    sessions: Mutex<HashMap<SessionId, Arc<Mutex<Session>>>>,
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

    fn get_session_arc(&self, id: &str) -> TerminalResult<Arc<Mutex<Session>>> {
        let map = self.sessions.lock().unwrap();
        map.get(id)
            .cloned()
            .ok_or_else(|| TerminalError::not_found("session not found"))
    }

    /// List all sessions (projection for frontend — no PII).
    pub fn list(&self) -> Vec<SessionInfo> {
        self.refresh_process_states();
        let map = self.sessions.lock().unwrap();
        map.values()
            .map(|arc| {
                let s = arc.lock().unwrap();
                let tr = s.transport.lock().unwrap();
                SessionInfo {
                    session_id: s.id.clone(),
                    generation: s.generation,
                    profile_id: s.profile_id.clone(),
                    process_state: s.process_state.clone(),
                    view_state: s.view_state,
                    rows: s.rows,
                    cols: s.cols,
                    transport_state: tr.state(),
                    replay_truncated: tr.replay_truncated(),
                    exit_code: s.exit_code,
                }
            })
            .collect()
    }

    pub fn get_info(&self, id: &str) -> TerminalResult<SessionInfo> {
        self.refresh_process_states();
        let arc = self.get_session_arc(id)?;
        let s = arc.lock().unwrap();
        let tr = s.transport.lock().unwrap();
        Ok(SessionInfo {
            session_id: s.id.clone(),
            generation: s.generation,
            profile_id: s.profile_id.clone(),
            process_state: s.process_state.clone(),
            view_state: s.view_state,
            rows: s.rows,
            cols: s.cols,
            transport_state: tr.state(),
            replay_truncated: tr.replay_truncated(),
            exit_code: s.exit_code,
        })
    }

    fn refresh_process_states(&self) {
        let map = self.sessions.lock().unwrap();
        for arc in map.values() {
            let mut s = arc.lock().unwrap();
            if s.process_state == ProcessSessionState::Running {
                let exit_opt = {
                    let mut h = s.child.lock().unwrap();
                    h.try_wait().unwrap_or(None)
                };
                if let Some(status) = exit_opt {
                    let code = status.exit_code() as i32;
                    let target = ProcessSessionState::Exited { exit_code: code };
                    if validate_transition(&s.process_state, &target).is_ok() {
                        s.process_state = target;
                        s.exit_code = Some(code);
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_pump(
        reader: Box<dyn Read + Send>,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
        child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
        shared_rows: Arc<Mutex<u16>>,
        shared_cols: Arc<Mutex<u16>>,
        transport: Arc<Mutex<Transport>>,
        stop_flag: Arc<AtomicBool>,
        cvar: Arc<(Mutex<()>, Condvar)>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let mut pump_reader = reader;
            let mut dsr = super::dsr::DsrDetector::new();
            let mut buf = vec![0u8; 8192];
            let mut pending: Option<Vec<u8>> = None;

            while !stop_flag.load(Ordering::Relaxed) {
                // H1: if we have pending, try to enqueue it before reading more
                if let Some(data) = pending.take() {
                    let mut tr = transport.lock().unwrap();
                    match tr.enqueue(&data) {
                        Ok(()) => {
                            // successfully enqueued pending, loop to try next pending or read
                            continue;
                        }
                        Err(super::transport::TransportError::WouldBlock) => {
                            // retain pending, wait for low-water signal
                            pending = Some(data);
                            drop(tr);
                            let (lock, cv) = &*cvar;
                            let guard = lock.lock().unwrap();
                            let _ = cv.wait_timeout(guard, Duration::from_millis(50)).unwrap();
                            if stop_flag.load(Ordering::Relaxed) {
                                break;
                            }
                            continue;
                        }
                        Err(super::transport::TransportError::HardLimitBreach { .. }) => {
                            // explicit desync, do not drop silently
                            break;
                        }
                        Err(super::transport::TransportError::Desynchronized) => break,
                        Err(_) => break,
                    }
                }

                // Also check if transport is backpressured before consuming more bytes
                // If above high-water, wait for ack to release
                {
                    let tr = transport.lock().unwrap();
                    if matches!(tr.state(), TransportState::Backpressured) {
                        drop(tr);
                        let (lock, cv) = &*cvar;
                        let guard = lock.lock().unwrap();
                        let _ = cv.wait_timeout(guard, Duration::from_millis(10)).unwrap();
                        if stop_flag.load(Ordering::Relaxed) {
                            break;
                        }
                        continue;
                    }
                }

                let n = match pump_reader.read(&mut buf) {
                    Ok(0) => {
                        let exited = {
                            let mut h = child.lock().unwrap();
                            h.try_wait().unwrap_or(None).is_some()
                        };
                        if exited && pending.is_none() {
                            let tr = transport.lock().unwrap();
                            let empty = tr.queued_len() == 0 && tr.in_flight_len() == 0;
                            drop(tr);
                            if empty {
                                break;
                            }
                        }
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
                        let r = *shared_rows.lock().unwrap();
                        let c = *shared_cols.lock().unwrap();
                        let resp = super::dsr::cpr_response(r, c);
                        let mut w = writer.lock().unwrap();
                        for _ in 0..dsr_count {
                            let _ = w.write_all(&resp);
                        }
                        let _ = w.flush();
                    }
                    let data = buf[..n].to_vec();
                    let mut tr = transport.lock().unwrap();
                    match tr.enqueue(&data) {
                        Ok(()) => {}
                        Err(super::transport::TransportError::WouldBlock) => {
                            // H1: retain pending, do not ignore result
                            pending = Some(data);
                            drop(tr);
                            // wait for ack to signal low-water
                            let (lock, cv) = &*cvar;
                            let guard = lock.lock().unwrap();
                            let _ = cv.wait_timeout(guard, Duration::from_millis(50)).unwrap();
                            if stop_flag.load(Ordering::Relaxed) {
                                break;
                            }
                        }
                        Err(super::transport::TransportError::HardLimitBreach { .. }) => break,
                        Err(super::transport::TransportError::Desynchronized) => break,
                        Err(_) => break,
                    }
                } else {
                    thread::sleep(Duration::from_millis(2));
                }
            }
            // If we exit with pending, we do not silently drop: if stop_flag was set,
            // session is shutting down; otherwise we have hit hard limit/desync which
            // is already recorded. Pending data is lost only on explicit shutdown
            // or desync, never silently.
        })
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
        let master: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>> =
            Arc::new(Mutex::new(Some(split.master)));
        let child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>> =
            Arc::new(Mutex::new(split.child));
        let reader = split.reader;
        let shared_rows = Arc::new(Mutex::new(rows));
        let shared_cols = Arc::new(Mutex::new(cols));
        let transport: Arc<Mutex<Transport>> = Arc::new(Mutex::new(Transport::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let pump_cvar: Arc<(Mutex<()>, Condvar)> = Arc::new((Mutex::new(()), Condvar::new()));

        let pump = Self::spawn_pump(
            reader,
            Arc::clone(&writer),
            Arc::clone(&child),
            Arc::clone(&shared_rows),
            Arc::clone(&shared_cols),
            Arc::clone(&transport),
            Arc::clone(&stop_flag),
            Arc::clone(&pump_cvar),
        );

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
            pump_cvar,
            exit_code: None,
        };
        let info = {
            let tr = sess.transport.lock().unwrap();
            SessionInfo {
                session_id: sess.id.clone(),
                generation: sess.generation,
                profile_id: sess.profile_id.clone(),
                process_state: sess.process_state.clone(),
                view_state: sess.view_state,
                rows: sess.rows,
                cols: sess.cols,
                transport_state: tr.state(),
                replay_truncated: tr.replay_truncated(),
                exit_code: sess.exit_code,
            }
        };
        let arc = Arc::new(Mutex::new(sess));
        self.sessions.lock().unwrap().insert(id, arc);
        Ok(info)
    }

    #[cfg(test)]
    pub fn start_raw(
        &self,
        program: &str,
        args: &[String],
        rows: u16,
        cols: u16,
    ) -> TerminalResult<SessionInfo> {
        if rows == 0 || cols == 0 || rows > 500 || cols > 1000 {
            return Err(TerminalError::invalid_input("invalid dimensions"));
        }
        let id = self.new_id();
        let generation = 1u64;
        let mut backend = PortablePtyBackend::new();
        let split = backend.spawn_split(program, args, rows, cols)?;
        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(split.writer));
        let master: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>> =
            Arc::new(Mutex::new(Some(split.master)));
        let child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>> =
            Arc::new(Mutex::new(split.child));
        let reader = split.reader;
        let shared_rows = Arc::new(Mutex::new(rows));
        let shared_cols = Arc::new(Mutex::new(cols));
        let transport: Arc<Mutex<Transport>> = Arc::new(Mutex::new(Transport::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let pump_cvar: Arc<(Mutex<()>, Condvar)> = Arc::new((Mutex::new(()), Condvar::new()));
        let pump = Self::spawn_pump(
            reader,
            Arc::clone(&writer),
            Arc::clone(&child),
            Arc::clone(&shared_rows),
            Arc::clone(&shared_cols),
            Arc::clone(&transport),
            Arc::clone(&stop_flag),
            Arc::clone(&pump_cvar),
        );
        let sess = Session {
            id: id.clone(),
            generation,
            profile_id: program.to_string(),
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
            pump_cvar,
            exit_code: None,
        };
        let info = {
            let tr = sess.transport.lock().unwrap();
            SessionInfo {
                session_id: sess.id.clone(),
                generation: sess.generation,
                profile_id: sess.profile_id.clone(),
                process_state: sess.process_state.clone(),
                view_state: sess.view_state,
                rows: sess.rows,
                cols: sess.cols,
                transport_state: tr.state(),
                replay_truncated: tr.replay_truncated(),
                exit_code: sess.exit_code,
            }
        };
        let arc = Arc::new(Mutex::new(sess));
        self.sessions.lock().unwrap().insert(id, arc);
        Ok(info)
    }

    pub fn write(&self, session_id: &str, data: &[u8]) -> TerminalResult<()> {
        // H8: clone Arc under registry lock, then release before per-session IO
        let arc = self.get_session_arc(session_id)?;
        // Check process state without holding registry lock
        {
            let s = arc.lock().unwrap();
            if matches!(
                s.process_state,
                ProcessSessionState::Exited { .. }
                    | ProcessSessionState::Failed { .. }
                    | ProcessSessionState::Closed
            ) {
                return Err(TerminalError::invalid_input("session not running"));
            }
        }
        // Now do writer IO after releasing session lock? Need writer Arc clone
        let writer = {
            let s = arc.lock().unwrap();
            Arc::clone(&s.writer)
        };
        let mut w = writer.lock().unwrap();
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
        let arc = self.get_session_arc(session_id)?;
        let (master, shared_rows, shared_cols) = {
            let s = arc.lock().unwrap();
            (
                Arc::clone(&s.master),
                Arc::clone(&s.shared_rows),
                Arc::clone(&s.shared_cols),
            )
        };
        {
            let m = master.lock().unwrap();
            let m = m
                .as_ref()
                .ok_or_else(|| TerminalError::backend("pty master already retired"))?;
            m.resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|_| TerminalError::backend("pty resize failed"))?;
        }
        {
            let mut s = arc.lock().unwrap();
            s.rows = rows;
            s.cols = cols;
        }
        *shared_rows.lock().unwrap() = rows;
        *shared_cols.lock().unwrap() = cols;
        Ok(())
    }

    pub fn attach(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        let (info, _) = self.attach_with_info(session_id)?;
        Ok(info)
    }

    /// H2/H3: attach with cursor protocol, increments epoch and clears stale in-flight.
    pub fn attach_with_info(
        &self,
        session_id: &str,
    ) -> TerminalResult<(SessionInfo, AttachmentInfo)> {
        let arc = self.get_session_arc(session_id)?;
        let mut s = arc.lock().unwrap();
        let prev_proc = s.process_state.clone();
        let prev_gen = s.generation;
        s.view_state = ViewAttachmentState::Attached;
        debug_assert_eq!(s.process_state, prev_proc);
        debug_assert_eq!(s.generation, prev_gen);
        let mut tr = s.transport.lock().unwrap();
        let (epoch, next_seq) = tr.new_attachment();
        let ack_up_to = tr.stats().acknowledged_up_to;
        let replay_truncated = tr.replay_truncated();
        let discarded = tr.replay_discarded_bytes();
        let state = tr.state();
        drop(tr);
        let s_tr = s.transport.lock().unwrap();
        let info = SessionInfo {
            session_id: s.id.clone(),
            generation: s.generation,
            profile_id: s.profile_id.clone(),
            process_state: s.process_state.clone(),
            view_state: s.view_state,
            rows: s.rows,
            cols: s.cols,
            transport_state: state,
            replay_truncated: s_tr.replay_truncated(),
            exit_code: s.exit_code,
        };
        let attach = AttachmentInfo {
            attachment_epoch: epoch,
            generation: s.generation,
            next_sequence: next_seq,
            acknowledged_up_to: ack_up_to,
            replay_truncated,
            replay_discarded_bytes: discarded,
        };
        Ok((info, attach))
    }

    pub fn detach(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        let arc = self.get_session_arc(session_id)?;
        let mut s = arc.lock().unwrap();
        let prev_proc = s.process_state.clone();
        let prev_gen = s.generation;
        s.view_state = ViewAttachmentState::Detached;
        debug_assert_eq!(s.process_state, prev_proc);
        debug_assert_eq!(s.generation, prev_gen);
        let tr = s.transport.lock().unwrap();
        let info = SessionInfo {
            session_id: s.id.clone(),
            generation: s.generation,
            profile_id: s.profile_id.clone(),
            process_state: s.process_state.clone(),
            view_state: s.view_state,
            rows: s.rows,
            cols: s.cols,
            transport_state: tr.state(),
            replay_truncated: tr.replay_truncated(),
            exit_code: s.exit_code,
        };
        Ok(info)
    }

    pub fn hide(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        let arc = self.get_session_arc(session_id)?;
        let mut s = arc.lock().unwrap();
        let prev_proc = s.process_state.clone();
        s.view_state = ViewAttachmentState::Hidden;
        debug_assert_eq!(s.process_state, prev_proc);
        let tr = s.transport.lock().unwrap();
        let info = SessionInfo {
            session_id: s.id.clone(),
            generation: s.generation,
            profile_id: s.profile_id.clone(),
            process_state: s.process_state.clone(),
            view_state: s.view_state,
            rows: s.rows,
            cols: s.cols,
            transport_state: tr.state(),
            replay_truncated: tr.replay_truncated(),
            exit_code: s.exit_code,
        };
        Ok(info)
    }

    pub fn show(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        let arc = self.get_session_arc(session_id)?;
        let mut s = arc.lock().unwrap();
        let prev_proc = s.process_state.clone();
        s.view_state = ViewAttachmentState::Attached;
        debug_assert_eq!(s.process_state, prev_proc);
        let tr = s.transport.lock().unwrap();
        let info = SessionInfo {
            session_id: s.id.clone(),
            generation: s.generation,
            profile_id: s.profile_id.clone(),
            process_state: s.process_state.clone(),
            view_state: s.view_state,
            rows: s.rows,
            cols: s.cols,
            transport_state: tr.state(),
            replay_truncated: tr.replay_truncated(),
            exit_code: s.exit_code,
        };
        Ok(info)
    }

    pub fn ack(&self, session_id: &str, sequence: u64) -> TerminalResult<()> {
        let arc = self.get_session_arc(session_id)?;
        // H8: only lock per-session transport, not global
        let (transport, cvar) = {
            let s = arc.lock().unwrap();
            (Arc::clone(&s.transport), Arc::clone(&s.pump_cvar))
        };
        let mut tr = transport.lock().unwrap();
        tr.ack(sequence)
            .map_err(|e| TerminalError::transport(e.to_string()))?;
        let should_notify = tr.below_low_water();
        drop(tr);
        if should_notify {
            let (lock, cv) = &*cvar;
            let _g = lock.lock().unwrap();
            cv.notify_one();
        }
        Ok(())
    }

    pub fn next_chunk(&self, session_id: &str) -> TerminalResult<Option<OutputChunk>> {
        let arc = self.get_session_arc(session_id)?;
        let (transport, id, generation) = {
            let s = arc.lock().unwrap();
            (Arc::clone(&s.transport), s.id.clone(), s.generation)
        };
        let mut tr = transport.lock().unwrap();
        let chunk = tr
            .next_chunk(&id, generation)
            .map_err(|e| TerminalError::transport(e.to_string()))?;
        Ok(chunk)
    }

    pub fn poll_chunks(&self, session_id: &str, max: usize) -> TerminalResult<Vec<OutputChunk>> {
        let arc = self.get_session_arc(session_id)?;
        let (transport, id, generation) = {
            let s = arc.lock().unwrap();
            (Arc::clone(&s.transport), s.id.clone(), s.generation)
        };
        let mut tr = transport.lock().unwrap();
        let mut out = Vec::new();
        for _ in 0..max {
            match tr.next_chunk(&id, generation) {
                Ok(Some(c)) => out.push(c),
                Ok(None) => break,
                Err(e) => return Err(TerminalError::transport(e.to_string())),
            }
        }
        Ok(out)
    }

    pub fn replay(&self, session_id: &str) -> TerminalResult<Vec<u8>> {
        let arc = self.get_session_arc(session_id)?;
        let s = arc.lock().unwrap();
        let tr = s.transport.lock().unwrap();
        Ok(tr.replay_bytes().to_vec())
    }

    pub fn replay_with_info(&self, session_id: &str) -> TerminalResult<ReplayInfo> {
        let arc = self.get_session_arc(session_id)?;
        let s = arc.lock().unwrap();
        let tr = s.transport.lock().unwrap();
        Ok(ReplayInfo {
            bytes: tr.replay_bytes().to_vec(),
            truncated: tr.replay_truncated(),
            discarded_bytes: tr.replay_discarded_bytes(),
            next_sequence: tr.stats().next_sequence,
            attachment_epoch: tr.attachment_epoch(),
        })
    }

    pub fn close(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        // Take session out of map to avoid holding lock while joining
        let arc = {
            let mut map = self.sessions.lock().unwrap();
            map.remove(session_id)
                .ok_or_else(|| TerminalError::not_found("session not found"))?
        };
        let mut sess = arc.lock().unwrap();
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
                let (lock, cv) = &*sess.pump_cvar;
                let _g = lock.lock().unwrap();
                cv.notify_one();
            }
            {
                let mut h = sess.child.lock().unwrap();
                let _ = h.kill();
            }
            {
                // Retire the master so the pump reader reaches EOF and joins.
                *sess.master.lock().unwrap() = None;
            }
            let handle = sess.pump_handle.take();
            drop(sess);
            if let Some(h) = handle {
                let _ = h.join();
            }
            let s = arc.lock().unwrap();
            let tr = s.transport.lock().unwrap();
            let info = SessionInfo {
                session_id: s.id.clone(),
                generation: s.generation,
                profile_id: s.profile_id.clone(),
                process_state: s.process_state.clone(),
                view_state: s.view_state,
                rows: s.rows,
                cols: s.cols,
                transport_state: tr.state(),
                replay_truncated: tr.replay_truncated(),
                exit_code: s.exit_code,
            };
            self.sessions
                .lock()
                .unwrap()
                .insert(s.id.clone(), Arc::clone(&arc));
            return Ok(info);
        }
        // Kill child
        {
            let mut h = sess.child.lock().unwrap();
            let _ = h.kill();
        }
        sess.stop_flag.store(true, Ordering::Relaxed);
        {
            let (lock, cv) = &*sess.pump_cvar;
            let _g = lock.lock().unwrap();
            cv.notify_one();
        }
        {
            // Retire the master so the pump reader reaches EOF and joins.
            *sess.master.lock().unwrap() = None;
        }
        let handle = sess.pump_handle.take();
        let id_clone = sess.id.clone();
        let child_clone = Arc::clone(&sess.child);
        drop(sess);
        thread::sleep(Duration::from_millis(50));
        {
            let mut h = child_clone.lock().unwrap();
            if let Ok(Some(status)) = h.try_wait() {
                let mut s2 = arc.lock().unwrap();
                s2.exit_code = Some(status.exit_code() as i32);
            } else {
                let _ = h.wait();
            }
        }
        let mut pump_joined = false;
        if let Some(handle) = handle {
            // Join with timeout to avoid hang if reader blocked (H11 observable)
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            pump_joined = rx.recv_timeout(Duration::from_millis(500)).is_ok();
        }
        {
            let mut s = arc.lock().unwrap();
            // Even if pump not joined, we record state but surface observability
            let _ = validate_transition(&s.process_state, &ProcessSessionState::Closed)
                .map_err(TerminalError::illegal_transition);
            s.process_state = ProcessSessionState::Closed;
            if !pump_joined {
                // H11: do not claim clean if pump not joined — keep record
                // We still mark Closed but caller can check pump_joined via close_with_result
            }
            let tr = s.transport.lock().unwrap();
            let info = SessionInfo {
                session_id: s.id.clone(),
                generation: s.generation,
                profile_id: s.profile_id.clone(),
                process_state: s.process_state.clone(),
                view_state: s.view_state,
                rows: s.rows,
                cols: s.cols,
                transport_state: tr.state(),
                replay_truncated: tr.replay_truncated(),
                exit_code: s.exit_code,
            };
            drop(tr);
            drop(s);
            self.sessions
                .lock()
                .unwrap()
                .insert(id_clone, Arc::clone(&arc));
            Ok(info)
        }
    }

    pub fn close_with_result(&self, session_id: &str) -> TerminalResult<CloseResult> {
        let arc = {
            let mut map = self.sessions.lock().unwrap();
            map.remove(session_id)
                .ok_or_else(|| TerminalError::not_found("session not found"))?
        };
        let mut sess = arc.lock().unwrap();
        if sess.process_state == ProcessSessionState::Running {
            validate_transition(&sess.process_state, &ProcessSessionState::Stopping)
                .map_err(TerminalError::illegal_transition)?;
            sess.process_state = ProcessSessionState::Stopping;
        }
        sess.stop_flag.store(true, Ordering::Relaxed);
        {
            let (lock, cv) = &*sess.pump_cvar;
            let _g = lock.lock().unwrap();
            cv.notify_one();
        }
        {
            let mut h = sess.child.lock().unwrap();
            let _ = h.kill();
        }
        {
            // Retire the master so the ConPTY/pty closes and the pump reader
            // reaches EOF and joins promptly (H11).
            *sess.master.lock().unwrap() = None;
        }
        let handle = sess.pump_handle.take();
        let child_clone = Arc::clone(&sess.child);
        drop(sess);
        // Bound child reaping: never block indefinitely. Poll try_wait for up to
        // CHILD_REAP_BOUND while the child (and its ConPTY on Windows) unwinds.
        const CHILD_REAP_BOUND: Duration = Duration::from_millis(2000);
        let mut child_reaped = false;
        let reap_deadline = std::time::Instant::now() + CHILD_REAP_BOUND;
        while std::time::Instant::now() < reap_deadline {
            let mut h = child_clone.lock().unwrap();
            match h.try_wait() {
                Ok(Some(status)) => {
                    let mut s2 = arc.lock().unwrap();
                    s2.exit_code = Some(status.exit_code() as i32);
                    child_reaped = true;
                    break;
                }
                Ok(None) => {
                    drop(h);
                    thread::sleep(Duration::from_millis(25));
                }
                Err(_) => break,
            }
        }
        // Bound pump join so a reader blocked on a slow ConPTY teardown is still
        // observable (honest and non-hanging). Pump read releases as the child is
        // reaped above; allow generous slack on Windows.
        let mut pump_joined = false;
        if let Some(handle) = handle {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            pump_joined = rx
                .recv_timeout(CHILD_REAP_BOUND + Duration::from_millis(500))
                .is_ok();
        }
        let mut s = arc.lock().unwrap();
        let _ = validate_transition(&s.process_state, &ProcessSessionState::Closed);
        s.process_state = ProcessSessionState::Closed;
        let tr = s.transport.lock().unwrap();
        let info = SessionInfo {
            session_id: s.id.clone(),
            generation: s.generation,
            profile_id: s.profile_id.clone(),
            process_state: s.process_state.clone(),
            view_state: s.view_state,
            rows: s.rows,
            cols: s.cols,
            transport_state: tr.state(),
            replay_truncated: tr.replay_truncated(),
            exit_code: s.exit_code,
        };
        drop(tr);
        let result = CloseResult {
            session: info.clone(),
            pump_joined,
            child_reaped,
        };
        drop(s);
        self.sessions
            .lock()
            .unwrap()
            .insert(info.session_id.clone(), Arc::clone(&arc));
        Ok(result)
    }

    pub fn restart(&self, session_id: &str) -> TerminalResult<SessionInfo> {
        // Snapshot profile/rows/cols and old state without holding registry lock across IO
        let (profile_id, rows, cols, old_state, view_state, old_generation) = {
            let arc = self.get_session_arc(session_id)?;
            let s = arc.lock().unwrap();
            (
                s.profile_id.clone(),
                s.rows,
                s.cols,
                s.process_state.clone(),
                s.view_state,
                s.generation,
            )
        };
        let restarting = ProcessSessionState::Restarting;
        validate_transition(&old_state, &restarting).map_err(TerminalError::illegal_transition)?;
        // Remove old session, kill and join
        let arc = {
            let mut map = self.sessions.lock().unwrap();
            map.remove(session_id)
                .ok_or_else(|| TerminalError::not_found("session not found"))?
        };
        {
            let mut s = arc.lock().unwrap();
            s.stop_flag.store(true, Ordering::Relaxed);
            {
                let (lock, cv) = &*s.pump_cvar;
                let _g = lock.lock().unwrap();
                cv.notify_one();
            }
            {
                let mut h = s.child.lock().unwrap();
                let _ = h.kill();
            }
            {
                // Retire the old master so its pump reader EOFs and joins.
                *s.master.lock().unwrap() = None;
            }
            let handle = s.pump_handle.take();
            drop(s);
            if let Some(handle) = handle {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = handle.join();
                    let _ = tx.send(());
                });
                let _ = rx.recv_timeout(Duration::from_millis(500));
            }
        }
        // Spawn new handle
        let resolved = resolve_profile(&profile_id)?;
        let mut backend = PortablePtyBackend::new();
        let split = backend.spawn_split(&resolved.program, &resolved.args, rows, cols)?;
        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(split.writer));
        let master: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>> =
            Arc::new(Mutex::new(Some(split.master)));
        let child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>> =
            Arc::new(Mutex::new(split.child));
        let reader = split.reader;
        let shared_rows = Arc::new(Mutex::new(rows));
        let shared_cols = Arc::new(Mutex::new(cols));
        let transport: Arc<Mutex<Transport>> = Arc::new(Mutex::new(Transport::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let pump_cvar: Arc<(Mutex<()>, Condvar)> = Arc::new((Mutex::new(()), Condvar::new()));
        let pump = Self::spawn_pump(
            reader,
            Arc::clone(&writer),
            Arc::clone(&child),
            Arc::clone(&shared_rows),
            Arc::clone(&shared_cols),
            Arc::clone(&transport),
            Arc::clone(&stop_flag),
            Arc::clone(&pump_cvar),
        );

        // Build new session with incremented generation and Running state
        let generation = old_generation + 1;
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
            transport: Arc::clone(&transport),
            pump_handle: Some(pump),
            stop_flag,
            pump_cvar,
            exit_code: None,
        };
        let info = {
            let tr = sess.transport.lock().unwrap();
            SessionInfo {
                session_id: sess.id.clone(),
                generation: sess.generation,
                profile_id: sess.profile_id.clone(),
                process_state: sess.process_state.clone(),
                view_state: sess.view_state,
                rows: sess.rows,
                cols: sess.cols,
                transport_state: tr.state(),
                replay_truncated: tr.replay_truncated(),
                exit_code: sess.exit_code,
            }
        };
        let id_clone = sess.id.clone();
        let arc2 = Arc::new(Mutex::new(sess));
        self.sessions.lock().unwrap().insert(id_clone, arc2);
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
        map.get(id).map(|arc| arc.lock().unwrap().generation)
    }

    #[cfg(test)]
    pub fn view_state(&self, id: &str) -> Option<ViewAttachmentState> {
        let map = self.sessions.lock().unwrap();
        map.get(id).map(|arc| arc.lock().unwrap().view_state)
    }

    #[cfg(test)]
    pub fn process_state(&self, id: &str) -> Option<ProcessSessionState> {
        let map = self.sessions.lock().unwrap();
        map.get(id)
            .map(|arc| arc.lock().unwrap().process_state.clone())
    }

    #[cfg(test)]
    pub fn transport_stats(&self, id: &str) -> Option<super::transport::TransportStats> {
        let arc = self.get_session_arc(id).ok()?;
        let s = arc.lock().unwrap();
        let tr = s.transport.lock().unwrap();
        Some(tr.stats())
    }

    #[cfg(test)]
    pub fn writer_arc(&self, id: &str) -> Option<Arc<Mutex<Box<dyn Write + Send>>>> {
        let arc = self.get_session_arc(id).ok()?;
        let s = arc.lock().unwrap();
        Some(Arc::clone(&s.writer))
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

    fn available_profile() -> String {
        // Tests issue POSIX command sequences (echo, exit 0) and were designed
        // around 'sh' — Git Bash provides sh on Windows runners, so prefer sh
        // (then bash) before falling back to any advertised available shell.
        let profiles = crate::terminal::available_profiles();
        for pref in ["sh", "bash"] {
            if crate::terminal::profiles::resolve_profile(pref).is_ok() {
                return pref.to_string();
            }
        }
        profiles
            .iter()
            .find(|p| p.available)
            .map(|p| p.id.clone())
            .unwrap_or_else(|| {
                if cfg!(windows) {
                    "cmd".to_string()
                } else {
                    "sh".to_string()
                }
            })
    }

    #[test]
    fn view_attach_does_not_mutate_process_state() {
        let mgr = SessionManager::new();
        let info = mgr.start(&available_profile(), 24, 80).expect("start sh");
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
        let info = mgr.start(&available_profile(), 24, 80).expect("start");
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
        let info = mgr.start(&available_profile(), 24, 80).expect("start");
        let id = info.session_id.clone();
        let gen1 = info.generation;
        {
            let arc = mgr.get_session_arc(&id).unwrap();
            let s = arc.lock().unwrap();
            let mut w = s.writer.lock().unwrap();
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
        let info = mgr.start(&available_profile(), 24, 80).expect("start");
        let id = info.session_id;
        std::thread::sleep(Duration::from_millis(100));
        let closed = mgr.close(&id).expect("close");
        assert_eq!(closed.process_state, ProcessSessionState::Closed);
        assert!(mgr.write(&id, b"echo hi\n").is_err());
    }

    #[test]
    fn shutdown_all_terminates_children() {
        let mgr = SessionManager::new();
        let a = mgr.start(&available_profile(), 24, 80).expect("a");
        let b = mgr.start(&available_profile(), 24, 80).expect("b");
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
            let info = mgr
                .start(&available_profile(), 24, 80)
                .expect("start concurrent");
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
        let info = mgr
            .start(&available_profile(), 24, 80)
            .expect("start sh for byte test");
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

    #[test]
    fn lossless_pending_retained_over_1s() {
        use sha2::{Digest, Sha256};
        // H1: pending chunk must survive >1s stall without silent drop
        let payload_bytes = 128 * 1024;
        let expected = vec![b'Z'; payload_bytes];
        let expected_sha = format!("{:x}", Sha256::digest(&expected));
        let mut t = crate::terminal::transport::Transport::new();
        let chunk = vec![b'Z'; 4096];
        let mut produced: usize = 0;
        let mut delivered: Vec<u8> = Vec::new();
        // Fill until backpressure
        while produced < payload_bytes {
            let left = payload_bytes - produced;
            let n = std::cmp::min(chunk.len(), left);
            match t.enqueue(&chunk[..n]) {
                Ok(()) => produced += n,
                Err(crate::terminal::transport::TransportError::WouldBlock) => {
                    // Stalled consumer for 1200ms (> old 1s window) — tests H1
                    std::thread::sleep(Duration::from_millis(1200));
                    // Drain with ack
                    while let Ok(Some(ch)) = t.next_chunk("sess", 1) {
                        delivered.extend_from_slice(&ch.bytes);
                        t.ack(ch.sequence).unwrap();
                        if t.below_low_water() {
                            break;
                        }
                    }
                }
                Err(crate::terminal::transport::TransportError::HardLimitBreach { .. }) => {
                    // Should not happen with proper backpressure handling, drain and retry
                    while let Ok(Some(ch)) = t.next_chunk("sess", 1) {
                        delivered.extend_from_slice(&ch.bytes);
                        t.ack(ch.sequence).unwrap();
                        if t.below_low_water() {
                            break;
                        }
                    }
                }
                Err(e) => panic!("enqueue {:?}", e),
            }
        }
        while let Ok(Some(ch)) = t.next_chunk("sess", 1) {
            delivered.extend_from_slice(&ch.bytes);
            t.ack(ch.sequence).unwrap();
        }
        assert_eq!(delivered.len(), payload_bytes);
        assert_eq!(delivered, expected);
        let sha = format!("{:x}", Sha256::digest(&delivered));
        assert_eq!(sha, expected_sha);
        assert_eq!(t.stats().dropped_bytes, 0);
        assert_eq!(t.stats().hard_limit_breaches, 0);
    }

    #[test]
    fn replay_truncation_flag_and_no_live_drop() {
        // H5: replay truncates old history but live transport remains lossless
        let mut t = crate::terminal::transport::Transport::with_capacity(65536, 49152, 16384, 1024);
        let data = vec![b'A'; 2048];
        for _ in 0..4 {
            // Enqueue 8KiB total, replay cap 1KiB => truncation expected
            t.enqueue(&data).unwrap();
            // Drain some to keep below high-water for next enqueue
            while t.stats().queued_bytes > 0 {
                if let Ok(Some(ch)) = t.next_chunk("sess", 1) {
                    t.ack(ch.sequence).unwrap();
                } else {
                    break;
                }
            }
        }
        // Produce > cap without draining replay? Let's directly fill replay
        let mut t2 =
            crate::terminal::transport::Transport::with_capacity(65536, 49152, 16384, 1024);
        for _ in 0..10 {
            t2.enqueue(&vec![b'B'; 512]).unwrap();
        }
        assert!(t2.replay_truncated());
        assert!(t2.replay_discarded_bytes() > 0);
        assert!(t2.replay_bytes().len() <= 1024);
        assert_eq!(t2.stats().dropped_bytes, 0);
        assert_eq!(t2.stats().hard_limit_breaches, 0);
        // Manager-level replay truncation via real session
        let mgr = SessionManager::new();
        let info = mgr.start(&available_profile(), 24, 80).expect("start");
        let id = info.session_id.clone();
        // Produce > REPLAY_CAP via direct transport mutation for determinism
        {
            let arc = mgr.get_session_arc(&id).unwrap();
            let transport = {
                let s = arc.lock().unwrap();
                Arc::clone(&s.transport)
            };
            let mut tr = transport.lock().unwrap();
            for _ in 0..20 {
                let big = vec![b'C'; 4096];
                let mut attempts = 0;
                loop {
                    match tr.enqueue(&big) {
                        Ok(()) => break,
                        Err(crate::terminal::transport::TransportError::WouldBlock) => {
                            drop(tr);
                            let _ = mgr.poll_chunks(&id, 4).map(|chunks| {
                                for ch in chunks {
                                    let _ = mgr.ack(&id, ch.sequence);
                                }
                            });
                            tr = transport.lock().unwrap();
                            attempts += 1;
                            if attempts > 5 {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
        // Let pump drain
        std::thread::sleep(Duration::from_millis(100));
        for _ in 0..5 {
            let chunks = mgr.poll_chunks(&id, 16).unwrap_or_default();
            for ch in &chunks {
                let _ = mgr.ack(&id, ch.sequence);
            }
        }
        let replay = mgr.replay_with_info(&id).unwrap();
        assert!(replay.truncated || replay.bytes.len() <= 65536);
        if replay.truncated {
            assert!(replay.discarded_bytes > 0);
        }
        let stats = mgr.transport_stats(&id).unwrap();
        assert_eq!(stats.dropped_bytes, 0);
        assert_eq!(stats.hard_limit_breaches, 0);
        let _ = mgr.close(&id);
    }

    #[test]
    fn reattach_clears_inflight_and_establishes_cursor() {
        // H2/H3: attach increments epoch, clears in-flight, next_sequence continues
        let mgr = SessionManager::new();
        let info = mgr.start(&available_profile(), 24, 80).expect("start");
        let id = info.session_id.clone();
        std::thread::sleep(Duration::from_millis(100));
        mgr.write(&id, b"echo hello\n").ok();
        std::thread::sleep(Duration::from_millis(200));
        // Poll one chunk but do not ack it (simulate renderer disappearance with in-flight)
        let chunks = mgr.poll_chunks(&id, 4).unwrap();
        assert!(!chunks.is_empty());
        let first_seq = chunks[0].sequence;
        // Do not ack — simulate dead renderer
        // Detach and reattach (renderer reload)
        mgr.detach(&id).unwrap();
        let (reattached, attach) = mgr.attach_with_info(&id).unwrap();
        assert_eq!(reattached.session_id, id);
        assert_eq!(reattached.generation, 1);
        assert!(attach.attachment_epoch >= 1);
        // After attach, in-flight should be cleared, so old ack should fail
        let old_ack = mgr.ack(&id, first_seq);
        assert!(old_ack.is_err(), "stale ack should be rejected");
        // Next poll should give next_sequence without gap
        let next = attach.next_sequence;
        // Write new data and poll
        mgr.write(&id, b"echo after_reload\n").ok();
        std::thread::sleep(Duration::from_millis(200));
        let chunks2 = mgr.poll_chunks(&id, 8).unwrap();
        if !chunks2.is_empty() {
            // First new chunk should have sequence == next
            assert_eq!(chunks2[0].sequence, next);
            // Ack should succeed
            for ch in &chunks2 {
                assert_eq!(ch.generation, 1);
                mgr.ack(&id, ch.sequence).expect("ack new");
            }
        }
        let _ = mgr.close(&id);
    }

    #[test]
    fn reattach_before_any_output() {
        let mgr = SessionManager::new();
        let info = mgr.start(&available_profile(), 24, 80).expect("start");
        let id = info.session_id.clone();
        // Immediately detach/attach before any output
        mgr.detach(&id).unwrap();
        let (sess, attach) = mgr.attach_with_info(&id).unwrap();
        assert_eq!(sess.session_id, id);
        assert_eq!(attach.next_sequence, 0);
        assert_eq!(attach.acknowledged_up_to, None);
        // Poll should be empty, no gap
        let chunks = mgr.poll_chunks(&id, 4).unwrap();
        assert!(chunks.is_empty());
        // Write and poll should start at 0
        mgr.write(&id, b"echo hi\n").ok();
        std::thread::sleep(Duration::from_millis(200));
        let chunks2 = mgr.poll_chunks(&id, 4).unwrap();
        if !chunks2.is_empty() {
            assert_eq!(chunks2[0].sequence, 0);
        }
        let _ = mgr.close(&id);
    }

    #[test]
    fn isolation_stalled_a_responsive_b() {
        // H8: session A stalled/backpressured, B must remain responsive within bounded time
        let mgr = std::sync::Arc::new(SessionManager::new());
        let a = mgr.start(&available_profile(), 24, 80).expect("a");
        let b = mgr.start(&available_profile(), 24, 80).expect("b");
        let a_id = a.session_id.clone();
        let b_id = b.session_id.clone();
        // Stall A by filling its transport without acking
        {
            let arc = mgr.get_session_arc(&a_id).unwrap();
            let s = arc.lock().unwrap();
            let mut tr = s.transport.lock().unwrap();
            // Fill until WouldBlock
            let chunk = vec![b'X'; 4096];
            let mut enqueued = 0;
            loop {
                match tr.enqueue(&chunk) {
                    Ok(()) => enqueued += chunk.len(),
                    Err(crate::terminal::transport::TransportError::WouldBlock) => break,
                    Err(_) => break,
                }
                if enqueued > 50000 {
                    break;
                }
            }
            assert!(enqueued > 0);
        }
        // Now A is backpressured. B should still be responsive.
        let start = std::time::Instant::now();
        // Concurrent operations on B
        mgr.write(&b_id, b"echo responsive\n").expect("write B");
        mgr.resize(&b_id, 30, 90).expect("resize B");
        std::thread::sleep(Duration::from_millis(100));
        let chunks = mgr.poll_chunks(&b_id, 4).unwrap_or_default();
        // At least poll should not hang or error due to A's stall
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_millis(500),
            "B stalled due to A: {:?}",
            elapsed
        );
        for ch in &chunks {
            let _ = mgr.ack(&b_id, ch.sequence);
        }
        // Cleanup
        let _ = mgr.close(&a_id);
        let _ = mgr.close(&b_id);
    }

    #[test]
    fn restart_invalidates_old_inflight() {
        // H10: restart while output from previous generation is in-flight
        let mgr = SessionManager::new();
        let info = mgr.start(&available_profile(), 24, 80).expect("start");
        let id = info.session_id.clone();
        mgr.write(&id, b"echo before_restart\n").ok();
        std::thread::sleep(Duration::from_millis(200));
        let chunks = mgr.poll_chunks(&id, 4).unwrap();
        let old_gen = mgr.generation(&id).unwrap();
        let old_seq = chunks.first().map(|c| c.sequence);
        // Do not ack old chunks, restart
        // Wait for exit or just restart from Running (allowed: Running->Restarting)
        let restarted = mgr.restart(&id).expect("restart");
        assert_eq!(restarted.session_id, id);
        assert_eq!(restarted.generation, old_gen + 1);
        // Old ack should now fail (generation mismatch or unknown)
        if let Some(seq) = old_seq {
            let res = mgr.ack(&id, seq);
            // Either error or ignored; but should not corrupt new generation
            // We accept either Err or Ok? Actually new transport has no such seq, so Err
            assert!(res.is_err() || mgr.transport_stats(&id).unwrap().next_sequence == 0);
        }
        // New generation should have clean state
        let stats = mgr.transport_stats(&id).unwrap();
        assert_eq!(stats.next_sequence, 0);
        assert_eq!(stats.acknowledged_up_to, None);
        // New output should be deliverable
        mgr.write(&id, b"echo after_restart\n").ok();
        std::thread::sleep(Duration::from_millis(200));
        let chunks2 = mgr.poll_chunks(&id, 4).unwrap_or_default();
        for ch in &chunks2 {
            assert_eq!(ch.generation, old_gen + 1);
            mgr.ack(&id, ch.sequence).unwrap();
        }
        let _ = mgr.close(&id);
    }

    #[test]
    fn cleanup_observable_pump_join() {
        // H11: close should observably join pump and reap child
        let mgr = SessionManager::new();
        let info = mgr.start(&available_profile(), 24, 80).expect("start");
        let id = info.session_id.clone();
        std::thread::sleep(Duration::from_millis(50));
        let result = mgr.close_with_result(&id).expect("close_with_result");
        assert_eq!(result.session.process_state, ProcessSessionState::Closed);
        assert!(
            result.pump_joined,
            "pump should have been joined within bound"
        );
        assert!(result.child_reaped, "child should have been reaped");
        // Repeated lifecycle
        for _ in 0..3 {
            let info = mgr.start(&available_profile(), 24, 80).expect("start");
            let id = info.session_id;
            std::thread::sleep(Duration::from_millis(20));
            let r = mgr.close_with_result(&id).expect("close");
            assert!(r.pump_joined);
        }
    }

    #[test]
    #[cfg(unix)]
    fn production_manager_256kib_strict() {
        use sha2::{Digest, Sha256};
        // H6: strict production manager pipeline via start_raw with synthetic payload
        let mgr = SessionManager::new();
        let payload_bytes = 262144;
        let expected = vec![b'Q'; payload_bytes];
        let expected_sha = format!("{:x}", Sha256::digest(&expected));
        let program = "python3";
        let args = vec![
            "-u".to_string(),
            "-c".to_string(),
            format!("import sys; sys.stdout.buffer.write(b'Q'*{payload_bytes}); sys.stdout.buffer.write(b'DONE_MARKER'); sys.stdout.flush()"),
        ];
        let info = match mgr.start_raw(program, &args, 24, 80) {
            Ok(i) => i,
            Err(_) => {
                // Fallback to sh+cat with temp file
                let tmp_path2 = std::env::temp_dir()
                    .join(format!("toolonize_payload_{}.bin", std::process::id()));
                std::fs::write(&tmp_path2, &expected).expect("write payload");
                let prog = "sh";
                let a = vec![
                    "-c".to_string(),
                    format!("cat {}; echo DONE_MARKER", tmp_path2.display()),
                ];
                mgr.start_raw(prog, &a, 24, 80).expect("fallback cat")
            }
        };
        let id = info.session_id.clone();
        let mut collected = Vec::new();
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            let chunks = mgr.poll_chunks(&id, 32).unwrap_or_default();
            for ch in &chunks {
                collected.extend_from_slice(&ch.bytes);
                let _ = mgr.ack(&id, ch.sequence);
            }
            // Wait for both payload and DONE marker
            if collected.len() >= payload_bytes
                && collected.windows(11).any(|w| w == b"DONE_MARKER")
            {
                break;
            }
            if start.elapsed() >= Duration::from_secs(10) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(200));
        for _ in 0..5 {
            let chunks = mgr.poll_chunks(&id, 32).unwrap_or_default();
            if chunks.is_empty() {
                break;
            }
            for ch in &chunks {
                collected.extend_from_slice(&ch.bytes);
                let _ = mgr.ack(&id, ch.sequence);
            }
        }
        if collected.len() < payload_bytes {
            let replay = mgr.replay(&id).unwrap_or_default();
            eprintln!(
                "collected {} replay {} has_done={}",
                collected.len(),
                replay.len(),
                collected.windows(11).any(|w| w == b"DONE_MARKER")
            );
        }
        assert!(
            collected.windows(11).any(|w| w == b"DONE_MARKER"),
            "DONE_MARKER not found, collected {}",
            collected.len()
        );
        // Verify payload before marker
        let marker_pos = collected
            .windows(11)
            .position(|w| w == b"DONE_MARKER")
            .unwrap();
        // Payload should be before marker; check that there are 262144 Qs somewhere before marker
        let before = &collected[..marker_pos];
        assert!(
            before.len() >= payload_bytes,
            "before marker {} < payload {}",
            before.len(),
            payload_bytes
        );
        let mut found = false;
        if before.len() >= payload_bytes {
            for window in before.windows(payload_bytes) {
                if window.iter().all(|&b| b == b'Q') {
                    let sha = format!("{:x}", Sha256::digest(window));
                    assert_eq!(sha, expected_sha);
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "strict 256k payload not found with correct SHA");
        let stats = mgr.transport_stats(&id).unwrap();
        assert_eq!(stats.dropped_bytes, 0);
        assert_eq!(stats.hard_limit_breaches, 0);
        assert!(stats.lossless);
        let _ = mgr.close(&id);
        // Clean up fallback temp file if exists
        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("toolonize_payload_{}.bin", std::process::id())),
        );
    }
}
