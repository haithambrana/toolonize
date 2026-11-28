//! Full backup and restore (ADR-014), and copying workspaces with fresh ids
//! (duplicate, templates).
//!
//! A backup is a local file with everything needed to rebuild ToolOnize on a
//! new machine: workspaces with layouts, member options and notes, templates,
//! launcher aliases/hidden flags, custom roots, typed connections and
//! (optionally) settings. Restoring treats the file like an import (T-CFG-01):
//! everything is validated, restored workspaces land next to the existing
//! ones in review staging, every member must be authorized again, launcher
//! approvals are not restored, and nothing starts automatically.

use crate::model::{new_id, now_ms, valid_id};
use crate::notes::{clean_text, WorkspaceNotes};
use crate::settings::Settings;
use crate::shells::SyntheticSpec;
use crate::workspace::{
    validate_color, validate_dir, validate_layout, validate_name, validate_title, StartupPolicy,
    Workspace, MAX_MEMBERS, MAX_WORKSPACES,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

pub const BACKUP_FORMAT: &str = "toolonize-backup";
pub const BACKUP_SCHEMA: u32 = 1;
pub const MAX_BACKUP_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TEMPLATES: usize = 50;

/// A saved workspace shape to start new workspaces from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub created_at: u64,
    pub workspace: Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backup {
    pub format: String,
    pub schema_version: u32,
    pub created_at: u64,
    pub app_version: String,
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    pub templates: Vec<Template>,
    /// launcher id -> alias
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default)]
    pub custom_roots: Vec<String>,
    #[serde(default)]
    pub synthetic: Vec<SyntheticSpec>,
    /// Kept as raw JSON so a settings block from another version only skips
    /// the settings, not the whole restore.
    #[serde(default)]
    pub settings: Option<Value>,
    /// workspace id (as in this file) -> notes
    #[serde(default)]
    pub notes: BTreeMap<String, WorkspaceNotes>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RestoreReport {
    pub workspaces: usize,
    pub members: usize,
    pub notes: usize,
    pub templates: usize,
    pub connections: usize,
    pub settings_restored: bool,
    pub warnings: Vec<String>,
}

/// Replace every occurrence of the old ids in a layout snapshot (member ids
/// in tab configs and the `tab-<id>` / `ts-<id>` node ids).
fn remap_layout(value: &mut Value, ids: &HashMap<String, String>) {
    match value {
        Value::String(s) => {
            for (old, new) in ids {
                if s == old {
                    *s = new.clone();
                } else if s.strip_prefix("tab-") == Some(old.as_str()) {
                    *s = format!("tab-{new}");
                } else if s.strip_prefix("ts-") == Some(old.as_str()) {
                    *s = format!("ts-{new}");
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| remap_layout(v, ids)),
        Value::Object(map) => map.values_mut().for_each(|v| remap_layout(v, ids)),
        _ => {}
    }
}

/// Copy a workspace with a new workspace id and new member ids, the layout
/// rewritten to match. Returns the copy and the old -> new member id map.
pub fn copy_with_new_ids(w: &Workspace, name: String) -> (Workspace, HashMap<String, String>) {
    let mut copy = w.clone();
    copy.id = new_id("ws");
    copy.name = name;
    copy.created_at = now_ms();
    copy.updated_at = copy.created_at;
    copy.staged_import = false;
    let ids: HashMap<String, String> = w
        .members
        .iter()
        .map(|m| (m.id.clone(), new_id("m")))
        .collect();
    for m in &mut copy.members {
        m.id = ids[&m.id].clone();
        m.added_at = copy.created_at;
    }
    if let Some(json) = copy.layout.json.as_mut() {
        remap_layout(json, &ids);
    }
    copy.layout.focused_member = w
        .layout
        .focused_member
        .as_ref()
        .and_then(|f| ids.get(f).cloned());
    (copy, ids)
}

/// Validate one workspace from a backup and turn it into a staged, fully
/// unauthorized workspace with fresh ids.
fn restore_workspace(
    w: &Workspace,
    warnings: &mut Vec<String>,
) -> Result<(Workspace, HashMap<String, String>), String> {
    let name = validate_name(&w.name)?;
    if w.members.len() > MAX_MEMBERS {
        return Err(format!("workspace “{name}” has too many members"));
    }
    let mut seen = HashSet::new();
    for m in &w.members {
        if !valid_id(&m.id) || m.id.len() > 64 || !seen.insert(m.id.clone()) {
            return Err(format!("workspace “{name}” has invalid member ids"));
        }
        if !valid_id(&m.launcher_id) {
            return Err(format!(
                "workspace “{name}” has an invalid launcher reference"
            ));
        }
    }
    let member_ids: Vec<String> = w.members.iter().map(|m| m.id.clone()).collect();
    let mut source = w.clone();
    if let Some(json) = &source.layout.json {
        if let Err(error) = validate_layout(json, &member_ids) {
            warnings.push(format!("“{name}”: layout discarded ({error})"));
            source.layout.json = None;
        }
    }
    let (mut out, ids) = copy_with_new_ids(&source, format!("{name} (restored)"));
    out.name = out.name.chars().take(80).collect();
    out.staged_import = true;
    out.startup_policy = StartupPolicy::Ask;
    out.color = validate_color(out.color.as_deref()).unwrap_or(None);
    if let Some(dir) = out.project_dir.clone() {
        if validate_dir(Some(&dir)).is_err() {
            warnings.push(format!(
                "“{name}”: project folder {dir} does not exist here"
            ));
            out.project_dir = None;
        }
    }
    for m in &mut out.members {
        m.authorized_fingerprint = None;
        m.authorized_at = None;
        m.title = validate_title(m.title.clone())?;
        m.color = validate_color(m.color.as_deref()).unwrap_or(None);
        m.zoom = m.zoom.clamp(-6, 12);
        // Environment and startup command wait for the user's confirmation;
        // invalid ones are dropped with a warning.
        m.launch.confirmed = None;
        if let Err(error) = crate::workspace::validate_launch(
            &m.launch.env,
            m.launch.startup_command.as_deref(),
            m.member_type,
        ) {
            warnings.push(format!("“{name}”: terminal settings dropped ({error})"));
            m.launch = Default::default();
        }
        if let Some(dir) = m.start_dir.clone() {
            if validate_dir(Some(&dir)).is_err() {
                m.start_dir = None;
            }
        }
    }
    Ok((out, ids))
}

#[derive(Debug)]
pub struct Restored {
    pub workspaces: Vec<Workspace>,
    pub templates: Vec<Template>,
    pub notes: Vec<(String, WorkspaceNotes)>,
    pub aliases: BTreeMap<String, String>,
    pub hidden: Vec<String>,
    pub custom_roots: Vec<String>,
    pub synthetic: Vec<SyntheticSpec>,
    pub settings: Option<Settings>,
    pub report: RestoreReport,
}

pub fn parse_and_validate(
    text: &str,
    restore_settings: bool,
    existing_workspaces: usize,
) -> Result<Restored, String> {
    if text.len() > MAX_BACKUP_BYTES {
        return Err("the backup file is too large".into());
    }
    let backup: Backup =
        serde_json::from_str(text).map_err(|e| format!("not a ToolOnize backup: {e}"))?;
    if backup.format != BACKUP_FORMAT {
        return Err("not a ToolOnize backup".into());
    }
    if backup.schema_version != BACKUP_SCHEMA {
        return Err(format!(
            "unsupported backup version {} (this version reads {BACKUP_SCHEMA})",
            backup.schema_version
        ));
    }
    if existing_workspaces + backup.workspaces.len() > MAX_WORKSPACES {
        return Err(format!("at most {MAX_WORKSPACES} workspaces"));
    }
    let mut report = RestoreReport::default();
    let mut workspaces = Vec::new();
    let mut notes = Vec::new();
    for w in &backup.workspaces {
        let (restored, ids) = restore_workspace(w, &mut report.warnings)?;
        if let Some(n) = backup.notes.get(&w.id) {
            let mut mapped = WorkspaceNotes {
                project: n.project.clone(),
                members: BTreeMap::new(),
            };
            mapped.project.text = clean_text(&mapped.project.text)?;
            for (old, note) in &n.members {
                if let Some(new) = ids.get(old) {
                    let mut note = note.clone();
                    note.text = clean_text(&note.text)?;
                    mapped.members.insert(new.clone(), note);
                }
            }
            if mapped.total_bytes() > crate::notes::MAX_WORKSPACE_BYTES {
                return Err(format!("the notes of “{}” are too large", w.name));
            }
            if !mapped.is_empty() {
                report.notes += usize::from(!mapped.project.text.is_empty()) + mapped.members.len();
                notes.push((restored.id.clone(), mapped));
            }
        }
        report.members += restored.members.len();
        workspaces.push(restored);
    }
    report.workspaces = workspaces.len();
    let mut templates = Vec::new();
    for t in backup.templates.iter().take(MAX_TEMPLATES) {
        let (mut w, _) = restore_workspace(&t.workspace, &mut report.warnings)?;
        w.staged_import = false;
        templates.push(Template {
            id: new_id("tpl"),
            name: validate_name(&t.name)?,
            created_at: t.created_at,
            workspace: w,
        });
    }
    report.templates = templates.len();
    let aliases = backup
        .aliases
        .into_iter()
        .filter(|(id, a)| valid_id(id) && !a.trim().is_empty() && a.chars().count() <= 80)
        .collect();
    let hidden = backup
        .hidden
        .into_iter()
        .filter(|id| valid_id(id))
        .collect();
    let custom_roots: Vec<String> = backup
        .custom_roots
        .into_iter()
        .filter(|r| validate_dir(Some(r)).ok().flatten().is_some())
        .collect();
    let synthetic: Vec<SyntheticSpec> = backup
        .synthetic
        .into_iter()
        .filter(|s| s.validate().is_ok())
        .collect();
    report.connections = synthetic.len();
    let settings = match (restore_settings, backup.settings) {
        (true, Some(s)) => match serde_json::from_value::<Settings>(s)
            .map_err(|e| e.to_string())
            .and_then(|s| s.validate().map(|()| s))
        {
            Ok(s) => Some(s),
            Err(error) => {
                report
                    .warnings
                    .push(format!("settings not restored: {error}"));
                None
            }
        },
        _ => None,
    };
    report.settings_restored = settings.is_some();
    Ok(Restored {
        workspaces,
        templates,
        notes,
        aliases,
        hidden,
        custom_roots,
        synthetic,
        settings,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Member, MemberType};
    use serde_json::json;

    fn member(id: &str) -> Member {
        Member {
            id: id.into(),
            launcher_id: "sys:shell:sh".into(),
            member_type: MemberType::Terminal,
            title: Some(format!("T {id}")),
            authorized_fingerprint: Some("fp".into()),
            authorized_at: Some(1),
            auto_restart: false,
            detach_expected: false,
            added_at: 1,
            portable: None,
            color: Some("teal".into()),
            start_dir: None,
            zoom: 3,
            launch: Default::default(),
        }
    }

    fn tab(id: &str) -> Value {
        json!({"type": "tab", "id": format!("tab-{id}"), "name": format!("T {id}"),
               "component": "member", "config": {"memberId": id}})
    }

    fn workspace() -> Workspace {
        let mut w = Workspace::new("ws_a1".into(), "Alpha".into());
        w.members = vec![member("m_1"), member("m_2")];
        w.layout.json = Some(json!({"global": {}, "borders": [], "layout": {
        "type": "row", "id": "root", "children": [
            {"type": "tabset", "id": "ts-m_1", "children": [tab("m_1")]},
            {"type": "tabset", "id": "ts-x", "children": [tab("m_2")]}
        ]}}));
        w.layout.focused_member = Some("m_2".into());
        w
    }

    fn backup_json(workspaces: Vec<Workspace>, notes: Value) -> String {
        json!({
            "format": BACKUP_FORMAT, "schema_version": 1, "created_at": 1,
            "app_version": "0", "workspaces": workspaces, "notes": notes,
        })
        .to_string()
    }

    #[test]
    fn copies_get_fresh_ids_and_a_matching_layout() {
        let w = workspace();
        let (copy, ids) = copy_with_new_ids(&w, "Beta".into());
        assert_ne!(copy.id, w.id);
        assert_eq!(copy.name, "Beta");
        assert_eq!(ids.len(), 2);
        let new: Vec<String> = copy.members.iter().map(|m| m.id.clone()).collect();
        assert!(new.iter().all(|id| id != "m_1" && id != "m_2"));
        assert_ne!(new[0], new[1]);
        let shown = validate_layout(copy.layout.json.as_ref().unwrap(), &new).unwrap();
        assert_eq!(shown, new);
        let text = copy.layout.json.as_ref().unwrap().to_string();
        assert!(
            !text.contains("\"m_1\"") && !text.contains("-m_1\""),
            "{text}"
        );
        assert!(
            !text.contains("\"m_2\"") && !text.contains("-m_2\""),
            "{text}"
        );
        assert!(text.contains(&format!("tab-{}", new[0])));
        assert!(text.contains(&format!("ts-{}", new[0])));
        assert!(text.contains("ts-x"), "unrelated ids stay");
        assert_eq!(copy.layout.focused_member.as_deref(), Some(new[1].as_str()));
        // Options are kept (titles, colors, zoom).
        assert_eq!(copy.members[0].title.as_deref(), Some("T m_1"));
        assert_eq!(copy.members[0].zoom, 3);
    }

    #[test]
    fn restore_stages_and_removes_every_authorization() {
        let mut w = workspace();
        w.startup_policy = StartupPolicy::RestoreAll;
        w.project_dir = Some("/definitely/not/here".into());
        w.members[0].start_dir = Some("/definitely/not/here".into());
        w.members[1].zoom = 100;
        let text = backup_json(
            vec![w],
            json!({"ws_a1": {"project": {"text": "plan", "updated_at": 5},
                   "members": {"m_2": {"text": "ssh box", "updated_at": 6}, "ghost": {"text": "x", "updated_at": 1}}}}),
        );
        let r = parse_and_validate(&text, false, 0).unwrap();
        let w = &r.workspaces[0];
        assert_eq!(w.name, "Alpha (restored)");
        assert!(w.staged_import);
        assert_eq!(w.startup_policy, StartupPolicy::Ask);
        assert!(w.project_dir.is_none());
        assert!(w
            .members
            .iter()
            .all(|m| m.authorized_fingerprint.is_none() && m.authorized_at.is_none()));
        assert!(w.members[0].start_dir.is_none());
        assert_eq!(w.members[1].zoom, 12);
        assert_eq!(r.report.workspaces, 1);
        assert_eq!(r.report.members, 2);
        assert_eq!(
            r.report.notes, 2,
            "the note of an unknown member is dropped"
        );
        let (id, notes) = &r.notes[0];
        assert_eq!(id, &w.id);
        assert_eq!(notes.project.text, "plan");
        assert_eq!(notes.members[&w.members[1].id].text, "ssh box");
        assert!(r
            .report
            .warnings
            .iter()
            .any(|x| x.contains("project folder")));
    }

    #[test]
    fn hostile_or_foreign_files_are_refused() {
        let ok = backup_json(vec![workspace()], json!({}));
        assert!(parse_and_validate(&ok, false, 0).is_ok());
        assert!(parse_and_validate("not json", false, 0).is_err());
        assert!(parse_and_validate(&ok.replace(BACKUP_FORMAT, "other"), false, 0).is_err());
        assert!(parse_and_validate(
            &ok.replace("\"schema_version\":1", "\"schema_version\":2"),
            false,
            0
        )
        .unwrap_err()
        .contains("version"));
        // Unknown top-level fields are refused (no smuggled data).
        let extra = ok.replacen('{', "{\"run\":\"rm -rf /\",", 1);
        assert!(parse_and_validate(&extra, false, 0).is_err());
        // Too large.
        let big = format!("{ok}{}", " ".repeat(MAX_BACKUP_BYTES));
        assert!(parse_and_validate(&big, false, 0)
            .unwrap_err()
            .contains("large"));
        // Too many workspaces in total.
        assert!(parse_and_validate(&ok, false, MAX_WORKSPACES).is_err());
        // Duplicate or invalid member ids.
        let mut dup = workspace();
        dup.members[1].id = "m_1".into();
        assert!(parse_and_validate(&backup_json(vec![dup], json!({})), false, 0).is_err());
        let mut bad = workspace();
        bad.members[0].id = "../x".into();
        assert!(parse_and_validate(&backup_json(vec![bad], json!({})), false, 0).is_err());
        let mut name = workspace();
        name.name = " ".into();
        assert!(parse_and_validate(&backup_json(vec![name], json!({})), false, 0).is_err());
        // A layout that does not match its members is dropped with a warning.
        let mut layout = workspace();
        layout.members.pop();
        let r = parse_and_validate(&backup_json(vec![layout], json!({})), false, 0).unwrap();
        assert!(r.workspaces[0].layout.json.is_none());
        assert!(r.report.warnings[0].contains("layout"));
        // Oversized notes.
        let huge = "x".repeat(crate::notes::MAX_NOTE_BYTES + 1);
        let notes = json!({"ws_a1": {"project": {"text": huge, "updated_at": 1}}});
        assert!(parse_and_validate(&backup_json(vec![workspace()], notes), false, 0).is_err());
    }

    #[test]
    fn settings_only_when_asked_and_valid() {
        let mut v: Value = serde_json::from_str(&backup_json(vec![], json!({}))).unwrap();
        let settings = Settings {
            font_size: 17,
            ..Settings::default()
        };
        v["settings"] = serde_json::to_value(&settings).unwrap();
        let text = v.to_string();
        assert!(parse_and_validate(&text, false, 0)
            .unwrap()
            .settings
            .is_none());
        let r = parse_and_validate(&text, true, 0).unwrap();
        assert_eq!(r.settings.unwrap().font_size, 17);
        assert!(r.report.settings_restored);
        for bad in [json!(40), json!(500), json!("big")] {
            v["settings"]["font_size"] = bad;
            let r = parse_and_validate(&v.to_string(), true, 0).unwrap();
            assert!(r.settings.is_none());
            assert!(!r.report.settings_restored);
            assert!(r.report.warnings[0].contains("settings"));
        }
    }
}
