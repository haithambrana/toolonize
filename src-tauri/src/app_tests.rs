//! Core integration tests: registry -> workspace -> policy -> sessions ->
//! persistence, against real temp dirs and (on Unix) the real PTY.

use super::*;
use serde_json::json;

fn core(dir: &tempfile::TempDir) -> Arc<Core> {
    let core = Core::new(dir.path().join("state"), dir.path().join("config"));
    core.rescan().unwrap();
    core
}

#[cfg(unix)]
fn write_entry(root: &std::path::Path, name: &str, exec: &str) {
    std::fs::write(
        root.join(name),
        format!(
            "[Desktop Entry]\nType=Application\nName=Fixture {name}\nExec={exec}\nTerminal=true\n"
        ),
    )
    .unwrap();
}

#[cfg(unix)]
fn fingerprint(core: &Core, id: &str) -> String {
    core.registry_entry(id).unwrap().launcher.fingerprint
}

#[cfg(unix)]
fn shell_id(core: &Core) -> String {
    core.registry_view()
        .entries
        .into_iter()
        .find(|e| e.launcher.launcher_id == "sys:shell:sh")
        .map(|e| e.launcher.launcher_id)
        .expect("sh is available on CI")
}

#[test]
fn workspace_crud_persists_across_restart_t_persist_001() {
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let c = core(&dir);
        let id = c.create_workspace("Project Phoenix").unwrap();
        c.set_policy(&id, StartupPolicy::RestoreTerminals).unwrap();
        c.save_layout(
            &id,
            LayoutMode::Tabs,
            Some(json!({"layout": {"type": "row", "children": []}})),
            None,
        )
        .unwrap();
        id
    };
    let c = core(&dir);
    let (list, active) = c.workspaces();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Project Phoenix");
    assert_eq!(list[0].startup_policy, StartupPolicy::RestoreTerminals);
    assert_eq!(list[0].layout.mode, LayoutMode::Tabs);
    assert_eq!(active.as_deref(), Some(id.as_str()));
    c.rename_workspace(&id, "Renamed").unwrap();
    c.delete_workspace(&id).unwrap();
    assert!(core(&dir).workspaces().0.is_empty());
}

#[test]
fn invalid_layout_rejected_and_state_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let id = c.create_workspace("W").unwrap();
    let bad = json!({"layout": {"type": "row", "children": [
        {"type": "tabset", "children": [{"type": "tab", "component": "member", "config": {"memberId": "ghost"}}]}
    ]}});
    assert!(c
        .save_layout(&id, LayoutMode::Grid, Some(bad), None)
        .is_err());
    assert!(c.workspaces().0[0].layout.json.is_none());
}

/// Uses Linux `.desktop` custom roots; the Windows equivalent (`.lnk`) is
/// covered by discovery::windows and policy tests.
#[cfg(unix)]
#[test]
fn custom_root_review_pin_change_reauthorize_flow() {
    let dir = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write_entry(
        root.path(),
        "watch.desktop",
        "sh -c \"echo first; sleep 30\"",
    );
    let c = core(&dir);
    c.add_custom_root(&root.path().to_string_lossy()).unwrap();
    let launcher = c
        .registry_view()
        .entries
        .into_iter()
        .find(|e| e.launcher.origin == crate::model::OriginKind::CustomRoot)
        .unwrap();
    let id = launcher.launcher.launcher_id.clone();
    assert!(
        launcher.review_pending,
        "custom roots need review (T-SEC-004)"
    );
    assert!(launcher.launcher.classification.interpreter);
    let ws = c.create_workspace("W").unwrap();
    let fp = fingerprint(&c, &id);
    assert!(
        c.add_member(&ws, &id, &fp, None).is_err(),
        "unreviewed launcher cannot be pinned"
    );
    c.approve(&id, &fp).unwrap();
    let member = c.add_member(&ws, &id, &fp, None).unwrap();
    let status = &c.workspaces().0[0].members[0].status;
    assert_eq!(status.state, policy::MemberState::Ready);

    // Descriptor tampering after pinning (T-SEC-005).
    write_entry(
        root.path(),
        "watch.desktop",
        "sh -c \"echo tampered; sleep 30\"",
    );
    c.rescan().unwrap();
    let view = &c.workspaces().0[0].members[0];
    assert_eq!(view.status.state, policy::MemberState::Changed);
    assert_eq!(
        view.member.launcher_id, id,
        "identity stable, membership kept"
    );
    let err = c.start_session(&ws, &member, 24, 80).unwrap_err();
    assert!(err.contains("re-review"), "{err}");
    assert!(c
        .audit_recent(5)
        .iter()
        .any(|e| e.action == "session.start" && e.outcome == Outcome::Denied));

    // Explicit re-review + re-authorization.
    let fp2 = fingerprint(&c, &id);
    assert_ne!(fp, fp2);
    assert!(
        c.authorize_member(&ws, &member, &fp2).is_err(),
        "registry review comes first"
    );
    c.approve(&id, &fp2).unwrap();
    c.authorize_member(&ws, &member, &fp2).unwrap();
    assert_eq!(
        c.workspaces().0[0].members[0].status.state,
        policy::MemberState::Ready
    );
}

#[test]
fn unauthorized_member_cannot_start_t_policy_001() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let ws = c.create_workspace("W").unwrap();
    assert!(c.start_session(&ws, "m_missing", 24, 80).is_err());
    assert!(c.launch_external(&ws, "m_missing").is_err());
    let denied = c.audit_recent(10);
    assert!(denied.iter().all(|e| e.outcome == Outcome::Denied));
}

#[test]
fn export_import_round_trip_executes_nothing_fs_06() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let ws = c.create_workspace("Exported").unwrap();
    let ssh = c
        .add_synthetic(SyntheticSpec::Ssh {
            host: "build.example.com".into(),
        })
        .unwrap();
    if let Some(entry) = c.registry_entry(&ssh) {
        if entry.executable {
            c.add_member(&ws, &ssh, &entry.launcher.fingerprint, None)
                .unwrap();
        }
    }
    let text = c.export_workspace(&ws, false).unwrap();
    let report = c.import_workspace(&text).unwrap();
    let (list, _) = c.workspaces();
    let imported = list.iter().find(|w| w.id == report.workspace_id).unwrap();
    assert!(imported.staged_import);
    assert_eq!(imported.startup_policy, StartupPolicy::Ask);
    assert!(imported
        .members
        .iter()
        .all(|m| m.status.state != policy::MemberState::Ready));
    let opened = c.open_workspace(&imported.id, None).unwrap();
    assert!(
        c.sessions.list().is_empty(),
        "import + open started nothing"
    );
    assert!(opened.outcomes.iter().all(|o| o.outcome != "started"));
    assert!(c.import_workspace("{\"format\": \"evil\"}").is_err());
}

#[cfg(unix)]
#[test]
fn terminal_member_lifecycle_and_startup_policy() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let ws = c.create_workspace("Shells").unwrap();
    let shell = shell_id(&c);
    let fp = fingerprint(&c, &shell);
    let member = c
        .add_member(&ws, &shell, &fp, Some("Main shell".into()))
        .unwrap();
    // Ask policy: a choice is required before anything starts.
    let ask = c.open_workspace(&ws, None).unwrap();
    assert!(ask.needs_choice);
    assert!(c.sessions.list().is_empty());
    let opened = c
        .open_workspace(
            &ws,
            Some(OpenChoice {
                start_terminals: true,
                launch_external: false,
            }),
        )
        .unwrap();
    assert_eq!(opened.outcomes[0].outcome, "started");
    let info = c.sessions.find_by_member(&ws, &member).unwrap();
    assert_eq!(info.title, "Main shell");
    // Opening again does not respawn.
    let again = c
        .open_workspace(
            &ws,
            Some(OpenChoice {
                start_terminals: true,
                launch_external: false,
            }),
        )
        .unwrap();
    assert_eq!(again.outcomes[0].outcome, "running");
    // A renderer reload re-opens without a choice: the policy does not run again.
    let reload = c.open_workspace(&ws, None).unwrap();
    assert!(!reload.needs_choice && reload.outcomes.is_empty());
    // Renaming a member renames its running session; process untouched.
    c.update_member(
        &ws,
        &member,
        MemberPatch {
            title: Some("Build shell".into()),
            auto_restart: None,
            detach_expected: None,
            color: None,
            start_dir: None,
            zoom: None,
        },
    )
    .unwrap();
    let renamed = c.sessions.info(&info.session_id).unwrap();
    assert_eq!(renamed.title, "Build shell");
    assert_eq!(renamed.generation, info.generation);
    assert_eq!(
        c.workspaces().0[0].members[0].member.title.as_deref(),
        Some("Build shell")
    );
    // An empty title resets to the launcher's name.
    c.update_member(
        &ws,
        &member,
        MemberPatch {
            title: Some("  ".into()),
            auto_restart: None,
            detach_expected: None,
            color: None,
            start_dir: None,
            zoom: None,
        },
    )
    .unwrap();
    assert_eq!(c.workspaces().0[0].members[0].member.title, None);
    assert_ne!(
        c.sessions.info(&info.session_id).unwrap().title,
        "Build shell"
    );
    let restarted = c.restart_session(&info.session_id).unwrap();
    assert_eq!(restarted.generation, 2);
    c.remove_member(&ws, &member).unwrap();
    assert!(
        c.sessions.list().is_empty(),
        "removing a member closes its session"
    );
    c.shutdown();
}

#[test]
fn settings_validated_and_persisted() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let mut s = c.settings();
    s.font_size = 99;
    assert!(c.update_settings(s.clone()).is_err());
    s.font_size = 16;
    s.live_updates = false;
    c.update_settings(s).unwrap();
    assert_eq!(core(&dir).settings().font_size, 16);
}

#[test]
fn color_tags_validate_and_persist() {
    let dir = tempfile::tempdir().unwrap();
    let ws = {
        let c = core(&dir);
        let ws = c.create_workspace("W").unwrap();
        c.set_workspace_color(&ws, Some("teal")).unwrap();
        assert!(c.set_workspace_color(&ws, Some("#123456")).is_err());
        ws
    };
    let c = core(&dir);
    assert_eq!(c.workspaces().0[0].color.as_deref(), Some("teal"));
    c.set_workspace_color(&ws, None).unwrap();
    assert_eq!(c.workspaces().0[0].color, None);
}

#[cfg(unix)]
#[test]
fn member_color_patch() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let ws = c.create_workspace("W").unwrap();
    let shell = shell_id(&c);
    let fp = fingerprint(&c, &shell);
    let member = c.add_member(&ws, &shell, &fp, None).unwrap();
    let patch = |color: &str| MemberPatch {
        title: None,
        auto_restart: None,
        detach_expected: None,
        color: Some(color.into()),
        start_dir: None,
        zoom: None,
    };
    c.update_member(&ws, &member, patch("purple")).unwrap();
    assert_eq!(
        c.workspaces().0[0].members[0].member.color.as_deref(),
        Some("purple")
    );
    assert!(c.update_member(&ws, &member, patch("url(x)")).is_err());
    c.update_member(&ws, &member, patch("")).unwrap();
    assert_eq!(c.workspaces().0[0].members[0].member.color, None);
}

#[derive(Default)]
struct Events(Mutex<Vec<AppEvent>>);

impl EventSink for Events {
    fn send(&self, event: &AppEvent) -> bool {
        self.0.lock().unwrap().push(event.clone());
        true
    }
}

impl Events {
    fn reconnects(&self) -> Vec<(bool, u32, u64)> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                AppEvent::Reconnect {
                    active,
                    attempt,
                    delay_ms,
                    ..
                } => Some((*active, *attempt, *delay_ms)),
                _ => None,
            })
            .collect()
    }
}

fn dropped(session_id: &str, code: u32, remote: bool) -> SessionInfo {
    let process_state = if remote && code == 255 {
        crate::session::ProcessState::Disconnected { code }
    } else {
        crate::session::ProcessState::Exited { code, signal: None }
    };
    SessionInfo {
        session_id: session_id.into(),
        workspace_id: "ws".into(),
        member_id: "m".into(),
        title: "Build host".into(),
        process_state,
        view_state: crate::session::ViewState::Detached,
        generation: 1,
        pid: None,
        remote,
        detach_expected: false,
        rows: 24,
        cols: 80,
        base_offset: 0,
        end_offset: 0,
        attached: false,
        backend: "test".into(),
        started_at_ms: now_ms(),
    }
}

#[test]
fn ssh_reconnect_backoff_rules() {
    assert_eq!(reconnect_delay(1), Duration::from_secs(2));
    assert_eq!(reconnect_delay(2), Duration::from_secs(4));
    assert_eq!(reconnect_delay(4), Duration::from_secs(16));
    assert_eq!(reconnect_delay(5), Duration::from_secs(30));
    assert_eq!(reconnect_delay(8), Duration::from_secs(30));

    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let events = Arc::new(Events::default());
    c.subscribe(events.clone());
    // Only remote sessions that exit with ssh's connection-failure status.
    assert!(!c.maybe_reconnect(&dropped("s_a", 0, true)));
    assert!(!c.maybe_reconnect(&dropped("s_a", 255, false)));
    assert!(events.reconnects().is_empty());
    // Backoff grows per consecutive drop, then gives up.
    for attempt in 1..=RECONNECT_MAX_ATTEMPTS {
        assert!(c.maybe_reconnect(&dropped("s_a", 255, true)));
        let last = *events.reconnects().last().unwrap();
        assert_eq!(
            last,
            (true, attempt, reconnect_delay(attempt).as_millis() as u64)
        );
    }
    assert!(c.maybe_reconnect(&dropped("s_a", 255, true)));
    assert!(!events.reconnects().last().unwrap().0, "gave up");
    // A stable connection restarts the backoff.
    let mut stable = dropped("s_b", 255, true);
    stable.started_at_ms = now_ms() - 2 * RECONNECT_STABLE_MS;
    assert!(c.maybe_reconnect(&dropped("s_b", 255, true)));
    assert!(c.maybe_reconnect(&dropped("s_b", 255, true)));
    assert!(c.maybe_reconnect(&stable));
    assert_eq!(events.reconnects().last().unwrap().1, 1);
    // Cancel stops it.
    assert!(c.maybe_reconnect(&dropped("s_c", 255, true)));
    c.cancel_reconnect("s_c").unwrap();
    assert!(!c.reconnects.lock().unwrap().contains_key("s_c"));
    // The setting turns it off.
    let mut settings = c.settings();
    settings.ssh_auto_reconnect = false;
    c.update_settings(settings).unwrap();
    assert!(!c.maybe_reconnect(&dropped("s_d", 255, true)));
}

/// End to end through the real PTY: a fake `ssh` that fails like a dropped
/// connection (exit 255) is restarted automatically after the first backoff.
#[cfg(unix)]
#[test]
fn ssh_member_reconnects_after_a_drop() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let ssh = bin.path().join("ssh");
    std::fs::write(&ssh, "#!/bin/sh\necho fake ssh to $1\nexit 255\n").unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    write_entry(
        root.path(),
        "build.desktop",
        &format!("{} build.example.com", ssh.display()),
    );
    let c = core(&dir);
    c.add_custom_root(&root.path().to_string_lossy()).unwrap();
    let launcher = c
        .registry_view()
        .entries
        .into_iter()
        .find(|e| e.launcher.origin == crate::model::OriginKind::CustomRoot)
        .unwrap();
    let id = launcher.launcher.launcher_id.clone();
    let fp = fingerprint(&c, &id);
    c.approve(&id, &fp).unwrap();
    assert_eq!(
        c.registry_entry(&id).unwrap().effective_type,
        LauncherType::RemoteSsh
    );
    let ws = c.create_workspace("Remote").unwrap();
    let member = c.add_member(&ws, &id, &fp, None).unwrap();
    let events = Arc::new(Events::default());
    c.subscribe(events.clone());
    let info = c.start_session(&ws, &member, 24, 80).unwrap();
    assert!(info.remote);
    let deadline = Instant::now() + Duration::from_secs(15);
    let generation = loop {
        let g = c.sessions.info(&info.session_id).unwrap().generation;
        if g > info.generation || Instant::now() > deadline {
            break g;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(generation > info.generation, "reconnected after the drop");
    assert!(events.reconnects().iter().any(|r| r.0 && r.1 == 1));
    c.close_session(&info.session_id).unwrap();
}

#[cfg(unix)]
#[test]
fn diagnostics_report_has_counts_but_no_names() {
    let dir = tempfile::tempdir().unwrap();
    crate::logging::init(&dir.path().join("logs"));
    let c = core(&dir);
    let ws = c.create_workspace("Secret Project Phoenix").unwrap();
    let shell = shell_id(&c);
    let fp = fingerprint(&c, &shell);
    c.add_member(&ws, &shell, &fp, Some("prod db shell".into()))
        .unwrap();
    crate::logging::info("test", "diagnostics marker line");
    let report = c.diagnostics_report();
    assert!(report.contains("workspaces: 1, members: 1"));
    assert!(report.contains("\"font_size\""), "settings included");
    assert!(report.contains("[recent log]"));
    for private in ["Secret Project Phoenix", "prod db shell"] {
        assert!(!report.contains(private), "{private} leaked");
    }
    if crate::logging::log_path().is_some_and(|p| p.starts_with(dir.path())) {
        assert!(report.contains("diagnostics marker line"));
    }
}

#[cfg(unix)]
fn notes_fixture(dir: &tempfile::TempDir) -> (Arc<Core>, String, String) {
    let c = core(dir);
    let ws = c.create_workspace("Notes WS").unwrap();
    let shell = shell_id(&c);
    let fp = fingerprint(&c, &shell);
    let member = c
        .add_member(&ws, &shell, &fp, Some("Build server".into()))
        .unwrap();
    (c, ws, member)
}

#[cfg(unix)]
#[test]
fn notes_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (ws, member) = {
        let (c, ws, member) = notes_fixture(&dir);
        c.save_note(&ws, None, "release on Thursday\nسطر بالعربي")
            .unwrap();
        c.save_note(&ws, Some(&member), "tail -f /var/log/app.log")
            .unwrap();
        (ws, member)
    };
    // A new core (= the app started again) reads the notes back from disk.
    let c = core(&dir);
    let v = c.notes_view(&ws).unwrap();
    assert_eq!(v.project, "release on Thursday\nسطر بالعربي");
    assert_eq!(v.members[&member], "tail -f /var/log/app.log");
    // Clearing a member note removes it.
    c.save_note(&ws, Some(&member), "").unwrap();
    assert!(core(&dir).notes_view(&ws).unwrap().members.is_empty());
}

#[cfg(unix)]
#[test]
fn notes_recover_from_a_torn_or_corrupt_file() {
    let dir = tempfile::tempdir().unwrap();
    let (c, ws, _) = notes_fixture(&dir);
    c.save_note(&ws, None, "version one").unwrap();
    c.save_note(&ws, None, "version two").unwrap();
    drop(c);
    let file = dir
        .path()
        .join("state/notes")
        .join(format!("notes-{ws}.json"));
    // Power cut while the newest file was half written.
    let bytes = std::fs::read(&file).unwrap();
    std::fs::write(&file, &bytes[..bytes.len() / 2]).unwrap();
    // A leftover temp file from an interrupted save must not matter either.
    std::fs::write(
        dir.path().join(format!("state/notes/.notes-{ws}.json.tmp")),
        b"garbage",
    )
    .unwrap();
    let c = core(&dir);
    assert_eq!(
        c.notes_view(&ws).unwrap().project,
        "version one",
        "falls back to the previous complete save"
    );
    // And the next save writes a clean newest file again.
    c.save_note(&ws, None, "version three").unwrap();
    assert_eq!(core(&dir).notes_view(&ws).unwrap().project, "version three");
}

#[cfg(unix)]
#[test]
fn removing_a_terminal_keeps_its_note_in_the_project_note() {
    let dir = tempfile::tempdir().unwrap();
    let (c, ws, member) = notes_fixture(&dir);
    c.save_note(&ws, None, "plan").unwrap();
    c.save_note(&ws, Some(&member), "remember to rotate keys")
        .unwrap();
    c.remove_member(&ws, &member).unwrap();
    let v = core(&dir).notes_view(&ws).unwrap();
    assert!(v.members.is_empty());
    assert!(v.project.contains("“Build server”"));
    assert!(v.project.ends_with("remember to rotate keys"));
}

#[cfg(unix)]
#[test]
fn deleted_workspace_notes_can_be_restored() {
    let dir = tempfile::tempdir().unwrap();
    let (c, ws, member) = notes_fixture(&dir);
    c.save_note(&ws, None, "old project plan").unwrap();
    c.save_note(&ws, Some(&member), "old terminal note")
        .unwrap();
    c.delete_workspace(&ws).unwrap();
    assert!(!dir
        .path()
        .join(format!("state/notes/notes-{ws}.json"))
        .exists());
    let other = c.create_workspace("Other").unwrap();
    let v = c.notes_view(&other).unwrap();
    assert_eq!(v.trash.len(), 1);
    assert_eq!(v.trash[0].workspace_name, "Notes WS");
    assert_eq!(v.trash[0].notes, 2);
    c.restore_trashed_notes(&v.trash[0].id, &other).unwrap();
    let v = core(&dir).notes_view(&other).unwrap();
    assert!(v.trash.is_empty());
    assert!(v.project.contains("“Notes WS”"));
    assert!(v.project.contains("old project plan"));
    assert!(v.project.contains("[Build server]\nold terminal note"));
}

#[cfg(unix)]
#[test]
fn notes_validate_targets_and_size_and_stay_private() {
    let dir = tempfile::tempdir().unwrap();
    let (c, ws, member) = notes_fixture(&dir);
    assert!(c.save_note(&ws, Some("ghost"), "x").is_err());
    assert!(c.save_note("ws_missing", None, "x").is_err());
    assert!(c.save_note("../etc", None, "x").is_err());
    c.save_note(&ws, Some(&member), "kept").unwrap();
    let too_big = "x".repeat(crate::notes::MAX_NOTE_BYTES + 1);
    assert!(c.save_note(&ws, Some(&member), &too_big).is_err());
    assert_eq!(c.notes_view(&ws).unwrap().members[&member], "kept");
    // Notes never leave through exports, diagnostics or the audit log.
    c.save_note(&ws, None, "PRIVATE-NOTE-MARKER").unwrap();
    assert!(!c
        .export_workspace(&ws, false)
        .unwrap()
        .contains("PRIVATE-NOTE-MARKER"));
    assert!(!c.diagnostics_report().contains("PRIVATE-NOTE-MARKER"));
    assert!(!format!("{:?}", c.audit_recent(500)).contains("PRIVATE-NOTE-MARKER"));
}

#[cfg(target_os = "linux")]
#[test]
fn terminals_start_in_member_then_project_folder() {
    let dir = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let here = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let ws = c.create_workspace("Folders").unwrap();
    let shell = shell_id(&c);
    let fp = fingerprint(&c, &shell);
    assert!(c.set_project_dir(&ws, Some("relative")).is_err());
    c.set_project_dir(&ws, Some(&project.path().to_string_lossy()))
        .unwrap();
    assert_eq!(
        c.workspaces().0[0].project_dir.as_deref(),
        Some(&*project.path().to_string_lossy())
    );
    let cwd_of = |member: &str| {
        let info = c.start_session(&ws, member, 24, 80).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let cwd = loop {
            let cwd = info.pid.and_then(crate::session::process_cwd);
            if cwd.is_some() || Instant::now() > deadline {
                break cwd;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        c.close_session(&info.session_id).unwrap();
        cwd.map(|p| std::path::PathBuf::from(p).canonicalize().unwrap())
    };
    let a = c.add_member(&ws, &shell, &fp, None).unwrap();
    assert_eq!(cwd_of(&a), Some(project.path().canonicalize().unwrap()));
    // The member's own folder wins over the project folder.
    let b = c.add_member(&ws, &shell, &fp, None).unwrap();
    let patch = |d: &str| MemberPatch {
        title: None,
        auto_restart: None,
        detach_expected: None,
        color: None,
        start_dir: Some(d.into()),
        zoom: None,
    };
    c.update_member(&ws, &b, patch(&here.path().to_string_lossy()))
        .unwrap();
    assert_eq!(cwd_of(&b), Some(here.path().canonicalize().unwrap()));
    // A folder that disappeared does not block the start.
    let gone = here.path().to_path_buf();
    drop(here);
    assert!(!gone.exists());
    assert!(cwd_of(&b).is_some());
    assert!(c
        .update_member(&ws, &b, patch("/definitely/missing"))
        .is_err());
}

#[test]
fn notifications_respect_settings_limits_and_rate() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let (t, b) = c
        .prepare_notification("Build\nserver exited", &"x".repeat(1000))
        .unwrap();
    assert_eq!(t, "Build server exited");
    assert_eq!(b.chars().count(), 400);
    assert!(c.prepare_notification("   ", "empty title").is_none());
    for _ in 0..5 {
        assert!(c.prepare_notification("bell", "").is_some());
    }
    assert!(c.prepare_notification("bell", "").is_none(), "rate limited");
    let mut s = c.settings();
    s.desktop_notifications = false;
    c.update_settings(s).unwrap();
    let c2 = core(&dir);
    assert!(c2.prepare_notification("off", "").is_none());
}

#[test]
fn window_title_follows_the_active_workspace_and_internal_sinks_survive() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    assert_eq!(c.window_title(), "ToolOnize");
    let internal = Arc::new(Events::default());
    c.add_internal_sink(internal.clone());
    let ui = Arc::new(Events::default());
    c.subscribe(ui.clone());
    let ws = c.create_workspace("Phoenix").unwrap();
    c.set_active(Some(&ws)).unwrap();
    assert_eq!(c.window_title(), "Phoenix — ToolOnize");
    // A renderer reload resubscribes the UI; the internal sink keeps going.
    c.subscribe(Arc::new(Events::default()));
    c.rename_workspace(&ws, "Phoenix 2").unwrap();
    assert_eq!(c.window_title(), "Phoenix 2 — ToolOnize");
    let count = |e: &Events| {
        e.0.lock()
            .unwrap()
            .iter()
            .filter(|x| matches!(x, AppEvent::Workspaces))
            .count()
    };
    assert!(
        count(&internal) > count(&ui),
        "internal sink saw the rename"
    );
}

#[test]
fn audit_log_exports_as_json_lines() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    c.create_workspace("A").unwrap();
    c.open_url("https://example.com").ok();
    let text = c.audit.export_text();
    assert!(text.lines().count() >= 1);
    for line in text.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v["action"].is_string());
    }
}

#[cfg(unix)]
fn backup_fixture(c: &Core) -> (String, String, String) {
    let project = std::env::temp_dir();
    let ws = c.create_workspace("Backup WS").unwrap();
    let shell = shell_id(c);
    let fp = fingerprint(c, &shell);
    let a = c.add_member(&ws, &shell, &fp, Some("API".into())).unwrap();
    let b = c.add_member(&ws, &shell, &fp, Some("Logs".into())).unwrap();
    c.set_project_dir(&ws, Some(&project.to_string_lossy()))
        .unwrap();
    c.set_policy(&ws, StartupPolicy::RestoreAll).unwrap();
    let tab = |m: &str| json!({"type": "tab", "id": format!("tab-{m}"), "component": "member", "config": {"memberId": m}});
    c.save_layout(
        &ws,
        LayoutMode::Grid,
        Some(json!({"layout": {"type": "row", "children": [
            {"type": "tabset", "id": format!("ts-{a}"), "children": [tab(&a)]},
            {"type": "tabset", "children": [tab(&b)]}
        ]}})),
        Some(b.clone()),
    )
    .unwrap();
    c.save_note(&ws, None, "release on friday").unwrap();
    c.save_note(&ws, Some(&b), "tail -f app.log").unwrap();
    c.set_alias(&shell, Some("My shell".into())).unwrap();
    (ws, a, b)
}

#[cfg(unix)]
#[test]
fn backup_restores_everything_staged_and_unauthorized() {
    let src_dir = tempfile::tempdir().unwrap();
    let src = core(&src_dir);
    let (ws, _, b) = backup_fixture(&src);
    src.save_template(&ws, "Two terminals").unwrap();
    let mut settings = src.settings();
    settings.font_size = 17;
    src.update_settings(settings).unwrap();
    let text = src.backup_text(true).unwrap();
    assert!(
        !text.contains("authorized_fingerprint\": \""),
        "no authorizations in the file"
    );
    assert!(text.contains("release on friday"));

    // A different machine (fresh state).
    let dst_dir = tempfile::tempdir().unwrap();
    let dst = core(&dst_dir);
    let report = dst.restore_backup(&text, false).unwrap();
    assert_eq!(report.workspaces, 1);
    assert_eq!(report.members, 2);
    assert_eq!(report.notes, 2);
    assert_eq!(report.templates, 1);
    assert!(!report.settings_restored);
    assert_eq!(dst.settings().font_size, Settings::default().font_size);

    let check = |c: &Core| {
        let (list, _) = c.workspaces();
        let w = list
            .iter()
            .find(|w| w.name == "Backup WS (restored)")
            .unwrap();
        assert!(w.staged_import, "restored workspaces wait for review");
        assert_eq!(w.startup_policy, StartupPolicy::Ask);
        assert_ne!(w.id, ws);
        for m in &w.members {
            assert_eq!(m.status.state, policy::MemberState::NotAuthorized);
            assert!(c.start_session(&w.id, &m.member.id, 24, 80).is_err());
        }
        let ids: Vec<String> = w.members.iter().map(|m| m.member.id.clone()).collect();
        assert!(!ids.contains(&b));
        let shown = crate::workspace::validate_layout(w.layout.json.as_ref().unwrap(), &ids);
        assert_eq!(shown.unwrap(), ids);
        assert_eq!(w.layout.focused_member.as_deref(), Some(ids[1].as_str()));
        assert_eq!(w.members[0].member.title.as_deref(), Some("API"));
        let notes = c.notes_view(&w.id).unwrap();
        assert_eq!(notes.project, "release on friday");
        assert_eq!(notes.members[&ids[1]], "tail -f app.log");
        assert_eq!(c.templates()[0].name, "Two terminals");
        assert_eq!(c.templates()[0].members, 2);
    };
    check(&dst);
    // Alias came along; the launcher approval/authorization did not.
    assert_eq!(
        dst.registry_entry(&shell_id(&dst))
            .unwrap()
            .decision
            .alias
            .as_deref(),
        Some("My shell")
    );
    // Everything is on disk: a restart shows the same.
    drop(dst);
    let dst = core(&dst_dir);
    check(&dst);

    // Authorizing a restored member makes it startable again.
    let w = dst.workspaces().0.remove(0);
    let m = &w.members[0];
    dst.authorize_member(&w.id, &m.member.id, &fingerprint(&dst, &shell_id(&dst)))
        .unwrap();
    let info = dst.start_session(&w.id, &m.member.id, 24, 80).unwrap();
    dst.close_session(&info.session_id).unwrap();

    // Restoring again adds another copy; with settings this time.
    let report = dst.restore_backup(&text, true).unwrap();
    assert!(report.settings_restored);
    assert_eq!(dst.settings().font_size, 17);
    assert_eq!(dst.workspaces().0.len(), 2);
}

#[cfg(unix)]
#[test]
fn failed_restore_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let (ws, _, _) = backup_fixture(&c);
    let before = c.workspaces().0.len();
    let notes_dir = dir.path().join("state/notes");
    let files = || std::fs::read_dir(&notes_dir).unwrap().count();
    let before_files = files();
    for bad in ["", "{}", "[1,2]", "{\"format\":\"toolonize-backup\"}"] {
        assert!(c.restore_backup(bad, true).is_err(), "{bad}");
    }
    // A valid file that does not fit (workspace limit) leaves no notes behind.
    let text = c.backup_text(false).unwrap();
    for i in 0..(MAX_WORKSPACES - before) {
        c.create_workspace(&format!("Filler {i}")).unwrap();
    }
    assert!(c.restore_backup(&text, false).is_err());
    assert_eq!(files(), before_files);
    assert_eq!(c.workspaces().0.len(), MAX_WORKSPACES);
    assert!(c.workspaces().0.iter().any(|w| w.id == ws));
}

#[cfg(unix)]
#[test]
fn duplicate_copies_members_layout_and_notes() {
    let dir = tempfile::tempdir().unwrap();
    let c = core(&dir);
    let (ws, a, b) = backup_fixture(&c);
    let copy = c.duplicate_workspace(&ws).unwrap();
    let (list, active) = c.workspaces();
    assert_eq!(active.as_deref(), Some(copy.as_str()));
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].id, copy, "the copy sits right after the original");
    let w = &list[1];
    assert_eq!(w.name, "Backup WS (copy)");
    assert!(!w.staged_import);
    let ids: Vec<String> = w.members.iter().map(|m| m.member.id.clone()).collect();
    assert!(!ids.contains(&a) && !ids.contains(&b));
    // Same machine, same launchers: the copy can start right away.
    assert!(w
        .members
        .iter()
        .all(|m| m.status.state == policy::MemberState::Ready));
    let info = c.start_session(&copy, &ids[0], 24, 80).unwrap();
    c.close_session(&info.session_id).unwrap();
    assert_eq!(
        crate::workspace::validate_layout(w.layout.json.as_ref().unwrap(), &ids).unwrap(),
        ids
    );
    let notes = c.notes_view(&copy).unwrap();
    assert_eq!(notes.project, "release on friday");
    assert_eq!(notes.members[&ids[1]], "tail -f app.log");
    // Notes are separate afterwards.
    c.save_note(&copy, None, "copy only").unwrap();
    assert_eq!(c.notes_view(&ws).unwrap().project, "release on friday");
    assert!(c.duplicate_workspace("ws_missing").is_err());
    assert!(c.duplicate_workspace("../x").is_err());
}

#[cfg(unix)]
#[test]
fn templates_save_create_delete_and_persist() {
    let dir = tempfile::tempdir().unwrap();
    let (tpl, ws) = {
        let c = core(&dir);
        let (ws, _, _) = backup_fixture(&c);
        let tpl = c.save_template(&ws, "Dev pair").unwrap();
        assert!(
            c.save_template(&ws, "Dev pair").is_err(),
            "names are unique"
        );
        assert!(c.save_template(&ws, "  ").is_err());
        assert!(c.save_template("ws_missing", "X").is_err());
        (tpl, ws)
    };
    let c = core(&dir);
    let list = c.templates();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, tpl);
    let new = c.create_from_template(&tpl, "From template").unwrap();
    let (all, active) = c.workspaces();
    assert_eq!(active.as_deref(), Some(new.as_str()));
    let w = all.iter().find(|w| w.id == new).unwrap();
    assert_eq!(w.members.len(), 2);
    assert!(w.project_dir.is_none(), "folders are project-specific");
    assert!(
        c.notes_view(&new).unwrap().project.is_empty(),
        "notes are not copied"
    );
    let original: Vec<String> = all
        .iter()
        .find(|w| w.id == ws)
        .unwrap()
        .members
        .iter()
        .map(|m| m.member.id.clone())
        .collect();
    assert!(w.members.iter().all(|m| !original.contains(&m.member.id)));
    // Two workspaces from one template never share ids.
    let again = c.create_from_template(&tpl, "Again").unwrap();
    let ids = |id: &str| -> Vec<String> {
        c.workspaces()
            .0
            .iter()
            .find(|w| w.id == id)
            .unwrap()
            .members
            .iter()
            .map(|m| m.member.id.clone())
            .collect()
    };
    assert!(ids(&again).iter().all(|m| !ids(&new).contains(m)));
    assert!(c.create_from_template("tpl_missing", "X").is_err());
    c.delete_template(&tpl).unwrap();
    assert!(c.templates().is_empty());
    assert!(c.delete_template(&tpl).is_err());
    assert!(core(&dir).templates().is_empty());
}
