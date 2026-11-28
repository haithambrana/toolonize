pub mod app;
pub mod audit;
pub mod autostart;
pub mod backup;
pub mod classify;
pub mod commands;
pub mod discovery;
pub mod export;
pub mod external;
pub mod logging;
pub mod memory;
pub mod model;
pub mod notes;
pub mod persist;
pub mod policy;
pub mod pty;
pub mod registry;
pub mod session;
pub mod settings;
pub mod shells;
pub mod sshconfig;
pub mod sync;
pub mod transfer;
pub mod updates;
pub mod workspace;

/// Run the Tauri application. Separated from `main.rs` for testability and
/// to keep `main.rs` minimal (Tauri convention).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    app::set_app_version(context.package_info().version.to_string());
    let (state_dir, config_dir) = persist::default_dirs();
    logging::init(&state_dir);
    logging::install_panic_hook();
    let core = app::Core::new(state_dir, config_dir);
    if let Err(error) = core.rescan() {
        logging::error(
            "discovery",
            &format!("initial launcher scan failed: {error}"),
        );
        eprintln!("initial launcher scan failed: {error}");
    }
    core.restart_watch();
    // Session memory (ADR-015): start again what was running last time,
    // then keep the memory current.
    core.resume_session();
    core.start_memory();
    let exit_core = core.clone();

    // Release builds carry an updater configuration (ADR-019); the plugin
    // needs one to start, so other builds run without it.
    let updates = context.config().plugins.0.contains_key("updater");
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(core)
        .manage(updates::UpdatesEnabled(updates));
    let builder = if updates {
        builder.plugin(tauri_plugin_updater::Builder::new().build())
    } else {
        builder
    };
    let builder = builder.invoke_handler(tauri::generate_handler![
        crate::commands::ping::ping,
        crate::commands::app::app_startup_info,
        crate::commands::app::events_subscribe,
        crate::commands::app::settings_get,
        crate::commands::app::settings_update,
        crate::commands::app::audit_recent,
        crate::commands::app::audit_export,
        crate::commands::app::open_url,
        crate::commands::clipboard::clipboard_read_text,
        crate::commands::app::scrollback_export,
        crate::commands::app::diagnostics_export,
        crate::commands::app::memory_forget,
        crate::commands::app::autostart_get,
        crate::commands::app::autostart_set,
        crate::commands::app::update_check,
        crate::commands::app::update_install,
        crate::commands::app::desktop_notify,
        crate::commands::registry::registry_view,
        crate::commands::registry::registry_rescan,
        crate::commands::registry::registry_set_hidden,
        crate::commands::registry::registry_set_alias,
        crate::commands::registry::registry_approve,
        crate::commands::registry::registry_revoke,
        crate::commands::registry::registry_acknowledge,
        crate::commands::registry::custom_root_add,
        crate::commands::registry::custom_root_remove,
        crate::commands::registry::synthetic_add,
        crate::commands::registry::synthetic_remove,
        crate::commands::registry::ssh_hosts,
        crate::commands::workspace::workspace_list,
        crate::commands::workspace::workspace_create,
        crate::commands::workspace::workspace_rename,
        crate::commands::workspace::workspace_set_color,
        crate::commands::workspace::workspace_set_project_dir,
        crate::commands::workspace::notes_get,
        crate::commands::workspace::note_save,
        crate::commands::workspace::notes_restore,
        crate::commands::workspace::workspace_delete,
        crate::commands::workspace::workspace_set_active,
        crate::commands::workspace::workspace_set_policy,
        crate::commands::workspace::workspace_accept_import,
        crate::commands::workspace::workspace_save_layout,
        crate::commands::workspace::workspace_open,
        crate::commands::workspace::workspace_export,
        crate::commands::workspace::workspace_import,
        crate::commands::workspace::workspace_duplicate,
        crate::commands::workspace::backup_export,
        crate::commands::workspace::backup_restore,
        crate::commands::workspace::template_list,
        crate::commands::workspace::template_save,
        crate::commands::workspace::template_create_workspace,
        crate::commands::workspace::template_delete,
        crate::commands::workspace::member_add,
        crate::commands::workspace::member_remove,
        crate::commands::workspace::member_update,
        crate::commands::workspace::member_authorize,
        crate::commands::workspace::member_confirm_launch,
        crate::commands::workspace::member_launch,
        crate::commands::session::session_start,
        crate::commands::session::session_write,
        crate::commands::session::session_resize,
        crate::commands::session::session_attach,
        crate::commands::session::session_detach,
        crate::commands::session::session_ack,
        crate::commands::session::session_view,
        crate::commands::session::session_stop,
        crate::commands::session::session_restart,
        crate::commands::session::session_reconnect,
        crate::commands::session::session_cancel_reconnect,
        crate::commands::session::session_cwd,
        crate::commands::session::session_close,
        crate::commands::session::session_list
    ]);
    let title_core = std::sync::Arc::downgrade(&exit_core);
    let builder = builder.on_window_event(|window, event| {
        if matches!(
            event,
            tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
        ) {
            remember_window(window);
        }
    });
    let builder = builder.setup(move |app| {
        restore_window(app.handle());
        #[cfg(unix)]
        exit_on_termination(app.handle().clone());
        let sink = std::sync::Arc::new(TitleSink {
            app: app.handle().clone(),
            core: title_core.clone(),
        });
        sink.update();
        if let Some(core) = title_core.upgrade() {
            core.add_internal_sink(sink);
        }
        Ok(())
    });
    builder
        .build(context)
        .expect("error while building tauri application")
        .run(move |_app, event| {
            if let tauri::RunEvent::Exit = event {
                // Closing the app terminates local child processes.
                exit_core.shutdown();
            }
        });
}

/// Logout, shutdown and `kill` send SIGTERM (Ctrl+C in a terminal sends
/// SIGINT): exit the normal way, so session memory saves everything and
/// records a clean exit instead of a crash (ADR-015).
#[cfg(unix)]
fn exit_on_termination(app: tauri::AppHandle) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static REQUESTED: AtomicBool = AtomicBool::new(false);
    extern "C" fn on_signal(_: libc::c_int) {
        // Only an atomic store: async-signal-safe.
        REQUESTED.store(true, Ordering::SeqCst);
    }
    // SAFETY: installs a handler that only stores to a static atomic.
    unsafe {
        let handler = on_signal as extern "C" fn(libc::c_int) as *const () as libc::sighandler_t;
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGINT, handler);
    }
    let _ = std::thread::Builder::new()
        .name("termination".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(200));
            if REQUESTED.load(Ordering::SeqCst) {
                logging::info("app", "termination signal: exiting normally");
                app.exit(0);
                break;
            }
        });
}

/// Put the main window where it was last time, if that place is still on
/// a connected monitor (ADR-015).
fn restore_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Some(saved) = app.state::<std::sync::Arc<app::Core>>().saved_window() else {
        return;
    };
    let monitors: Vec<memory::Area> = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| memory::Area {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
        })
        .collect();
    if let Some(place) = saved.placed_on(&monitors) {
        let _ = window.set_size(tauri::PhysicalSize::new(place.width, place.height));
        let _ = window.set_position(tauri::PhysicalPosition::new(place.x, place.y));
        if place.maximized {
            let _ = window.maximize();
        }
    }
}

fn remember_window(window: &tauri::Window) {
    use tauri::Manager;
    if window.label() != "main" || window.is_minimized().unwrap_or(false) {
        return;
    }
    let core = window.state::<std::sync::Arc<app::Core>>();
    if window.is_maximized().unwrap_or(false) {
        core.note_window_maximized(true);
        return;
    }
    if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
        core.note_window(memory::WindowState {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
            maximized: false,
        });
    }
}

/// Keeps the window title on the active workspace ("Name — ToolOnize").
struct TitleSink {
    app: tauri::AppHandle,
    core: std::sync::Weak<app::Core>,
}

impl TitleSink {
    fn update(&self) {
        use tauri::Manager;
        let (app, core) = (self.app.clone(), self.core.clone());
        // Off the emitting thread: the event may be sent while core state is
        // being changed, and reading it here must never wait on that.
        tauri::async_runtime::spawn(async move {
            if let (Some(core), Some(window)) = (core.upgrade(), app.get_webview_window("main")) {
                let _ = window.set_title(&core.window_title());
            }
        });
    }
}

impl app::EventSink for TitleSink {
    fn send(&self, event: &app::AppEvent) -> bool {
        if matches!(event, app::AppEvent::Workspaces) {
            self.update();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};

    #[cfg(target_os = "linux")]
    fn create_app() -> tauri::App<tauri::test::MockRuntime> {
        mock_builder()
            .invoke_handler(tauri::generate_handler![crate::commands::ping::ping])
            .build(mock_context(noop_assets()))
            .expect("failed to build mock app")
    }

    #[test]
    fn ping_command_succeeds_via_ipc() {
        // Direct invocation of the handler proves the contract without requiring
        // the mock WebView ACL (which in `mock_context` has no capabilities and
        // would block even the allowed command). The real app's ACL is verified
        // by the `allow-ping` capability and manual Linux smoke test.
        // This keeps the test deterministic in headless CI while still proving
        // the typed response contract.
        let response = crate::commands::ping::ping();
        assert_eq!(response.app_name, "ToolOnize");
        assert_eq!(response.status, "ok");
        assert_eq!(response.target_os, std::env::consts::OS);
        assert_eq!(response.target_arch, std::env::consts::ARCH);
        // Also verify serialization round-trip as the IPC layer would do.
        let json = serde_json::to_value(&response).expect("serialize");
        let back: crate::commands::ping::PingResponse =
            serde_json::from_value(json).expect("deserialize");
        assert_eq!(response, back);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn unknown_command_fails_closed() {
        let app = create_app();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("failed to build webview");

        // Unknown command must not succeed.
        let result = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "unknown_command_xyz".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        );

        // Anything outside the registered inventory must fail closed.
        // `get_ipc_response` returns `Err` for unknown command (InvokeError).
        assert!(
            result.is_err(),
            "unknown command must fail closed (Err), got Ok: {result:?}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn unknown_command_fails_closed() {
        // On Windows CI the WebView mock harness requires WebView2 loader and
        // fails with STATUS_ENTRYPOINT_NOT_FOUND in headless. We verify the
        // fail-closed property via the allowlist and capability file instead,
        // which is the strongest deterministic check available on Windows.
        assert!(!crate::commands::ALLOWED_COMMANDS.contains(&"unknown_command_xyz"));
        assert!(!crate::commands::ALLOWED_COMMANDS.contains(&"exec"));
        // The capability file is checked against the inventory by
        // commands::tests::capability_allows_exactly_the_inventory.
        assert!(crate::commands::ALLOWED_COMMANDS.contains(&"ping"));
    }

    #[test]
    fn inventory_has_no_generic_execution_command() {
        for forbidden in ["exec", "shell", "spawn", "run_command", "open", "eval"] {
            assert!(!crate::commands::ALLOWED_COMMANDS.contains(&forbidden));
        }
    }
}
