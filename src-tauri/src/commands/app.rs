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

/// Write a diagnostics report into the Downloads folder (no names, no output).
#[tauri::command]
pub async fn diagnostics_export(core: CoreState<'_>) -> Result<String, String> {
    blocking(&core, move |c| c.export_diagnostics()).await
}

/// Delete every saved terminal output (session memory); returns how many.
#[tauri::command]
pub async fn memory_forget(core: CoreState<'_>) -> Result<usize, String> {
    blocking(&core, move |c| Ok(c.forget_terminal_content())).await
}

/// Is ToolOnize set to open at login (ADR-017)?
#[tauri::command]
pub async fn autostart_get() -> Result<bool, String> {
    Ok(
        tauri::async_runtime::spawn_blocking(crate::autostart::is_enabled)
            .await
            .unwrap_or(false),
    )
}

/// Turn opening at login on or off; returns the resulting state.
#[tauri::command]
pub async fn autostart_set(core: CoreState<'_>, enabled: bool) -> Result<bool, String> {
    blocking(&core, move |c| {
        crate::autostart::set_enabled(enabled)?;
        c.record_autostart(enabled);
        Ok(crate::autostart::is_enabled())
    })
    .await
}

/// Is a newer ToolOnize available (ADR-019)? Never takes a URL.
#[tauri::command]
pub async fn update_check(app: tauri::AppHandle) -> Result<crate::updates::UpdateStatus, String> {
    crate::updates::check(&app).await
}

/// Download, verify and install the available update, then restart.
#[tauri::command]
pub async fn update_install(app: tauri::AppHandle, core: CoreState<'_>) -> Result<(), String> {
    let core = core.inner().clone();
    crate::updates::install(app, core).await
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

/// Show a desktop notification (terminal exited, bell, program request).
/// The core applies the Settings switch, a rate limit and length limits.
#[tauri::command]
pub async fn desktop_notify(
    app: tauri::AppHandle,
    core: CoreState<'_>,
    title: String,
    body: String,
) -> Result<bool, String> {
    use tauri_plugin_notification::NotificationExt;
    let Some((title, body)) = core.prepare_notification(&title, &body) else {
        return Ok(false);
    };
    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        // No notification service (e.g. a bare X session): not an app error.
        crate::logging::warn("notify", &error.to_string());
        return Ok(false);
    }
    Ok(true)
}

/// Copy the local audit log (JSON lines) into the Downloads folder.
#[tauri::command]
pub async fn audit_export(core: CoreState<'_>) -> Result<String, String> {
    blocking(&core, move |c| c.export_audit()).await
}
