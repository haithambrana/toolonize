//! Security / Execution Policy (SEC-001, SEC-003, FR-013, FR-015).
//!
//! The only path from "a member exists" to "a process starts". Plans are
//! built in Rust from reviewed descriptors; the frontend can only name a
//! workspace member. There is no API that accepts command text.

use crate::classify::program_name;
use crate::model::{DescriptorKind, LauncherType, ParseStatus};
use crate::pty::SpawnSpec;
use crate::registry::{Registry, RegistryState};
use crate::workspace::{Member, MemberType, Workspace};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchMode {
    Pty,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub mode: LaunchMode,
    pub spec: SpawnSpec,
    /// Windows Shell Link argument string for external launches, passed
    /// through unmodified (no interpreter layer added).
    pub raw_args: Option<String>,
    pub remote: bool,
    pub detach_expected: bool,
    pub title: String,
    pub launcher_id: String,
    pub fingerprint: String,
    pub interpreter: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberState {
    Ready,
    /// Descriptor changed since authorization: re-review required.
    Changed,
    /// Never authorized for this workspace (e.g. imported).
    NotAuthorized,
    /// The launcher no longer exists.
    Orphaned,
    /// The launcher cannot run right now (needs review / unknown).
    Blocked,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemberStatus {
    pub state: MemberState,
    pub reason: Option<String>,
    pub launcher_name: Option<String>,
    pub current_fingerprint: Option<String>,
}

pub fn member_status(member: &Member, registry: &Registry, state: &RegistryState) -> MemberStatus {
    let Some(entry) = registry.effective(&member.launcher_id, state) else {
        return MemberStatus {
            state: MemberState::Orphaned,
            reason: Some("the launcher was removed or is not present on this machine".into()),
            launcher_name: None,
            current_fingerprint: None,
        };
    };
    let name = entry
        .decision
        .alias
        .clone()
        .unwrap_or_else(|| entry.launcher.display_name.clone());
    let current = entry.launcher.fingerprint.clone();
    let (state, reason) = if entry.launcher.parse_status == ParseStatus::Unavailable {
        (
            MemberState::Unavailable,
            Some(
                entry
                    .launcher
                    .issues
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "target unavailable".into()),
            ),
        )
    } else if member.authorized_fingerprint.is_none() {
        (
            MemberState::NotAuthorized,
            Some("review and authorize this member before it can start".into()),
        )
    } else if member.authorized_fingerprint.as_deref() != Some(current.as_str()) {
        (
            MemberState::Changed,
            Some(
                "the launcher's command changed since you authorized it: re-review required".into(),
            ),
        )
    } else if entry.effective_type == LauncherType::Unknown {
        (
            MemberState::Blocked,
            Some(if entry.review_pending {
                "the launcher needs review in the Launchers view".into()
            } else {
                "ToolOnize cannot run this launcher".into()
            }),
        )
    } else if MemberType::from_launcher(entry.effective_type) != Some(member.member_type) {
        (
            MemberState::Changed,
            Some("the launcher's kind changed: re-review required".into()),
        )
    } else {
        (MemberState::Ready, None)
    };
    MemberStatus {
        state,
        reason,
        launcher_name: Some(name),
        current_fingerprint: Some(current),
    }
}

/// Split a Windows command-line argument string with the
/// `CommandLineToArgvW` rules (used only to hand Shell Link arguments to the
/// PTY layer, which requires argv; external launches pass the string raw).
pub fn split_windows_args(raw: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut have_arg = false;
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' if !in_quotes => {
                if have_arg {
                    args.push(std::mem::take(&mut current));
                    have_arg = false;
                }
                i += 1;
            }
            '\\' => {
                let start = i;
                while i < chars.len() && chars[i] == '\\' {
                    i += 1;
                }
                let count = i - start;
                if i < chars.len() && chars[i] == '"' {
                    current.push_str(&"\\".repeat(count / 2));
                    if count % 2 == 1 {
                        current.push('"');
                        i += 1;
                    }
                } else {
                    current.push_str(&"\\".repeat(count));
                }
                have_arg = true;
            }
            '"' => {
                if in_quotes && chars.get(i + 1) == Some(&'"') {
                    current.push('"');
                    i += 2;
                } else {
                    in_quotes = !in_quotes;
                    i += 1;
                }
                have_arg = true;
            }
            other => {
                current.push(other);
                have_arg = true;
                i += 1;
            }
        }
    }
    if have_arg {
        args.push(current);
    }
    args
}

/// Build a launch plan for a member, or explain why it may not start.
pub fn plan_start(
    workspace: &Workspace,
    member_id: &str,
    registry: &Registry,
    state: &RegistryState,
) -> Result<LaunchPlan, String> {
    let member = workspace
        .member(member_id)
        .ok_or("this member is not part of the workspace")?;
    let status = member_status(member, registry, state);
    if status.state != MemberState::Ready {
        return Err(status
            .reason
            .unwrap_or_else(|| "member cannot start".into()));
    }
    let entry = registry
        .effective(&member.launcher_id, state)
        .ok_or("launcher not found")?;
    let launcher = &entry.launcher;
    let d = &launcher.descriptor;
    let program = d
        .program
        .clone()
        .ok_or("the launcher has no executable target")?;
    let cwd = match &d.working_dir {
        None => None,
        Some(dir) => {
            let path = PathBuf::from(dir);
            if !path.is_absolute() || !path.is_dir() {
                return Err(format!("working directory does not exist: {dir}"));
            }
            Some(path)
        }
    };
    let mode = if member.member_type.runs_in_pty() {
        LaunchMode::Pty
    } else {
        LaunchMode::External
    };
    let (args, raw_args) = match (d.kind, mode) {
        (DescriptorKind::ShellLink, LaunchMode::Pty) => (
            d.raw_args
                .as_deref()
                .map(split_windows_args)
                .unwrap_or_default(),
            None,
        ),
        (DescriptorKind::ShellLink, LaunchMode::External) => (vec![], d.raw_args.clone()),
        _ => (d.args.clone(), None),
    };
    let name = program_name(&program);
    let title = member
        .title
        .clone()
        .or(entry.decision.alias.clone())
        .unwrap_or_else(|| launcher.display_name.clone());
    Ok(LaunchPlan {
        mode,
        spec: SpawnSpec {
            program,
            args,
            cwd,
            env: vec![],
        },
        raw_args,
        remote: member.member_type == MemberType::RemoteCommand,
        detach_expected: member.detach_expected
            || matches!(name.as_str(), "tmux" | "screen" | "zellij" | "byobu")
            || d.args.iter().any(|a| a == "tmux"),
        title,
        launcher_id: launcher.launcher_id.clone(),
        fingerprint: launcher.fingerprint.clone(),
        interpreter: launcher.classification.interpreter,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Descriptor, OriginKind, Platform, RawLauncher};
    use crate::registry::approve;

    fn raw(
        id: &str,
        origin: OriginKind,
        program: &str,
        args: &[&str],
        terminal: bool,
    ) -> RawLauncher {
        RawLauncher {
            launcher_id: id.into(),
            display_name: id.into(),
            platform: Platform::Linux,
            origin,
            origin_root: "/fixture".into(),
            relative_path: format!("{id}.desktop"),
            descriptor: Descriptor {
                kind: DescriptorKind::ExecTemplate,
                program: Some(program.into()),
                args: args.iter().map(|s| s.to_string()).collect(),
                raw_args: None,
                exec_template: None,
                working_dir: None,
                terminal,
                try_exec: None,
                icon: None,
                path_resolved: false,
                source_path: format!("/fixture/{id}.desktop"),
            },
            parse_status: ParseStatus::Ok,
            issues: vec![],
            no_display: false,
            not_shown_here: false,
            comment: None,
        }
    }

    fn member(id: &str, launcher: &str, t: MemberType, fp: Option<String>) -> Member {
        Member {
            id: id.into(),
            launcher_id: launcher.into(),
            member_type: t,
            title: None,
            authorized_fingerprint: fp,
            authorized_at: None,
            auto_restart: false,
            detach_expected: false,
            added_at: 0,
            portable: None,
            color: None,
        }
    }

    fn setup() -> (Registry, RegistryState, Workspace) {
        let mut reg = Registry::default();
        let mut state = RegistryState::default();
        reg.apply_scan(
            vec![
                raw("tool", OriginKind::XdgSystem, "/usr/bin/htop", &[], true),
                raw(
                    "desk",
                    OriginKind::DesktopDir,
                    "/usr/bin/htop",
                    &["-d", "5"],
                    true,
                ),
                raw(
                    "gui",
                    OriginKind::XdgSystem,
                    "/opt/example/gui",
                    &["--x"],
                    false,
                ),
            ],
            vec![],
            &mut state,
        );
        let ws = Workspace::new("w".into(), "W".into());
        (reg, state, ws)
    }

    #[test]
    fn unpinned_or_unknown_member_denied_t_policy_001() {
        let (reg, state, ws) = setup();
        assert!(plan_start(&ws, "nope", &reg, &state).is_err());
        let mut ws = ws;
        ws.members
            .push(member("m", "tool", MemberType::LocalCommand, None));
        let err = plan_start(&ws, "m", &reg, &state).unwrap_err();
        assert!(err.contains("authorize"), "{err}");
    }

    #[test]
    fn authorized_member_builds_discrete_argv() {
        let (reg, state, mut ws) = setup();
        let fp = reg.get("tool").unwrap().fingerprint.clone();
        ws.members
            .push(member("m", "tool", MemberType::LocalCommand, Some(fp)));
        let plan = plan_start(&ws, "m", &reg, &state).unwrap();
        assert_eq!(plan.mode, LaunchMode::Pty);
        assert_eq!(plan.spec.program, "/usr/bin/htop");
        let gui_fp = reg.get("gui").unwrap().fingerprint.clone();
        ws.members
            .push(member("g", "gui", MemberType::ExternalApp, Some(gui_fp)));
        let plan = plan_start(&ws, "g", &reg, &state).unwrap();
        assert_eq!(plan.mode, LaunchMode::External);
        assert_eq!(plan.spec.args, vec!["--x"]);
    }

    #[test]
    fn descriptor_change_denies_until_reauthorized_t_policy_003_t_sec_005() {
        let (mut reg, mut state, mut ws) = setup();
        let fp = reg.get("tool").unwrap().fingerprint.clone();
        ws.members
            .push(member("m", "tool", MemberType::LocalCommand, Some(fp)));
        assert!(plan_start(&ws, "m", &reg, &state).is_ok());
        // Target swapped after pinning.
        reg.apply_scan(
            vec![raw("tool", OriginKind::XdgSystem, "/tmp/evil", &[], true)],
            vec![],
            &mut state,
        );
        let status = member_status(ws.member("m").unwrap(), &reg, &state);
        assert_eq!(status.state, MemberState::Changed);
        assert!(plan_start(&ws, "m", &reg, &state).is_err());
        assert_eq!(ws.members.len(), 1, "membership preserved");
        // Explicit re-authorization of the new fingerprint.
        let new_fp = reg.get("tool").unwrap().fingerprint.clone();
        ws.member_mut("m").unwrap().authorized_fingerprint = Some(new_fp);
        assert!(plan_start(&ws, "m", &reg, &state).is_ok());
    }

    #[test]
    fn review_required_launcher_blocked_until_approved_t_sec_004() {
        let (reg, mut state, mut ws) = setup();
        let fp = reg.get("desk").unwrap().fingerprint.clone();
        ws.members.push(member(
            "d",
            "desk",
            MemberType::LocalCommand,
            Some(fp.clone()),
        ));
        assert_eq!(
            member_status(ws.member("d").unwrap(), &reg, &state).state,
            MemberState::Blocked
        );
        assert!(plan_start(&ws, "d", &reg, &state).is_err());
        approve(&reg, &mut state, "desk", &fp).unwrap();
        let plan = plan_start(&ws, "d", &reg, &state).unwrap();
        assert_eq!(plan.spec.args, vec!["-d", "5"]);
    }

    #[test]
    fn orphaned_member_kept_but_not_startable() {
        let (mut reg, mut state, mut ws) = setup();
        let fp = reg.get("gui").unwrap().fingerprint.clone();
        ws.members
            .push(member("g", "gui", MemberType::ExternalApp, Some(fp)));
        reg.apply_scan(vec![], vec![], &mut state);
        assert_eq!(
            member_status(ws.member("g").unwrap(), &reg, &state).state,
            MemberState::Orphaned
        );
        assert!(plan_start(&ws, "g", &reg, &state).is_err());
    }

    #[test]
    fn metacharacters_remain_single_literal_arguments_t_sec_002a() {
        let (mut reg, mut state, mut ws) = setup();
        reg.apply_scan(
            vec![raw(
                "inj",
                OriginKind::XdgSystem,
                "/opt/example/probe",
                &["foo; rm -rf ~", "$(whoami)", "`id`"],
                false,
            )],
            vec![],
            &mut state,
        );
        let fp = reg.get("inj").unwrap().fingerprint.clone();
        ws.members
            .push(member("i", "inj", MemberType::ExternalApp, Some(fp)));
        let plan = plan_start(&ws, "i", &reg, &state).unwrap();
        assert_eq!(plan.spec.program, "/opt/example/probe");
        assert_eq!(plan.spec.args, vec!["foo; rm -rf ~", "$(whoami)", "`id`"]);
    }

    #[test]
    fn windows_argument_splitting() {
        assert_eq!(split_windows_args(r#"a "b c" d"#), vec!["a", "b c", "d"]);
        assert_eq!(split_windows_args(r#""a\"b""#), vec![r#"a"b"#]);
        assert_eq!(split_windows_args(r#"a\\\"b"#), vec![r#"a\"b"#]);
        assert_eq!(split_windows_args(r#"a\\"b c""#), vec![r#"a\b c"#]);
        assert_eq!(
            split_windows_args(r#"C:\path\to\ "x""#),
            vec![r#"C:\path\to\"#, "x"]
        );
        assert_eq!(split_windows_args(r#""""#), vec![""]);
        assert_eq!(split_windows_args("  "), Vec::<String>::new());
    }
}
