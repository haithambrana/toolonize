//! Core integration tests: registry -> workspace -> policy -> sessions ->
//! persistence, against real temp dirs and (on Unix) the real PTY.

use super::*;
use serde_json::json;

fn core(dir: &tempfile::TempDir) -> Arc<Core> {
    let core = Core::new(dir.path().join("state"), dir.path().join("config"));
    core.rescan().unwrap();
    core
}

fn write_entry(root: &std::path::Path, name: &str, exec: &str) {
    std::fs::write(
        root.join(name),
        format!(
            "[Desktop Entry]\nType=Application\nName=Fixture {name}\nExec={exec}\nTerminal=true\n"
        ),
    )
    .unwrap();
}

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
