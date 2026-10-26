//! IPC command surface (SEC-002, T-WEB-01).
//!
//! Every command takes typed, validated identifiers or values. None accepts a
//! program, argument list, or command line: execution is only reachable via
//! `session_start` / `member_launch`, which name a workspace member that the
//! execution policy then checks. `ALLOWED_COMMANDS` is the single inventory
//! that the build manifest, capability file, and tests are checked against.

pub mod app;
pub mod clipboard;
pub mod ping;
pub mod registry;
pub mod session;
#[cfg(feature = "spike")]
pub mod spike;
pub mod workspace;

pub use ping::ping;

use crate::app::Core;
use std::sync::Arc;

/// Every custom command the frontend may invoke.
pub const ALLOWED_COMMANDS: &[&str] = &[
    "ping",
    "app_startup_info",
    "events_subscribe",
    "settings_get",
    "settings_update",
    "audit_recent",
    "open_url",
    "clipboard_read_text",
    "scrollback_export",
    "registry_view",
    "registry_rescan",
    "registry_set_hidden",
    "registry_set_alias",
    "registry_approve",
    "registry_revoke",
    "registry_acknowledge",
    "custom_root_add",
    "custom_root_remove",
    "synthetic_add",
    "synthetic_remove",
    "ssh_hosts",
    "workspace_list",
    "workspace_create",
    "workspace_rename",
    "workspace_set_color",
    "workspace_delete",
    "workspace_set_active",
    "workspace_set_policy",
    "workspace_accept_import",
    "workspace_save_layout",
    "workspace_open",
    "workspace_export",
    "workspace_import",
    "member_add",
    "member_remove",
    "member_update",
    "member_authorize",
    "member_launch",
    "session_start",
    "session_write",
    "session_resize",
    "session_attach",
    "session_detach",
    "session_ack",
    "session_view",
    "session_stop",
    "session_restart",
    "session_reconnect",
    "session_cancel_reconnect",
    "session_close",
    "session_list",
];

type CoreState<'a> = tauri::State<'a, Arc<Core>>;

/// Run blocking core work off the main thread.
async fn blocking<T, F>(core: &CoreState<'_>, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(Arc<Core>) -> Result<T, String> + Send + 'static,
{
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || f(core))
        .await
        .map_err(|e| format!("worker failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCES: &[&str] = &[
        include_str!("app.rs"),
        include_str!("clipboard.rs"),
        include_str!("ping.rs"),
        include_str!("registry.rs"),
        include_str!("session.rs"),
        include_str!("workspace.rs"),
    ];

    fn command_names() -> Vec<String> {
        let mut names = Vec::new();
        for src in SOURCES {
            let mut lines = src.lines();
            while let Some(line) = lines.next() {
                if line.trim() == "#[tauri::command]" {
                    let sig = lines.next().unwrap_or_default();
                    let name = sig
                        .split("fn ")
                        .nth(1)
                        .and_then(|rest| rest.split(['(', '<']).next())
                        .unwrap_or_default();
                    names.push(name.to_string());
                }
            }
        }
        names
    }

    #[test]
    fn inventory_matches_registered_commands_t_sec_001() {
        let mut found = command_names();
        found.sort();
        let mut allowed: Vec<String> = ALLOWED_COMMANDS.iter().map(|s| s.to_string()).collect();
        allowed.sort();
        assert_eq!(found, allowed);
        let build = include_str!("../../build.rs");
        for cmd in ALLOWED_COMMANDS {
            assert!(
                build.contains(&format!("\"{cmd}\"")),
                "{cmd} missing from build.rs"
            );
        }
    }

    #[test]
    fn capability_allows_exactly_the_inventory_t_sec_001() {
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/default.json")).unwrap();
        let mut perms: Vec<String> = cap["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().to_string())
            .collect();
        perms.sort();
        let mut expected: Vec<String> = ALLOWED_COMMANDS
            .iter()
            .map(|c| format!("allow-{}", c.replace('_', "-")))
            .collect();
        expected.sort();
        assert_eq!(perms, expected, "no core:/plugin permissions, no extras");
        assert_eq!(cap["windows"], serde_json::json!(["main"]));
    }

    #[test]
    fn no_command_accepts_command_text_t_sec_006() {
        // Static inventory over parameter names of every command signature.
        let forbidden = [
            "program",
            "args",
            "argv",
            "command",
            "cmd",
            "exec",
            "shell",
            "script",
            "path_to_run",
        ];
        for src in SOURCES {
            let mut lines = src.lines().peekable();
            while let Some(line) = lines.next() {
                if line.trim() != "#[tauri::command]" {
                    continue;
                }
                let mut sig = String::new();
                for l in lines.by_ref() {
                    sig.push_str(l);
                    if l.contains('{') {
                        break;
                    }
                }
                let params = sig.split('(').nth(1).unwrap_or_default();
                for p in params.split(',') {
                    let name = p.split(':').next().unwrap_or_default().trim();
                    assert!(
                        !forbidden.contains(&name),
                        "command parameter `{name}` looks like command text: {sig}"
                    );
                }
            }
        }
    }
}
