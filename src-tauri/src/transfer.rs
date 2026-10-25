//! Workspace import/export (FR-025, T-CFG-01, SEC-004).
//!
//! Exports are non-secret JSON: members travel as portable hints, never as
//! command lines, and machine-specific absolute paths are refused unless the
//! user explicitly asks for a machine-bound export. Imports are parsed into
//! a review staging state: nothing is authorized and nothing executes.

use crate::model::{new_id, now_ms, valid_id};
use crate::shells::SyntheticSpec;
use crate::workspace::{
    validate_layout, validate_name, validate_title, LayoutMode, LayoutState, Member, MemberType,
    PortableHint, StartupPolicy, Workspace, MAX_MEMBERS,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORMAT: &str = "toolonize-workspace";
pub const EXPORT_SCHEMA: u32 = 1;
pub const MAX_IMPORT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportedMember {
    pub id: String,
    pub member_type: MemberType,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub auto_restart: bool,
    #[serde(default)]
    pub detach_expected: bool,
    pub portable: PortableHint,
    /// Present only in machine-bound exports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportedLayout {
    pub mode: LayoutMode,
    #[serde(default)]
    pub json: Option<Value>,
    #[serde(default)]
    pub focused_member: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportedWorkspace {
    pub name: String,
    #[serde(default)]
    pub startup_policy: StartupPolicy,
    pub layout: ExportedLayout,
    pub members: Vec<ExportedMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub format: String,
    pub schema_version: u32,
    pub exported_at: u64,
    pub machine_bound: bool,
    pub workspace: ExportedWorkspace,
}

/// Portable hint for a launcher id. Machine-specific locations collapse to
/// a display name only.
pub fn portable_hint(launcher_id: &str, display_name: &str) -> PortableHint {
    let hint = |kind: &str, value: &str| PortableHint {
        kind: kind.into(),
        value: value.into(),
    };
    if let Some(id) = launcher_id.strip_prefix("linux:app:") {
        return hint("desktop-id", id);
    }
    if let Some(rest) = launcher_id.strip_prefix("sys:") {
        if let Some((kind, value)) = rest.split_once(':') {
            return hint(kind, value);
        }
    }
    for origin in ["programs_user", "programs_common"] {
        if let Some(rel) = launcher_id.strip_prefix(&format!("win:{origin}:")) {
            return hint("start-menu", rel);
        }
    }
    hint("name", display_name)
}

/// Best-effort secret-shaped content scan (TEST_STRATEGY §2 export linter).
pub fn lint(text: &str, machine_bound: bool) -> Vec<String> {
    let mut findings = Vec::new();
    let lower = text.to_ascii_lowercase();
    for (needle, label) in [
        ("-----begin", "PEM block header"),
        ("private key", "private key reference"),
        ("ghp_", "GitHub token prefix"),
        ("github_pat_", "GitHub token prefix"),
        ("xoxb-", "Slack token prefix"),
        ("xoxp-", "Slack token prefix"),
        ("authorization:", "HTTP authorization header"),
        ("password", "password-like field"),
        ("passwd", "password-like field"),
        ("secret", "secret-like field"),
        ("api_key", "API key-like field"),
        ("apikey", "API key-like field"),
    ] {
        if lower.contains(needle) {
            findings.push(format!("contains a {label}"));
        }
    }
    let bytes = text.as_bytes();
    for window in bytes.windows(20) {
        if window.starts_with(b"AKIA")
            && window[4..]
                .iter()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            findings.push("contains an AWS access key id".into());
            break;
        }
    }
    let mut run = 0usize;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || "+/=_-".contains(c) {
            run += 1;
            if run == 48 {
                findings.push("contains a long token-like string".into());
                break;
            }
        } else {
            run = 0;
        }
    }
    if !machine_bound {
        for marker in ["/home/", "/users/", "c:\\users\\", "\\\\", "/root/"] {
            if lower.contains(marker) {
                findings.push("contains a machine-specific absolute path".into());
                break;
            }
        }
    }
    findings.sort();
    findings.dedup();
    findings
}

pub fn export(
    workspace: &Workspace,
    names: &dyn Fn(&str) -> Option<String>,
    machine_bound: bool,
) -> Result<String, Vec<String>> {
    let members = workspace
        .members
        .iter()
        .map(|m| ExportedMember {
            id: m.id.clone(),
            member_type: m.member_type,
            title: m.title.clone(),
            auto_restart: m.auto_restart,
            detach_expected: m.detach_expected,
            portable: m.portable.clone().unwrap_or_else(|| {
                portable_hint(
                    &m.launcher_id,
                    &names(&m.launcher_id).unwrap_or_else(|| m.launcher_id.clone()),
                )
            }),
            launcher_id: machine_bound.then(|| m.launcher_id.clone()),
        })
        .collect();
    let bundle = Bundle {
        format: FORMAT.into(),
        schema_version: EXPORT_SCHEMA,
        exported_at: now_ms(),
        machine_bound,
        workspace: ExportedWorkspace {
            name: workspace.name.clone(),
            startup_policy: workspace.startup_policy,
            layout: ExportedLayout {
                mode: workspace.layout.mode,
                json: workspace.layout.json.clone(),
                focused_member: workspace.layout.focused_member.clone(),
            },
            members,
        },
    };
    let text = serde_json::to_string_pretty(&bundle).map_err(|e| vec![e.to_string()])?;
    let findings = lint(&text, machine_bound);
    if findings.is_empty() {
        Ok(text)
    } else {
        Err(findings)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub workspace_id: String,
    pub members: usize,
    pub unresolved: Vec<String>,
    pub warnings: Vec<String>,
}

/// Resolution of a portable hint against this machine.
pub struct Resolver<'a> {
    pub exists: &'a dyn Fn(&str) -> bool,
}

pub struct Imported {
    pub workspace: Workspace,
    pub synthetic: Vec<SyntheticSpec>,
    pub report: ImportReport,
}

fn resolve(
    hint: &PortableHint,
    resolver: &Resolver<'_>,
) -> (Option<String>, Option<SyntheticSpec>) {
    let v = hint.value.as_str();
    let candidate = |id: String| ((resolver.exists)(&id)).then_some(id);
    match hint.kind.as_str() {
        "desktop-id" => (candidate(format!("linux:app:{v}")), None),
        "shell" => (candidate(format!("sys:shell:{v}")), None),
        "wsl" => (candidate(format!("sys:wsl:{v}")), None),
        "start-menu" => (
            candidate(format!("win:programs_user:{v}"))
                .or_else(|| candidate(format!("win:programs_common:{v}"))),
            None,
        ),
        "ssh" => {
            let spec = SyntheticSpec::Ssh { host: v.into() };
            (spec.validate().ok().map(|_| spec.launcher_id()), Some(spec))
        }
        "tmux" => {
            let spec = SyntheticSpec::Tmux { session: v.into() };
            (spec.validate().ok().map(|_| spec.launcher_id()), Some(spec))
        }
        "ssh-tmux" => match v.rsplit_once(':') {
            Some((host, session)) => {
                let spec = SyntheticSpec::SshTmux {
                    host: host.into(),
                    session: session.into(),
                };
                (spec.validate().ok().map(|_| spec.launcher_id()), Some(spec))
            }
            None => (None, None),
        },
        _ => (None, None),
    }
}

/// Parse and stage an import. Hostile input is rejected with reasons; a
/// valid bundle becomes a staged workspace whose members all need review.
pub fn import(text: &str, resolver: &Resolver<'_>) -> Result<Imported, String> {
    if text.len() > MAX_IMPORT_BYTES {
        return Err("import file is too large".into());
    }
    let bundle: Bundle =
        serde_json::from_str(text).map_err(|e| format!("not a ToolOnize workspace export: {e}"))?;
    if bundle.format != FORMAT {
        return Err("not a ToolOnize workspace export".into());
    }
    if bundle.schema_version != EXPORT_SCHEMA {
        return Err(format!(
            "unsupported export schema {} (this version reads {EXPORT_SCHEMA})",
            bundle.schema_version
        ));
    }
    let w = bundle.workspace;
    let name = validate_name(&w.name)?;
    if w.members.len() > MAX_MEMBERS {
        return Err(format!("too many members (max {MAX_MEMBERS})"));
    }
    let mut workspace = Workspace::new(
        new_id("ws"),
        format!("{name} (imported)").chars().take(80).collect(),
    );
    workspace.staged_import = true;
    // Defense in depth: imported workspaces never auto-start anything.
    workspace.startup_policy = StartupPolicy::Ask;
    let mut warnings = Vec::new();
    if w.startup_policy != StartupPolicy::Ask {
        warnings.push("startup policy reset to Ask for review".into());
    }
    let mut unresolved = Vec::new();
    let mut synthetic = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for m in w.members {
        if !valid_id(&m.id) || m.id.len() > 64 || !ids.insert(m.id.clone()) {
            return Err("member ids must be unique, short identifiers".into());
        }
        if m.portable.kind.len() > 32
            || m.portable.value.len() > 300
            || m.portable.value.chars().any(char::is_control)
        {
            return Err("member hint is malformed".into());
        }
        let title = validate_title(m.title)?;
        let (mut launcher_id, spec) = resolve(&m.portable, resolver);
        if launcher_id.is_none() && bundle.machine_bound {
            if let Some(id) = m
                .launcher_id
                .filter(|id| valid_id(id) && (resolver.exists)(id))
            {
                launcher_id = Some(id);
            }
        }
        if let Some(spec) = spec.filter(|s| s.validate().is_ok()) {
            synthetic.push(spec);
        }
        let launcher_id = launcher_id.unwrap_or_else(|| {
            unresolved.push(format!("{} ({})", m.portable.value, m.portable.kind));
            format!("import:{}:{}", m.portable.kind, m.portable.value)
        });
        workspace.members.push(Member {
            id: m.id,
            launcher_id,
            member_type: m.member_type,
            title,
            authorized_fingerprint: None,
            authorized_at: None,
            auto_restart: m.auto_restart,
            detach_expected: m.detach_expected,
            added_at: now_ms(),
            portable: Some(m.portable),
        });
    }
    let member_ids: Vec<String> = workspace.members.iter().map(|m| m.id.clone()).collect();
    let layout_json = match w.layout.json {
        Some(json) => match validate_layout(&json, &member_ids) {
            Ok(_) => Some(json),
            Err(error) => {
                warnings.push(format!("layout discarded: {error}"));
                None
            }
        },
        None => None,
    };
    workspace.layout = LayoutState {
        mode: w.layout.mode,
        json: layout_json,
        focused_member: w.layout.focused_member.filter(|f| member_ids.contains(f)),
    };
    let report = ImportReport {
        workspace_id: workspace.id.clone(),
        members: workspace.members.len(),
        unresolved,
        warnings,
    };
    Ok(Imported {
        workspace,
        synthetic,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn workspace() -> Workspace {
        let mut w = Workspace::new("ws_1".into(), "Project Phoenix".into());
        w.startup_policy = StartupPolicy::RestoreAll;
        for (id, launcher) in [
            ("m1", "linux:app:org.example.Editor.desktop"),
            ("m2", "sys:ssh:build.example.com"),
            ("m3", "linux:desktop:Org.Example.DeployHelper.desktop"),
        ] {
            w.members.push(Member {
                id: id.into(),
                launcher_id: launcher.into(),
                member_type: MemberType::Terminal,
                title: None,
                authorized_fingerprint: Some("fp".into()),
                authorized_at: Some(1),
                auto_restart: false,
                detach_expected: false,
                added_at: 1,
                portable: None,
            });
        }
        w.layout.json = Some(json!({"layout": {"type": "row", "children": [
            {"type": "tabset", "children": [
                {"type": "tab", "component": "member", "config": {"memberId": "m1"}}
            ]}
        ]}}));
        w
    }

    fn names(id: &str) -> Option<String> {
        (id == "linux:desktop:Org.Example.DeployHelper.desktop").then(|| "Deploy Helper".into())
    }

    #[test]
    fn export_is_portable_and_secret_free_t_persist_002() {
        let text = export(&workspace(), &names, false).unwrap();
        assert!(!text.contains("authorized_fingerprint"));
        assert!(
            !text.contains("linux:desktop"),
            "machine-bound identity omitted"
        );
        assert!(text.contains("\"desktop-id\""));
        assert!(text.contains("\"Deploy Helper\""));
        let bundle: Bundle = serde_json::from_str(&text).unwrap();
        assert!(bundle
            .workspace
            .members
            .iter()
            .all(|m| m.launcher_id.is_none()));
    }

    #[test]
    fn round_trip_lands_in_review_staging() {
        let text = export(&workspace(), &names, false).unwrap();
        let exists = |id: &str| id == "linux:app:org.example.Editor.desktop";
        let imported = import(&text, &Resolver { exists: &exists }).unwrap();
        let w = &imported.workspace;
        assert!(w.staged_import);
        assert_eq!(w.startup_policy, StartupPolicy::Ask);
        assert_eq!(w.members.len(), 3);
        assert!(
            w.members.iter().all(|m| m.authorized_fingerprint.is_none()),
            "nothing authorized"
        );
        assert_eq!(
            w.members[0].launcher_id,
            "linux:app:org.example.Editor.desktop"
        );
        assert_eq!(w.members[1].launcher_id, "sys:ssh:build.example.com");
        assert!(
            w.members[2].launcher_id.starts_with("import:"),
            "unresolved stays orphaned"
        );
        assert_eq!(imported.report.unresolved.len(), 1);
        assert!(w.layout.json.is_some());
    }

    #[test]
    fn hostile_imports_rejected_fs_06_t_persist_003() {
        let exists = |_: &str| true;
        let r = Resolver { exists: &exists };
        let ok = export(&workspace(), &names, false).unwrap();
        let mut v: Value = serde_json::from_str(&ok).unwrap();

        let mut extra = v.clone();
        extra["workspace"]["members"][0]["exec"] = json!("rm -rf ~");
        assert!(
            import(&extra.to_string(), &r).is_err(),
            "unknown fields rejected"
        );

        let mut traversal = v.clone();
        traversal["workspace"]["members"][0]["id"] = json!("../../etc");
        assert!(import(&traversal.to_string(), &r).is_err());

        let mut evil_ssh = v.clone();
        evil_ssh["workspace"]["members"][1]["portable"] =
            json!({"kind": "ssh", "value": "-oProxyCommand=evil"});
        let imported = import(&evil_ssh.to_string(), &r).unwrap();
        assert!(
            imported.synthetic.is_empty(),
            "invalid ssh host never becomes a launcher"
        );
        assert!(imported.workspace.members[1]
            .launcher_id
            .starts_with("import:"));

        let mut bad_layout = v.clone();
        bad_layout["workspace"]["layout"]["json"] = json!({"layout": {"type": "iframe"}});
        let imported = import(&bad_layout.to_string(), &r).unwrap();
        assert!(imported.workspace.layout.json.is_none());
        assert!(!imported.report.warnings.is_empty());

        v["format"] = json!("other");
        assert!(import(&v.to_string(), &r).is_err());
        assert!(import(&"[".repeat(10_000), &r).is_err(), "deep nesting");
        assert!(import(&"x".repeat(MAX_IMPORT_BYTES + 1), &r).is_err());
        assert!(import("not json", &r).is_err());
    }

    #[test]
    fn linter_catches_secret_shapes() {
        let pem_header = ["-----BEGIN OPENSSH ", "PRIV", "ATE KEY-----"].concat();
        assert!(!lint(&pem_header, false).is_empty());
        assert!(!lint("ghp_abcdefghijklmnopqrstuvwxyz", false).is_empty());
        assert!(!lint(&format!("AKIA{}", "ABCDEFGHIJKLMNOP"), false).is_empty());
        assert!(!lint("\"/home/alice/work\"", false).is_empty());
        assert!(lint("\"/home/alice/work\"", true).is_empty());
        assert!(lint("{\"name\": \"Project Phoenix\"}", false).is_empty());
    }
}
