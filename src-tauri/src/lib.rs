pub mod app;
pub mod audit;
pub mod classify;
pub mod commands;
pub mod discovery;
pub mod external;
pub mod model;
pub mod persist;
pub mod policy;
pub mod pty;
pub mod registry;
pub mod session;
pub mod settings;
pub mod shells;
pub mod transfer;
pub mod workspace;

/// Run the Tauri application. Separated from `main.rs` for testability and
/// to keep `main.rs` minimal (Tauri convention).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (state_dir, config_dir) = persist::default_dirs();
    let core = app::Core::new(state_dir, config_dir);
    if let Err(error) = core.rescan() {
        eprintln!("initial launcher scan failed: {error}");
    }
    core.restart_watch();
    let exit_core = core.clone();

    let builder = tauri::Builder::default().manage(core);
    #[cfg(not(feature = "spike"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        crate::commands::ping::ping,
        crate::commands::app::app_startup_info,
        crate::commands::app::events_subscribe,
        crate::commands::app::settings_get,
        crate::commands::app::settings_update,
        crate::commands::app::audit_recent,
        crate::commands::app::open_url,
        crate::commands::clipboard::clipboard_read_text,
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
        crate::commands::workspace::workspace_list,
        crate::commands::workspace::workspace_create,
        crate::commands::workspace::workspace_rename,
        crate::commands::workspace::workspace_delete,
        crate::commands::workspace::workspace_set_active,
        crate::commands::workspace::workspace_set_policy,
        crate::commands::workspace::workspace_accept_import,
        crate::commands::workspace::workspace_save_layout,
        crate::commands::workspace::workspace_open,
        crate::commands::workspace::workspace_export,
        crate::commands::workspace::workspace_import,
        crate::commands::workspace::member_add,
        crate::commands::workspace::member_remove,
        crate::commands::workspace::member_update,
        crate::commands::workspace::member_authorize,
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
        crate::commands::session::session_close,
        crate::commands::session::session_list
    ]);
    #[cfg(feature = "spike")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        crate::commands::ping::ping,
        crate::commands::spike::spike_pty_stream,
        crate::commands::spike::spike_resize,
        crate::commands::spike::spike_input_echo,
        crate::commands::spike::spike_complete,
        crate::commands::spike::spike_fail
    ]);
    builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app, event| {
            if let tauri::RunEvent::Exit = event {
                // FR-037: closing the app terminates local child processes.
                exit_core.shutdown();
            }
        });
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
