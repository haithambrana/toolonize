//! Production terminal commands — typed, capability-scoped.
//!
//! Semantic mapping (Tauri 2 invoke identifiers are snake_case function names;
//! the logical namespace `terminal::*` is preserved in docs and permission ids):
//! - `terminal::profiles` → `terminal_profiles`
//! - `terminal::start`    → `terminal_start`
//! - `terminal::list`     → `terminal_list`
//! - `terminal::attach`   → `terminal_attach`
//! - `terminal::detach`   → `terminal_detach`
//! - `terminal::write`    → `terminal_write`
//! - `terminal::resize`   → `terminal_resize`
//! - `terminal::ack`      → `terminal_ack`
//! - `terminal::close`    → `terminal_close`
//! - `terminal::restart`  → `terminal_restart`
//! - `terminal::poll`     → `terminal_poll` (chunk delivery, internal)
//! - `terminal::replay`   → `terminal_replay` (reattach replay)

use serde::{Deserialize, Serialize};

use crate::terminal::error::TerminalError;
use crate::terminal::manager::{global_manager, SessionInfo};
use crate::terminal::profiles::TerminalProfile;
use crate::terminal::transport::OutputChunk;

// ----- Request / Response types (typed) -----

#[derive(Debug, Deserialize)]
pub struct TerminalStartRequest {
    pub profile_id: String,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Serialize)]
pub struct TerminalStartResponse {
    pub session: SessionInfo,
}

#[derive(Debug, Deserialize)]
pub struct TerminalWriteRequest {
    pub session_id: String,
    /// Bytes as base64 or raw vec? For M3, frontend sends Vec<u8> as number array.
    /// Use Vec<u8> directly via serde (JSON numbers) — bounded, validated.
    pub data: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalResizeRequest {
    pub session_id: String,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Deserialize)]
pub struct TerminalAckRequest {
    pub session_id: String,
    pub sequence: u64,
}

#[derive(Debug, Deserialize)]
pub struct TerminalSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct TerminalListResponse {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Serialize)]
pub struct TerminalPollResponse {
    pub chunks: Vec<OutputChunk>,
    /// If replay was truncated due to cap, frontend surfaces warning.
    pub replay_truncated: bool,
    pub replay_discarded_bytes: u64,
    pub next_sequence: u64,
}

#[derive(Debug, Serialize)]
pub struct TerminalAttachResponse {
    pub session: SessionInfo,
    pub attachment_epoch: u64,
    pub next_sequence: u64,
    pub acknowledged_up_to: Option<u64>,
    pub replay_truncated: bool,
    pub replay_discarded_bytes: u64,
}

#[derive(Debug, Deserialize)]
pub struct TerminalPollRequest {
    pub session_id: String,
    pub max_chunks: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct TerminalReplayResponse {
    pub bytes: Vec<u8>,
    pub truncated: bool,
    pub discarded_bytes: u64,
    pub next_sequence: u64,
    pub attachment_epoch: u64,
}

// ----- Commands -----

/// `terminal::profiles` — returns sanitized opaque profile metadata only.
#[tauri::command]
pub fn terminal_profiles() -> Vec<TerminalProfile> {
    crate::terminal::available_profiles()
}

/// `terminal::start` — accepts only opaque profile_id + dimensions.
/// Executable construction is Rust-owned.
#[tauri::command]
pub fn terminal_start(request: TerminalStartRequest) -> Result<TerminalStartResponse, String> {
    let mgr = global_manager();
    mgr.start(&request.profile_id, request.rows, request.cols)
        .map(|session| TerminalStartResponse { session })
        .map_err(|e: TerminalError| e.to_string())
}

/// `terminal::list` — lists active Rust-owned sessions (for reattach after reload).
#[tauri::command]
pub fn terminal_list() -> TerminalListResponse {
    let mgr = global_manager();
    TerminalListResponse {
        sessions: mgr.list(),
    }
}

/// `terminal::attach` — view attach, does not mutate process state/generation.
/// H2/H3: returns attachment cursor (epoch, next_sequence) for renderer reload protocol.
#[tauri::command]
pub fn terminal_attach(request: TerminalSessionRequest) -> Result<TerminalAttachResponse, String> {
    let mgr = global_manager();
    let (session, attach) = mgr
        .attach_with_info(&request.session_id)
        .map_err(|e| e.to_string())?;
    Ok(TerminalAttachResponse {
        session,
        attachment_epoch: attach.attachment_epoch,
        next_sequence: attach.next_sequence,
        acknowledged_up_to: attach.acknowledged_up_to,
        replay_truncated: attach.replay_truncated,
        replay_discarded_bytes: attach.replay_discarded_bytes,
    })
}

/// `terminal::detach`
#[tauri::command]
pub fn terminal_detach(request: TerminalSessionRequest) -> Result<SessionInfo, String> {
    let mgr = global_manager();
    mgr.detach(&request.session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn terminal_hide(request: TerminalSessionRequest) -> Result<SessionInfo, String> {
    let mgr = global_manager();
    mgr.hide(&request.session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn terminal_show(request: TerminalSessionRequest) -> Result<SessionInfo, String> {
    let mgr = global_manager();
    mgr.show(&request.session_id).map_err(|e| e.to_string())
}

/// `terminal::write` — forwards bytes to existing session only.
#[tauri::command]
pub fn terminal_write(request: TerminalWriteRequest) -> Result<(), String> {
    if request.data.len() > 16 * 1024 {
        return Err(TerminalError::invalid_input("write payload too large").to_string());
    }
    let mgr = global_manager();
    mgr.write(&request.session_id, &request.data)
        .map_err(|e| e.to_string())
}

/// `terminal::resize` — validated, then backend resize.
#[tauri::command]
pub fn terminal_resize(request: TerminalResizeRequest) -> Result<(), String> {
    let mgr = global_manager();
    mgr.resize(&request.session_id, request.rows, request.cols)
        .map_err(|e| e.to_string())
}

/// `terminal::ack` — frontend acknowledges sequence after xterm write completes.
#[tauri::command]
pub fn terminal_ack(request: TerminalAckRequest) -> Result<(), String> {
    let mgr = global_manager();
    mgr.ack(&request.session_id, request.sequence)
        .map_err(|e| e.to_string())
}

/// `terminal::close` — Running -> Stopping -> Closed, reap child.
#[tauri::command]
pub fn terminal_close(request: TerminalSessionRequest) -> Result<SessionInfo, String> {
    let mgr = global_manager();
    mgr.close(&request.session_id).map_err(|e| e.to_string())
}

/// `terminal::restart` — retains SessionId, increments generation.
#[tauri::command]
pub fn terminal_restart(request: TerminalSessionRequest) -> Result<SessionInfo, String> {
    let mgr = global_manager();
    mgr.restart(&request.session_id).map_err(|e| e.to_string())
}

/// Chunk polling — frontend fetches sequenced OutputChunk(s). Each chunk must be ack'd.
#[tauri::command]
pub fn terminal_poll(request: TerminalPollRequest) -> Result<TerminalPollResponse, String> {
    let mgr = global_manager();
    let max = request.max_chunks.unwrap_or(16).min(64);
    let chunks = mgr
        .poll_chunks(&request.session_id, max)
        .map_err(|e| e.to_string())?;
    let info = mgr
        .replay_with_info(&request.session_id)
        .map_err(|e| e.to_string())?;
    Ok(TerminalPollResponse {
        chunks,
        replay_truncated: info.truncated,
        replay_discarded_bytes: info.discarded_bytes,
        next_sequence: info.next_sequence,
    })
}

/// `terminal::replay` — bounded server-side replay for renderer reload reattachment.
/// H2/H5: returns replay watermark and truncation metadata.
#[tauri::command]
pub fn terminal_replay(request: TerminalSessionRequest) -> Result<TerminalReplayResponse, String> {
    let mgr = global_manager();
    let replay = mgr
        .replay_with_info(&request.session_id)
        .map_err(|e| e.to_string())?;
    Ok(TerminalReplayResponse {
        bytes: replay.bytes,
        truncated: replay.truncated,
        discarded_bytes: replay.discarded_bytes,
        next_sequence: replay.next_sequence,
        attachment_epoch: replay.attachment_epoch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_do_not_expose_raw_exec() {
        let profiles = terminal_profiles();
        for p in profiles {
            assert!(!p.id.contains('/'));
            // Serialize and check no path leakage
            let v = serde_json::to_value(&p).unwrap();
            let s = v.to_string();
            assert!(!s.contains("/home"));
            assert!(!s.contains("C:\\"));
        }
    }

    #[test]
    fn start_requires_opaque_id_not_raw_exec() {
        let mgr = global_manager();
        // Attempt raw exec via profile_id containing path traversal should fail
        let result = mgr.start("../../bin/sh", 24, 80);
        assert!(result.is_err());

        // Unknown profile
        let result = mgr.start("nonexistent_profile_xyz", 24, 80);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_dimensions_rejected() {
        let mgr = global_manager();
        let r = mgr.start("sh", 0, 80);
        assert!(r.is_err());
        let r = mgr.start("sh", 24, 0);
        assert!(r.is_err());
        let r = mgr.start("sh", 501, 80);
        assert!(r.is_err());
    }
}
