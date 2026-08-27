use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PingResponse {
    pub app_name: String,
    pub app_version: String,
    pub target_os: String,
    pub target_arch: String,
    pub status: String,
}

/// Single M1 IPC command with semantic identity `app::ping`.
///
/// Tauri invoke identifier is `ping` (Rust function name). The semantic
/// namespace `app::ping` is documented and enforced by handler registration:
/// only this command is registered via `generate_handler![ping]`.
///
/// Returns only non-sensitive sanitized values:
/// - app_name: static product name
/// - app_version: crate version
/// - target_os: compile-time OS (e.g. "linux", "windows")
/// - target_arch: compile-time arch (e.g. "x86_64")
/// - status: protocol/status marker
///
/// No hostname, username, cwd, IP, env, machine-id, or credential is returned.
#[tauri::command]
pub fn ping() -> PingResponse {
    PingResponse {
        app_name: "ToolOnize".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        target_os: std::env::consts::OS.to_string(),
        target_arch: std::env::consts::ARCH.to_string(),
        status: "ok".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_returns_sanitized_contract() {
        let r = ping();
        assert_eq!(r.app_name, "ToolOnize");
        assert_eq!(r.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(r.target_os, std::env::consts::OS);
        assert_eq!(r.target_arch, std::env::consts::ARCH);
        assert_eq!(r.status, "ok");
    }

    #[test]
    fn ping_response_serializes_expected_keys_only() {
        let r = ping();
        let v = serde_json::to_value(&r).expect("serialize");
        let obj = v.as_object().expect("object");
        // Exact expected keys — no leakage of sensitive fields
        let mut keys: Vec<&String> = obj.keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "app_name",
                "app_version",
                "status",
                "target_arch",
                "target_os"
            ]
        );
        // Ensure no sensitive key appears
        for forbidden in [
            "hostname", "username", "home", "cwd", "ip", "env", "machine",
        ] {
            assert!(
                !obj.contains_key(forbidden),
                "forbidden key {forbidden} leaked"
            );
        }
    }

    #[test]
    fn ping_never_panics_and_is_deterministic() {
        let a = ping();
        let b = ping();
        assert_eq!(a, b);
    }
}
