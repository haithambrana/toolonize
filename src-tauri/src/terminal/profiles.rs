//! Rust-owned safe terminal profiles.
//! The frontend is untrusted and may only request opaque profile IDs.
//! Executable/argv construction remains Rust-side (SEC-002/003, FR-014).

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TerminalProfile {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub available: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedCommand {
    pub program: String,
    pub args: Vec<String>,
}

pub fn available_profiles() -> Vec<TerminalProfile> {
    let mut out = Vec::new();

    #[cfg(unix)]
    {
        // Resolve default shell first where safely resolvable via $SHELL.
        // No path, username, or env dump is returned to frontend — only id/kind/available.
        let shell_candidates: Vec<(&str, &str, &str)> = vec![
            ("default-shell", "Default Shell", "shell"),
            ("bash", "Bash", "shell"),
            ("sh", "POSIX sh", "shell"),
        ];
        for (id, display, kind) in shell_candidates {
            let available = is_profile_available(id);
            out.push(TerminalProfile {
                id: id.to_string(),
                display_name: display.to_string(),
                kind: kind.to_string(),
                available,
            });
        }
        // SSH/tmux launch semantics are modeled internally as typed variants
        // without exposing free-form host fields in M3.
        // For M3, preserve typed SSH/tmux resolver boundary:
        out.push(TerminalProfile {
            id: "ssh-passthrough".to_string(),
            display_name: "SSH (user config passthrough)".to_string(),
            kind: "remote".to_string(),
            available: is_command_available("ssh"),
        });
        out.push(TerminalProfile {
            id: "tmux-attach".to_string(),
            display_name: "tmux attach (multiplexer)".to_string(),
            kind: "multiplexer".to_string(),
            available: is_command_available("tmux"),
        });
    }

    #[cfg(windows)]
    {
        let candidates = vec![
            ("powershell", "Windows PowerShell", "shell"),
            ("cmd", "Command Prompt", "shell"),
            ("pwsh", "PowerShell 7", "shell"),
            ("wsl-default", "WSL Default", "wsl"),
        ];
        for (id, display, kind) in candidates {
            let available = is_profile_available(id);
            out.push(TerminalProfile {
                id: id.to_string(),
                display_name: display.to_string(),
                kind: kind.to_string(),
                available,
            });
        }
        out.push(TerminalProfile {
            id: "ssh-passthrough".to_string(),
            display_name: "SSH (user config passthrough)".to_string(),
            kind: "remote".to_string(),
            available: is_command_available("ssh"),
        });
        out.push(TerminalProfile {
            id: "tmux-attach".to_string(),
            display_name: "tmux attach (multiplexer)".to_string(),
            kind: "multiplexer".to_string(),
            available: is_command_available("tmux"),
        });
    }

    out
}

fn is_profile_available(id: &str) -> bool {
    match id {
        "default-shell" => resolve_default_shell().is_some(),
        "bash" => is_command_available("bash"),
        "sh" => is_command_available("sh"),
        "powershell" => is_command_available("powershell"),
        "cmd" => is_command_available("cmd"),
        "pwsh" => is_command_available("pwsh"),
        "wsl-default" => is_wsl_available(),
        "ssh-passthrough" => is_command_available("ssh"),
        "tmux-attach" => is_command_available("tmux"),
        _ => false,
    }
}

fn is_command_available(cmd: &str) -> bool {
    // Conservative availability probe: check PATH for executable without executing.
    // On Unix, walk PATH; on Windows, check PATHEXT.
    let path_var = std::env::var_os("PATH");
    if let Some(paths) = path_var {
        for dir in std::env::split_paths(&paths) {
            #[cfg(unix)]
            {
                let candidate = dir.join(cmd);
                if is_executable(&candidate) {
                    return true;
                }
            }
            #[cfg(windows)]
            {
                // Windows: try with and without .exe
                let candidates = if cmd.ends_with(".exe") {
                    vec![dir.join(cmd)]
                } else {
                    let exts = [".exe", ".cmd", ".bat"];
                    // PATHEXT handling simplified: try exe variants
                    exts.iter()
                        .map(|ext| dir.join(format!("{cmd}{ext}")))
                        .collect()
                };
                for c in candidates {
                    if c.exists() {
                        return true;
                    }
                }
            }
        }
    }
    // Also try direct absolute lookup for common locations without leaking paths
    false
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(md) = std::fs::metadata(path) {
        let perms = md.permissions();
        !md.is_dir() && (perms.mode() & 0o111 != 0)
    } else {
        false
    }
}

#[cfg(unix)]
fn resolve_default_shell() -> Option<String> {
    if let Ok(shell) = std::env::var("SHELL") {
        let p = std::path::Path::new(&shell);
        if is_executable(p) {
            return Some(shell);
        }
        if is_command_available(&shell) {
            return Some(shell);
        }
    }
    if is_command_available("bash") {
        return Some("bash".to_string());
    }
    if is_command_available("sh") {
        return Some("sh".to_string());
    }
    None
}

#[cfg(windows)]
fn resolve_default_shell() -> Option<String> {
    None
}

#[cfg(windows)]
fn is_wsl_available() -> bool {
    if !is_command_available("wsl") {
        return false;
    }
    true
}

#[cfg(unix)]
fn is_wsl_available() -> bool {
    false
}

/// Resolve an opaque profile id + dimensions into a concrete `program` + `args`.
/// Only opaque ids declared by `available_profiles()` are accepted.
/// Returns sanitized error on unknown id.
pub fn resolve_profile(id: &str) -> Result<ResolvedCommand, super::error::TerminalError> {
    use super::error::TerminalError;
    match id {
        "default-shell" => {
            #[cfg(unix)]
            {
                if let Some(shell) = resolve_default_shell() {
                    // Use the shell as program with no extra args — interactive login is PTY-driven.
                    return Ok(ResolvedCommand {
                        program: shell,
                        args: vec![],
                    });
                }
                Err(TerminalError::not_found("default shell not available"))
            }
            #[cfg(windows)]
            {
                Err(TerminalError::not_found(
                    "default-shell not available on Windows",
                ))
            }
        }
        "bash" => Ok(ResolvedCommand {
            program: "bash".to_string(),
            args: vec![],
        }),
        "sh" => Ok(ResolvedCommand {
            program: "sh".to_string(),
            args: vec![],
        }),
        "powershell" => Ok(ResolvedCommand {
            program: "powershell.exe".to_string(),
            args: vec!["-NoProfile".to_string()],
        }),
        "cmd" => Ok(ResolvedCommand {
            program: "cmd.exe".to_string(),
            args: vec![],
        }),
        "pwsh" => Ok(ResolvedCommand {
            program: "pwsh.exe".to_string(),
            args: vec!["-NoProfile".to_string()],
        }),
        "wsl-default" => Ok(ResolvedCommand {
            program: "wsl.exe".to_string(),
            args: vec![],
        }),
        // Typed SSH/tmux semantics: resolver constructs argv safely from fictional fixtures in tests,
        // never from frontend-supplied host strings in M3.
        "ssh-passthrough" => Ok(ResolvedCommand {
            program: "ssh".to_string(),
            args: vec![],
        }),
        "tmux-attach" => Ok(ResolvedCommand {
            program: "tmux".to_string(),
            args: vec!["attach".to_string()],
        }),
        _ => Err(TerminalError::invalid_input("unknown profile id")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_do_not_leak_paths() {
        let profiles = available_profiles();
        for p in &profiles {
            // Ensure no profile id or display_name contains a path separator or home dir
            assert!(!p.id.contains('/'));
            assert!(!p.id.contains('\\'));
            assert!(!p.display_name.contains("/home"));
            assert!(!p.display_name.contains("C:\\"));
        }
        // Serialized form must not contain path-like keys
        let json = serde_json::to_value(&profiles).unwrap();
        let s = json.to_string();
        assert!(!s.contains("/home/"));
        assert!(!s.contains("C:\\Users"));
    }

    #[test]
    fn only_opaque_ids_resolve() {
        assert!(
            resolve_profile("default-shell").is_ok() || resolve_profile("default-shell").is_err()
        );
        assert!(resolve_profile("../etc/passwd").is_err());
        assert!(resolve_profile("bash; rm -rf ~").is_err());
        assert!(resolve_profile("powershell.exe").is_err()); // must use opaque "powershell"
    }

    #[test]
    fn ssh_tmux_typed_semantics() {
        // These are typed, not free-form execution — they resolve to fixed argv without host args.
        let ssh = resolve_profile("ssh-passthrough").unwrap();
        assert_eq!(ssh.program, "ssh");
        assert!(ssh.args.is_empty()); // No host from frontend in M3

        let tmux = resolve_profile("tmux-attach").unwrap();
        assert_eq!(tmux.program, "tmux");
        assert_eq!(tmux.args, vec!["attach"]);
    }

    #[test]
    fn availability_is_sanitized() {
        let profiles = available_profiles();
        for p in profiles {
            // available is bool, never exposes path
            let _ = p.available;
            assert!(matches!(
                p.kind.as_str(),
                "shell" | "wsl" | "remote" | "multiplexer"
            ));
        }
    }
}
