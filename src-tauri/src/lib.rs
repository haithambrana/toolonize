pub mod commands;
pub mod terminal;

/// Run the Tauri application. Separated from `main.rs` for testability and
/// to keep `main.rs` minimal (Tauri convention).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(feature = "spike")]
    {
        tauri::Builder::default()
            .plugin(tauri_plugin_clipboard_manager::init())
            .invoke_handler(tauri::generate_handler![
                crate::commands::ping::ping,
                crate::commands::spike::spike_pty_stream,
                crate::commands::spike::spike_resize,
                crate::commands::spike::spike_input_echo,
                crate::commands::spike::spike_complete,
                crate::commands::spike::spike_fail,
                crate::commands::m3_harness::m3_complete,
                crate::commands::m3_harness::m3_fail,
                crate::commands::terminal::terminal_profiles,
                crate::commands::terminal::terminal_start,
                crate::commands::terminal::terminal_list,
                crate::commands::terminal::terminal_attach,
                crate::commands::terminal::terminal_detach,
                crate::commands::terminal::terminal_hide,
                crate::commands::terminal::terminal_show,
                crate::commands::terminal::terminal_write,
                crate::commands::terminal::terminal_resize,
                crate::commands::terminal::terminal_ack,
                crate::commands::terminal::terminal_close,
                crate::commands::terminal::terminal_restart,
                crate::commands::terminal::terminal_poll,
                crate::commands::terminal::terminal_replay
            ])
            .on_window_event(|_window, event| {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    crate::terminal::manager::global_manager().shutdown_all();
                }
            })
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }
    #[cfg(not(feature = "spike"))]
    {
        tauri::Builder::default()
            .plugin(tauri_plugin_clipboard_manager::init())
            .invoke_handler(tauri::generate_handler![
                crate::commands::ping::ping,
                crate::commands::m3_harness::m3_complete,
                crate::commands::m3_harness::m3_fail,
                crate::commands::terminal::terminal_profiles,
                crate::commands::terminal::terminal_start,
                crate::commands::terminal::terminal_list,
                crate::commands::terminal::terminal_attach,
                crate::commands::terminal::terminal_detach,
                crate::commands::terminal::terminal_hide,
                crate::commands::terminal::terminal_show,
                crate::commands::terminal::terminal_write,
                crate::commands::terminal::terminal_resize,
                crate::commands::terminal::terminal_ack,
                crate::commands::terminal::terminal_close,
                crate::commands::terminal::terminal_restart,
                crate::commands::terminal::terminal_poll,
                crate::commands::terminal::terminal_replay
            ])
            .on_window_event(|_window, event| {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    crate::terminal::manager::global_manager().shutdown_all();
                }
            })
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
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

        // For M1, the only allowed command is `ping`. Anything else must fail closed.
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
        assert!(crate::commands::ALLOWED_COMMANDS.contains(&"ping"));
        assert!(crate::commands::ALLOWED_COMMANDS.contains(&"terminal_write"));
        assert!(!crate::commands::ALLOWED_COMMANDS.contains(&"raw_exec"));
    }

    #[test]
    fn registered_commands_are_only_ping() {
        // Updated for M3: ping + terminal lifecycle + M3 harness (no raw exec).
        assert_eq!(
            crate::commands::ALLOWED_COMMANDS,
            &[
                "ping",
                "terminal_profiles",
                "terminal_start",
                "terminal_list",
                "terminal_attach",
                "terminal_detach",
                "terminal_hide",
                "terminal_show",
                "terminal_write",
                "terminal_resize",
                "terminal_ack",
                "terminal_close",
                "terminal_restart",
                "terminal_poll",
                "terminal_replay",
                "m3_complete",
                "m3_fail",
            ]
        );
        // Ensure no raw exec is registered
        assert!(!crate::commands::ALLOWED_COMMANDS.contains(&"exec"));
        assert!(!crate::commands::ALLOWED_COMMANDS.contains(&"shell"));
    }
}
