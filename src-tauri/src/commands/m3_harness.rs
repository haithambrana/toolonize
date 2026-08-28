//! M3 production WebView reload harness — verifies actual Tauri WebView + SessionManager.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct M3ReloadReport {
    pub tauri_boot_ok: bool,
    pub terminal_view_ready: bool,
    pub session_started: bool,
    pub before_reload_output_ok: bool,
    pub session_id_before: String,
    pub generation_before: u64,
    pub webview_reloaded: bool,
    pub session_listed: bool,
    pub reattached: bool,
    pub same_session_id: bool,
    pub same_generation: bool,
    pub replay_ok: bool,
    pub live_sequence_resumed: bool,
    pub after_reload_output_ok: bool,
    pub after_reload_input_ok: bool,
    pub after_reload_resize_ok: bool,
    pub close_ok: bool,
    pub app_exit_ok: bool,
}

fn report_is_valid(r: &M3ReloadReport) -> bool {
    r.tauri_boot_ok
        && r.terminal_view_ready
        && r.session_started
        && r.before_reload_output_ok
        && !r.session_id_before.is_empty()
        && r.webview_reloaded
        && r.session_listed
        && r.reattached
        && r.same_session_id
        && r.same_generation
        && r.replay_ok
        && r.live_sequence_resumed
        && r.after_reload_output_ok
        && r.after_reload_input_ok
        && r.after_reload_resize_ok
        && r.close_ok
        && r.app_exit_ok
}

fn exit_after_response(app: tauri::AppHandle, code: i32) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        app.exit(code);
        std::process::exit(code);
    });
}

#[tauri::command]
pub fn m3_complete(app: tauri::AppHandle, report: M3ReloadReport) -> Result<(), String> {
    let valid = report_is_valid(&report);
    let json = serde_json::to_string(&report).map_err(|e| e.to_string())?;
    println!("M3_REAL_WEBVIEW_REPORT={json}");
    if valid {
        exit_after_response(app, 0);
        Ok(())
    } else {
        eprintln!("M3_REAL_WEBVIEW_INVALID={json}");
        exit_after_response(app, 1);
        Err("M3 real WebView report failed validation".to_string())
    }
}

#[tauri::command]
pub fn m3_fail(app: tauri::AppHandle, message: String) -> Result<(), String> {
    eprintln!("M3_REAL_WEBVIEW_FAILURE={message}");
    exit_after_response(app, 1);
    Ok(())
}
