//! App-level commands: startup info, events, settings, audit, URL opening.

use super::{blocking, CoreState};
use crate::app::{AppEvent, EventSink, StartupInfo};
use crate::audit::AuditEntry;
use crate::settings::Settings;
use std::sync::Arc;
use tauri::ipc::Channel;

struct ChannelSink(Channel<AppEvent>);

impl EventSink for ChannelSink {
    fn send(&self, event: &AppEvent) -> bool {
        self.0.send(event.clone()).is_ok()
    }
}

#[tauri::command]
pub async fn app_startup_info(core: CoreState<'_>) -> Result<StartupInfo, String> {
    Ok(core.startup_info())
}

#[tauri::command]
pub async fn events_subscribe(
    core: CoreState<'_>,
    events: Channel<AppEvent>,
) -> Result<(), String> {
    core.subscribe(Arc::new(ChannelSink(events)));
    Ok(())
}

#[tauri::command]
pub async fn settings_get(core: CoreState<'_>) -> Result<Settings, String> {
    Ok(core.settings())
}

#[tauri::command]
pub async fn settings_update(core: CoreState<'_>, settings: Settings) -> Result<(), String> {
    blocking(&core, move |c| c.update_settings(settings)).await
}

#[tauri::command]
pub async fn audit_recent(core: CoreState<'_>, limit: u32) -> Result<Vec<AuditEntry>, String> {
    Ok(core.audit_recent(limit as usize))
}

#[tauri::command]
pub async fn open_url(core: CoreState<'_>, url: String) -> Result<(), String> {
    blocking(&core, move |c| c.open_url(&url)).await
}

/// Save terminal text into the Downloads folder; the core picks the path.
#[tauri::command]
pub async fn scrollback_export(
    core: CoreState<'_>,
    name: String,
    text: String,
) -> Result<String, String> {
    blocking(&core, move |c| c.export_scrollback(&name, &text)).await
}
