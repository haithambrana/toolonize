//! Launcher Registry: normalized records + user decisions (FR-050).
//!
//! Identity (`launcher_id`) and authorization (`descriptor_fingerprint`) are
//! separate. Decisions are keyed by identity, so they survive descriptor
//! edits; review approvals record the exact fingerprint they approved, so
//! any security-relevant edit requires a new review (FR-015, T-DISC-04).

use crate::classify::{classify, effective_type};
use crate::discovery::SourceReport;
use crate::model::{
    descriptor_fingerprint, now_ms, Launcher, LauncherType, OriginKind, ParseStatus, RawLauncher,
};
use crate::shells::SyntheticSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const REMOVED_RETENTION_MS: u64 = 30 * 24 * 60 * 60 * 1000;
pub const MAX_ALIAS: usize = 120;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub alias: Option<String>,
    /// Fingerprint a human approved in review (only for launchers whose
    /// classification requires review).
    #[serde(default)]
    pub approved_fingerprint: Option<String>,
    #[serde(default)]
    pub approved_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Known {
    pub fingerprint: String,
    pub display_name: String,
    pub first_seen: u64,
    pub last_seen: u64,
    /// Fingerprint the user last acknowledged in the registry UI.
    #[serde(default)]
    pub acknowledged: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Removed {
    pub launcher_id: String,
    pub display_name: String,
    pub removed_at: u64,
}

/// Persisted registry state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryState {
    #[serde(default)]
    pub decisions: BTreeMap<String, Decision>,
    #[serde(default)]
    pub known: BTreeMap<String, Known>,
    #[serde(default)]
    pub removed: Vec<Removed>,
    #[serde(default)]
    pub custom_roots: Vec<String>,
    #[serde(default)]
    pub synthetic: Vec<SyntheticSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    New,
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegistryEntry {
    #[serde(flatten)]
    pub launcher: Launcher,
    pub decision: Decision,
    pub change: ChangeKind,
    pub effective_type: LauncherType,
    /// Requires human review of this exact descriptor before use.
    pub review_pending: bool,
    pub executable: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Diff {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }
}

pub fn normalize(raw: RawLauncher) -> Launcher {
    let classification = classify(&raw);
    let fingerprint = descriptor_fingerprint(raw.origin, &raw.launcher_id, &raw.descriptor);
    Launcher {
        origin_label: raw.origin.label().to_string(),
        launcher_id: raw.launcher_id,
        display_name: raw.display_name,
        platform: raw.platform,
        origin: raw.origin,
        origin_root: raw.origin_root,
        relative_path: raw.relative_path,
        descriptor: raw.descriptor,
        fingerprint,
        parse_status: raw.parse_status,
        issues: raw.issues,
        classification,
        no_display: raw.no_display,
        not_shown_here: raw.not_shown_here,
        comment: raw.comment,
    }
}

#[derive(Debug, Default, Clone)]
pub struct Registry {
    launchers: BTreeMap<String, Launcher>,
    pub reports: Vec<SourceReport>,
    pub last_scan_ms: u64,
}

impl Registry {
    pub fn get(&self, id: &str) -> Option<&Launcher> {
        self.launchers.get(id)
    }

    pub fn len(&self) -> usize {
        self.launchers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.launchers.is_empty()
    }

    /// Replace the record set with a fresh scan, updating persisted change
    /// tracking. Pure data: nothing executes (FR-004).
    pub fn apply_scan(
        &mut self,
        raws: Vec<RawLauncher>,
        reports: Vec<SourceReport>,
        state: &mut RegistryState,
    ) -> Diff {
        let now = now_ms();
        let mut fresh = BTreeMap::new();
        for raw in raws {
            let launcher = normalize(raw);
            // First record wins for a duplicate id (sources are precedence-ordered).
            fresh
                .entry(launcher.launcher_id.clone())
                .or_insert(launcher);
        }
        let mut diff = Diff::default();
        for (id, launcher) in &fresh {
            match state.known.get_mut(id) {
                Some(known) => {
                    if known.fingerprint != launcher.fingerprint {
                        diff.changed.push(id.clone());
                        known.fingerprint = launcher.fingerprint.clone();
                    }
                    known.display_name = launcher.display_name.clone();
                    known.last_seen = now;
                }
                None => {
                    diff.added.push(id.clone());
                    state.removed.retain(|r| &r.launcher_id != id);
                    state.known.insert(
                        id.clone(),
                        Known {
                            fingerprint: launcher.fingerprint.clone(),
                            display_name: launcher.display_name.clone(),
                            first_seen: now,
                            last_seen: now,
                            acknowledged: None,
                        },
                    );
                }
            }
        }
        let gone: Vec<String> = state
            .known
            .keys()
            .filter(|id| !fresh.contains_key(*id))
            .cloned()
            .collect();
        for id in gone {
            if let Some(known) = state.known.remove(&id) {
                state.removed.push(Removed {
                    launcher_id: id.clone(),
                    display_name: known.display_name,
                    removed_at: now,
                });
            }
            diff.removed.push(id);
        }
        state
            .removed
            .retain(|r| now.saturating_sub(r.removed_at) < REMOVED_RETENTION_MS);
        self.launchers = fresh;
        self.reports = reports;
        self.last_scan_ms = now;
        diff
    }

    pub fn entry(&self, launcher: &Launcher, state: &RegistryState) -> RegistryEntry {
        let decision = state
            .decisions
            .get(&launcher.launcher_id)
            .cloned()
            .unwrap_or_default();
        let effective = effective_type(
            &launcher.classification,
            launcher.parse_status,
            &launcher.fingerprint,
            decision.approved_fingerprint.as_deref(),
        );
        let change = match state.known.get(&launcher.launcher_id) {
            Some(k) if k.acknowledged.as_deref() == Some(launcher.fingerprint.as_str()) => {
                ChangeKind::Unchanged
            }
            Some(k) if k.acknowledged.is_some() => ChangeKind::Changed,
            _ if launcher.origin == OriginKind::Synthetic => ChangeKind::Unchanged,
            _ => ChangeKind::New,
        };
        let review_pending = launcher.classification.needs_review
            && !launcher.classification.blocked
            && launcher.parse_status == ParseStatus::Ok
            && decision.approved_fingerprint.as_deref() != Some(launcher.fingerprint.as_str());
        RegistryEntry {
            executable: effective != LauncherType::Unknown,
            launcher: launcher.clone(),
            decision,
            change,
            effective_type: effective,
            review_pending,
        }
    }

    pub fn entries(&self, state: &RegistryState) -> Vec<RegistryEntry> {
        self.launchers
            .values()
            .map(|l| self.entry(l, state))
            .collect()
    }

    pub fn effective(&self, id: &str, state: &RegistryState) -> Option<RegistryEntry> {
        self.launchers.get(id).map(|l| self.entry(l, state))
    }
}

/// Record a human review approval of the exact fingerprint shown.
pub fn approve(
    registry: &Registry,
    state: &mut RegistryState,
    id: &str,
    fingerprint: &str,
) -> Result<(), String> {
    let launcher = registry.get(id).ok_or("launcher not found")?;
    if launcher.fingerprint != fingerprint {
        return Err("the launcher changed since it was displayed; review it again".into());
    }
    if launcher.classification.blocked || launcher.parse_status != ParseStatus::Ok {
        return Err("this launcher is malformed or unavailable and cannot be approved".into());
    }
    if launcher.classification.proposed == LauncherType::Unknown {
        return Err("ToolOnize cannot tell how to run this launcher".into());
    }
    let decision = state.decisions.entry(id.to_string()).or_default();
    decision.approved_fingerprint = Some(fingerprint.to_string());
    decision.approved_at = Some(now_ms());
    Ok(())
}

pub fn set_alias(state: &mut RegistryState, id: &str, alias: Option<String>) -> Result<(), String> {
    let alias = alias
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty());
    if let Some(a) = &alias {
        if a.chars().count() > MAX_ALIAS || a.chars().any(char::is_control) {
            return Err("alias must be a short single line".into());
        }
    }
    state.decisions.entry(id.to_string()).or_default().alias = alias;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Descriptor, DescriptorKind, Platform};

    fn raw(id: &str, exec_args: &[&str], origin: OriginKind) -> RawLauncher {
        RawLauncher {
            launcher_id: id.into(),
            display_name: id.into(),
            platform: Platform::Linux,
            origin,
            origin_root: "/r".into(),
            relative_path: format!("{id}.desktop"),
            descriptor: Descriptor {
                kind: DescriptorKind::ExecTemplate,
                program: Some("htop".into()),
                args: exec_args.iter().map(|s| s.to_string()).collect(),
                raw_args: None,
                exec_template: None,
                working_dir: None,
                terminal: true,
                try_exec: None,
                icon: None,
                path_resolved: true,
                source_path: format!("/r/{id}.desktop"),
            },
            parse_status: ParseStatus::Ok,
            issues: vec![],
            no_display: false,
            not_shown_here: false,
            comment: None,
        }
    }

    #[test]
    fn diff_added_changed_removed() {
        let mut reg = Registry::default();
        let mut state = RegistryState::default();
        let d = reg.apply_scan(
            vec![
                raw("a", &[], OriginKind::XdgSystem),
                raw("b", &[], OriginKind::XdgSystem),
            ],
            vec![],
            &mut state,
        );
        assert_eq!(d.added, vec!["a", "b"]);
        let d = reg.apply_scan(
            vec![raw("a", &["-d"], OriginKind::XdgSystem)],
            vec![],
            &mut state,
        );
        assert_eq!(d.changed, vec!["a"]);
        assert_eq!(d.removed, vec!["b"]);
        assert_eq!(state.removed.len(), 1);
        let d = reg.apply_scan(
            vec![
                raw("a", &["-d"], OriginKind::XdgSystem),
                raw("b", &[], OriginKind::XdgSystem),
            ],
            vec![],
            &mut state,
        );
        assert_eq!(d.added, vec!["b"]);
        assert!(state.removed.is_empty());
    }

    #[test]
    fn exec_edit_keeps_identity_and_invalidates_approval_t_policy_002() {
        let mut reg = Registry::default();
        let mut state = RegistryState::default();
        reg.apply_scan(
            vec![raw("d", &[], OriginKind::DesktopDir)],
            vec![],
            &mut state,
        );
        let fp1 = reg.get("d").unwrap().fingerprint.clone();
        assert_eq!(
            reg.effective("d", &state).unwrap().effective_type,
            LauncherType::Unknown
        );
        approve(&reg, &mut state, "d", &fp1).unwrap();
        assert_eq!(
            reg.effective("d", &state).unwrap().effective_type,
            LauncherType::LocalCommand
        );
        state.decisions.get_mut("d").unwrap().alias = Some("Mine".into());

        reg.apply_scan(
            vec![raw("d", &["--evil"], OriginKind::DesktopDir)],
            vec![],
            &mut state,
        );
        let entry = reg.effective("d", &state).unwrap();
        assert_eq!(entry.launcher.launcher_id, "d", "identity stable");
        assert_ne!(entry.launcher.fingerprint, fp1);
        assert_eq!(
            entry.effective_type,
            LauncherType::Unknown,
            "approval suspended"
        );
        assert!(entry.review_pending);
        assert_eq!(
            entry.decision.alias.as_deref(),
            Some("Mine"),
            "decisions preserved"
        );
    }

    #[test]
    fn approval_requires_current_fingerprint() {
        let mut reg = Registry::default();
        let mut state = RegistryState::default();
        reg.apply_scan(
            vec![raw("d", &[], OriginKind::DesktopDir)],
            vec![],
            &mut state,
        );
        assert!(approve(&reg, &mut state, "d", "stale").is_err());
        assert!(approve(&reg, &mut state, "missing", "x").is_err());
    }

    #[test]
    fn acknowledged_change_tracking() {
        let mut reg = Registry::default();
        let mut state = RegistryState::default();
        reg.apply_scan(
            vec![raw("a", &[], OriginKind::XdgSystem)],
            vec![],
            &mut state,
        );
        assert_eq!(reg.effective("a", &state).unwrap().change, ChangeKind::New);
        let fp = reg.get("a").unwrap().fingerprint.clone();
        state.known.get_mut("a").unwrap().acknowledged = Some(fp);
        assert_eq!(
            reg.effective("a", &state).unwrap().change,
            ChangeKind::Unchanged
        );
        reg.apply_scan(
            vec![raw("a", &["x"], OriginKind::XdgSystem)],
            vec![],
            &mut state,
        );
        assert_eq!(
            reg.effective("a", &state).unwrap().change,
            ChangeKind::Changed
        );
    }

    #[test]
    fn alias_validation() {
        let mut state = RegistryState::default();
        assert!(set_alias(&mut state, "a", Some("x\ny".into())).is_err());
        set_alias(&mut state, "a", Some("  Name ".into())).unwrap();
        assert_eq!(state.decisions["a"].alias.as_deref(), Some("Name"));
        set_alias(&mut state, "a", Some("   ".into())).unwrap();
        assert_eq!(state.decisions["a"].alias, None);
    }
}
