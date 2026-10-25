//! Terminal session commands. `session_start` names a workspace member; the
//! execution policy builds the process. Input is a byte stream only.

use super::{blocking, CoreState};
use crate::session::{AttachInfo, SessionEvent, SessionInfo, SessionSink, ViewEvent, MAX_WRITE};
use std::sync::Arc;
use tauri::ipc::{Channel, InvokeResponseBody};

struct ViewSink {
    output: Channel<InvokeResponseBody>,
    events: Channel<SessionEvent>,
}

impl SessionSink for ViewSink {
    fn output(&self, frame: Vec<u8>) -> bool {
        self.output.send(InvokeResponseBody::Raw(frame)).is_ok()
    }
    fn event(&self, event: &SessionEvent) -> bool {
        self.events.send(event.clone()).is_ok()
    }
}

fn err(e: crate::session::SessionError) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn session_start(
    core: CoreState<'_>,
    workspace_id: String,
    member_id: String,
    rows: u16,
    cols: u16,
) -> Result<SessionInfo, String> {
    blocking(&core, move |c| {
        c.start_session(&workspace_id, &member_id, rows, cols)
    })
    .await
}

#[tauri::command]
pub async fn session_write(
    core: CoreState<'_>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    if data.len() > MAX_WRITE {
        return Err("input exceeds the per-write limit".into());
    }
    core.sessions.write(&session_id, &data).map_err(err)
}

#[tauri::command]
pub async fn session_resize(
    core: CoreState<'_>,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<SessionInfo, String> {
    core.sessions.resize(&session_id, rows, cols).map_err(err)
}

#[tauri::command]
pub async fn session_attach(
    core: CoreState<'_>,
    session_id: String,
    from_offset: u64,
    output: Channel<InvokeResponseBody>,
    events: Channel<SessionEvent>,
) -> Result<AttachInfo, String> {
    core.attach(
        &session_id,
        from_offset,
        Arc::new(ViewSink { output, events }),
    )
}

#[tauri::command]
pub async fn session_detach(
    core: CoreState<'_>,
    session_id: String,
    attachment_id: u64,
) -> Result<SessionInfo, String> {
    core.sessions
        .detach(&session_id, attachment_id)
        .map_err(err)
}

#[tauri::command]
pub async fn session_ack(
    core: CoreState<'_>,
    session_id: String,
    attachment_id: u64,
    offset: u64,
) -> Result<(), String> {
    core.sessions
        .ack(&session_id, attachment_id, offset)
        .map_err(err)
}

#[tauri::command]
pub async fn session_view(
    core: CoreState<'_>,
    session_id: String,
    event: ViewEvent,
) -> Result<SessionInfo, String> {
    core.sessions.view_event(&session_id, event).map_err(err)
}

#[tauri::command]
pub async fn session_stop(core: CoreState<'_>, session_id: String) -> Result<SessionInfo, String> {
    blocking(&core, move |c| c.sessions.stop(&session_id).map_err(err)).await
}

#[tauri::command]
pub async fn session_restart(
    core: CoreState<'_>,
    session_id: String,
) -> Result<SessionInfo, String> {
    blocking(&core, move |c| c.restart_session(&session_id)).await
}

#[tauri::command]
pub async fn session_reconnect(
    core: CoreState<'_>,
    session_id: String,
) -> Result<SessionInfo, String> {
    blocking(&core, move |c| c.reconnect_session(&session_id)).await
}

#[tauri::command]
pub async fn session_close(core: CoreState<'_>, session_id: String) -> Result<(), String> {
    blocking(&core, move |c| c.close_session(&session_id)).await
}

#[tauri::command]
pub async fn session_list(core: CoreState<'_>) -> Result<Vec<SessionInfo>, String> {
    Ok(core.sessions.list())
}
