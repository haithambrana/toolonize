//! External Application Launcher (FR-040/041) and the URL opener (SEC-007).
//!
//! Launches an authorized plan detached from ToolOnize, preserving the
//! target's native invocation semantics: discrete argv on Linux (no shell);
//! on Windows the stored Shell Link argument string is passed through
//! unmodified with `raw_arg` (no interpreter layer added). Launched windows
//! are not managed afterwards.

use crate::policy::LaunchPlan;
use std::process::{Command, Stdio};

pub fn launch_detached(plan: &LaunchPlan) -> Result<u32, String> {
    let mut command = Command::new(&plan.spec.program);
    command
        .args(&plan.spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = &plan.spec.cwd {
        command.current_dir(cwd);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own process group: not killed with ToolOnize, not tied to our tty.
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        if let Some(raw) = &plan.raw_args {
            command.raw_arg(raw);
        }
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", plan.title))?;
    let pid = child.id();
    // Reap the child when it exits so it never lingers as a zombie.
    std::thread::Builder::new()
        .name("external-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        })
        .map_err(|e| e.to_string())?;
    Ok(pid)
}

/// http/https only, single line, bounded (SEC-007, T-EXT-01).
pub fn validate_url(url: &str) -> Result<(), String> {
    if url.len() > 2048 || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("invalid URL".into());
    }
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .ok_or("only http and https links can be opened")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() || host.starts_with('-') || host.contains('@') {
        return Err("invalid URL host".into());
    }
    Ok(())
}

pub fn open_url(url: &str) -> Result<(), String> {
    validate_url(url)?;
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        let wide = |s: &str| {
            s.encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        };
        let verb = wide("open");
        let target = wide(url);
        // SAFETY: NUL-terminated wide strings that outlive the call.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if (result as isize) <= 32 {
            return Err("the system could not open the link".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let mut child = Command::new("xdg-open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("could not open link: {e}"))?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::LaunchMode;
    use crate::pty::SpawnSpec;

    #[test]
    fn url_allowlist_t_ext_01() {
        assert!(validate_url("https://example.com/docs").is_ok());
        assert!(validate_url("http://example.com").is_ok());
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ms-settings:",
            "https://",
            "https://example.com/a b",
            "https://user@example.com",
            "ssh://example.com",
            "https://-oProxyCommand",
        ] {
            assert!(validate_url(bad).is_err(), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn detached_launch_passes_literal_argv_t_sec_002a() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("argv.txt");
        // A probe that records exactly the argv it received.
        let plan = LaunchPlan {
            mode: LaunchMode::External,
            spec: SpawnSpec {
                program: "python3".into(),
                args: vec![
                    "-c".into(),
                    "import sys,json; open(sys.argv[1],'w').write(json.dumps(sys.argv[2:]))".into(),
                    out.to_string_lossy().into_owned(),
                    "foo; rm -rf ~".into(),
                    "$(whoami)".into(),
                    "a|b".into(),
                ],
                cwd: None,
                env: vec![],
            },
            raw_args: None,
            remote: false,
            detach_expected: false,
            title: "probe".into(),
            launcher_id: "probe".into(),
            fingerprint: "fp".into(),
            interpreter: false,
        };
        launch_detached(&plan).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !out.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        let got: Vec<String> =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(got, vec!["foo; rm -rf ~", "$(whoami)", "a|b"]);
    }

    #[test]
    fn launch_failure_reports_reason_fs_05() {
        let plan = LaunchPlan {
            mode: LaunchMode::External,
            spec: SpawnSpec {
                program: "/nonexistent/toolonize-missing".into(),
                args: vec![],
                cwd: None,
                env: vec![],
            },
            raw_args: None,
            remote: false,
            detach_expected: false,
            title: "Missing".into(),
            launcher_id: "x".into(),
            fingerprint: "fp".into(),
            interpreter: false,
        };
        let err = launch_detached(&plan).unwrap_err();
        assert!(err.contains("Missing"));
    }
}
