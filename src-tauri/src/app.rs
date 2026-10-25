//! Application core: owns all state and is the only place that mutates it.
//!
//! Every mutation runs against a copy of the persisted state, is saved
//! atomically, and only then becomes visible (transactional commands; a
//! failed save changes nothing). IPC commands are thin wrappers over these
//! methods.

use crate::audit::{Audit, AuditEntry, Outcome, Record};
use crate::discovery::{self, watch, DiscoveryConfig, SourceReport};
use crate::external;
use crate::model::{new_id, now_ms, valid_id, LauncherType};
use crate::persist::{LoadReport, Store};
use crate::policy::{self, LaunchMode, LaunchPlan, MemberStatus};
use crate::pty::{PortableBackend, PtyDims};
use crate::registry::{self, Diff, Registry, RegistryEntry, RegistryState};
use crate::session::{SessionInfo, SessionManager, SessionPlan, SessionSink};
use crate::settings::{Settings, SETTINGS_SCHEMA};
use crate::shells::{self, SyntheticSpec};
use crate::transfer::{self, ImportReport};
use crate::workspace::{
    validate_layout, validate_name, validate_title, LayoutMode, LayoutState, Member, MemberType,
    StartupPolicy, Workspace, MAX_MEMBERS, MAX_WORKSPACES,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

pub const STATE_SCHEMA: u32 = 1;
const STATE_FORMAT: &str = "toolonize-state";
const SETTINGS_FORMAT: &str = "toolonize-settings";
const MAX_CUSTOM_ROOTS: usize = 16;
const MAX_SYNTHETIC: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    pub registry: RegistryState,
    #[serde(default)]
    pub active_workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    Session { info: SessionInfo },
    SessionClosed { session_id: String },
    Registry { diff: Diff },
    Workspaces,
    Settings,
    Watch { status: watch::WatchStatus },
    Notice { level: String, message: String },
}

pub trait EventSink: Send + Sync {
    fn send(&self, event: &AppEvent) -> bool;
}

#[derive(Default)]
struct Bus {
    sinks: Mutex<Vec<Arc<dyn EventSink>>>,
}

impl Bus {
    fn emit(&self, event: AppEvent) {
        let sinks: Vec<_> = self.sinks.lock().unwrap().clone();
        let mut dead = Vec::new();
        for (index, sink) in sinks.iter().enumerate() {
            if !sink.send(&event) {
                dead.push(index);
            }
        }
        if !dead.is_empty() {
            let mut sinks = self.sinks.lock().unwrap();
            let mut i = 0;
            sinks.retain(|_| {
                let keep = !dead.contains(&i);
                i += 1;
                keep
            });
        }
    }
}

struct Data {
    state: PersistedState,
    registry: Registry,
    settings: Settings,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberView {
    #[serde(flatten)]
    pub member: Member,
    pub status: MemberStatus,
    pub effective_type: Option<LauncherType>,
    pub origin_label: Option<String>,
    pub interpreter: bool,
    pub review_pending: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceView {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub startup_policy: StartupPolicy,
    pub layout: LayoutState,
    pub staged_import: bool,
    pub members: Vec<MemberView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegistryView {
    pub entries: Vec<RegistryEntry>,
    pub reports: Vec<SourceReport>,
    pub last_scan_ms: u64,
    pub custom_roots: Vec<String>,
    pub synthetic: Vec<SyntheticSpec>,
    pub removed: Vec<registry::Removed>,
    pub watch: watch::WatchStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub struct OpenChoice {
    pub start_terminals: bool,
    pub launch_external: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberOutcome {
    pub member_id: String,
    pub outcome: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenResult {
    pub needs_choice: bool,
    pub outcomes: Vec<MemberOutcome>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartupInfo {
    pub load_report: LoadReport,
    pub platform: String,
    pub version: String,
    pub backend: String,
    pub state_dir: String,
    pub config_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemberPatch {
    pub title: Option<String>,
    pub auto_restart: Option<bool>,
    pub detach_expected: Option<bool>,
}

pub struct Core {
    data: Mutex<Data>,
    store: Store,
    config_store: Store,
    pub sessions: SessionManager,
    audit: Audit,
    bus: Bus,
    watch: Mutex<Option<watch::WatchHandle>>,
    scan_lock: Mutex<()>,
    load_report: LoadReport,
    restarts: Mutex<HashMap<String, Vec<Instant>>>,
    /// Workspaces whose startup policy already ran in this app process; a
    /// renderer reload must not run it (or ask) again.
    opened: Mutex<std::collections::HashSet<String>>,
    me: OnceLock<Weak<Core>>,
}

fn check_id(id: &str) -> Result<(), String> {
    if valid_id(id) {
        Ok(())
    } else {
        Err("invalid identifier".into())
    }
}

impl Core {
    pub fn new(state_dir: PathBuf, config_dir: PathBuf) -> Arc<Core> {
        let store = Store::new(state_dir.clone());
        let config_store = Store::new(config_dir);
        let (state, load_report) =
            store.load::<PersistedState>("state", STATE_FORMAT, STATE_SCHEMA, &[]);
        let (settings, _) =
            config_store.load::<Settings>("settings", SETTINGS_FORMAT, SETTINGS_SCHEMA, &[]);
        let settings = settings
            .filter(|s| s.validate().is_ok())
            .unwrap_or_default();
        let replay = settings.replay_buffer_mib as usize * 1024 * 1024;
        Arc::new_cyclic(|weak: &Weak<Core>| {
            let listener_core = weak.clone();
            let sessions = SessionManager::new(
                Arc::new(PortableBackend::new()),
                Arc::new(move |info: &SessionInfo| {
                    if let Some(core) = listener_core.upgrade() {
                        core.on_session(info);
                    }
                }),
            );
            sessions.set_retention(replay);
            let me = OnceLock::new();
            let _ = me.set(weak.clone());
            Core {
                data: Mutex::new(Data {
                    state: state.unwrap_or_default(),
                    registry: Registry::default(),
                    settings,
                }),
                store,
                config_store: config_store.clone(),
                sessions,
                audit: Audit::new(Some(state_dir.join("audit.log"))),
                bus: Bus::default(),
                watch: Mutex::new(None),
                scan_lock: Mutex::new(()),
                load_report,
                restarts: Mutex::new(HashMap::new()),
                opened: Mutex::new(std::collections::HashSet::new()),
                me,
            }
        })
    }

    fn arc(&self) -> Option<Arc<Core>> {
        self.me.get().and_then(Weak::upgrade)
    }

    pub fn startup_info(&self) -> StartupInfo {
        StartupInfo {
            load_report: self.load_report.clone(),
            platform: std::env::consts::OS.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            backend: "portable-pty-0.9.0".to_string(),
            state_dir: self.store.dir().to_string_lossy().into_owned(),
            config_dir: self.config_store.dir().to_string_lossy().into_owned(),
        }
    }

    /// Register the event stream of the (single) main window. A reloaded
    /// page subscribes again, replacing the stream of the page it replaced.
    pub fn subscribe(&self, sink: Arc<dyn EventSink>) {
        *self.bus.sinks.lock().unwrap() = vec![sink];
    }

    fn notice(&self, level: &str, message: impl Into<String>) {
        self.bus.emit(AppEvent::Notice {
            level: level.into(),
            message: message.into(),
        });
    }

    /// Apply `f` to a copy of the persisted state, save, then commit.
    fn mutate<R>(
        &self,
        f: impl FnOnce(&mut PersistedState, &Registry) -> Result<R, String>,
    ) -> Result<R, String> {
        let mut data = self.data.lock().unwrap();
        let mut next = data.state.clone();
        let result = f(&mut next, &data.registry)?;
        if next != data.state {
            self.store
                .save("state", STATE_FORMAT, STATE_SCHEMA, &next)?;
            data.state = next;
        }
        Ok(result)
    }

    // ---- discovery / registry -------------------------------------------

    fn discovery_config(&self) -> DiscoveryConfig {
        let data = self.data.lock().unwrap();
        DiscoveryConfig {
            custom_roots: data
                .state
                .registry
                .custom_roots
                .iter()
                .map(PathBuf::from)
                .collect(),
        }
    }

    /// Full rescan of every source plus synthetic launchers. Read-only.
    pub fn rescan(&self) -> Result<Diff, String> {
        let _guard = self.scan_lock.lock().unwrap();
        let config = self.discovery_config();
        let mut output = discovery::scan_all(&config);
        output.launchers.extend(shells::system_shells());
        let synthetic = self.data.lock().unwrap().state.registry.synthetic.clone();
        output
            .launchers
            .extend(synthetic.iter().map(shells::synthetic_launcher));
        let diff = {
            let mut data = self.data.lock().unwrap();
            let mut next = data.state.registry.clone();
            let mut registry = data.registry.clone();
            let diff = registry.apply_scan(output.launchers, output.reports, &mut next);
            if next != data.state.registry {
                let mut state = data.state.clone();
                state.registry = next;
                // Registry tracking is best-effort: a failed save must not
                // hide what is on disk now.
                if self
                    .store
                    .save("state", STATE_FORMAT, STATE_SCHEMA, &state)
                    .is_ok()
                {
                    data.state = state;
                }
            }
            data.registry = registry;
            diff
        };
        self.bus.emit(AppEvent::Registry { diff: diff.clone() });
        self.bus.emit(AppEvent::Workspaces);
        Ok(diff)
    }

    /// (Re)start watching launcher roots according to settings.
    pub fn restart_watch(&self) {
        let enabled = self.data.lock().unwrap().settings.live_updates;
        let mut slot = self.watch.lock().unwrap();
        *slot = None;
        if !enabled {
            drop(slot);
            self.bus.emit(AppEvent::Watch {
                status: watch::WatchStatus::Off,
            });
            return;
        }
        let roots = discovery::watch_roots(&self.discovery_config());
        let weak = self.me.get().cloned();
        let handle = watch::start(
            roots,
            Arc::new(move || {
                if let Some(core) = weak.as_ref().and_then(Weak::upgrade) {
                    let _ = core.rescan();
                }
            }),
        );
        let status = handle.status();
        *slot = Some(handle);
        drop(slot);
        self.bus.emit(AppEvent::Watch { status });
    }

    pub fn watch_status(&self) -> watch::WatchStatus {
        self.watch
            .lock()
            .unwrap()
            .as_ref()
            .map(|w| w.status())
            .unwrap_or(watch::WatchStatus::Off)
    }

    pub fn registry_view(&self) -> RegistryView {
        let data = self.data.lock().unwrap();
        RegistryView {
            entries: data.registry.entries(&data.state.registry),
            reports: data.registry.reports.clone(),
            last_scan_ms: data.registry.last_scan_ms,
            custom_roots: data.state.registry.custom_roots.clone(),
            synthetic: data.state.registry.synthetic.clone(),
            removed: data.state.registry.removed.clone(),
            watch: self.watch_status(),
        }
    }

    fn registry_changed(&self) {
        self.bus.emit(AppEvent::Registry {
            diff: Diff::default(),
        });
        self.bus.emit(AppEvent::Workspaces);
    }

    pub fn set_hidden(&self, id: &str, hidden: bool) -> Result<(), String> {
        check_id(id)?;
        self.mutate(|s, _| {
            s.registry
                .decisions
                .entry(id.to_string())
                .or_default()
                .hidden = hidden;
            Ok(())
        })?;
        self.registry_changed();
        Ok(())
    }

    pub fn set_alias(&self, id: &str, alias: Option<String>) -> Result<(), String> {
        check_id(id)?;
        self.mutate(|s, _| registry::set_alias(&mut s.registry, id, alias))?;
        self.registry_changed();
        Ok(())
    }

    pub fn approve(&self, id: &str, fingerprint: &str) -> Result<(), String> {
        check_id(id)?;
        let result = self.mutate(|s, reg| registry::approve(reg, &mut s.registry, id, fingerprint));
        self.audit.record(
            "registry.approve",
            if result.is_ok() {
                Outcome::Recorded
            } else {
                Outcome::Denied
            },
            Record {
                launcher_id: Some(id),
                fingerprint: Some(fingerprint),
                detail: result.as_ref().err().map(String::as_str),
                ..Default::default()
            },
        );
        result?;
        self.registry_changed();
        Ok(())
    }

    pub fn revoke(&self, id: &str) -> Result<(), String> {
        check_id(id)?;
        self.mutate(|s, _| {
            if let Some(d) = s.registry.decisions.get_mut(id) {
                d.approved_fingerprint = None;
                d.approved_at = None;
            }
            Ok(())
        })?;
        self.audit.record(
            "registry.revoke",
            Outcome::Recorded,
            Record {
                launcher_id: Some(id),
                ..Default::default()
            },
        );
        self.registry_changed();
        Ok(())
    }

    pub fn acknowledge(&self, ids: &[String]) -> Result<(), String> {
        if ids.len() > 10_000 {
            return Err("too many ids".into());
        }
        self.mutate(|s, reg| {
            for id in ids {
                if let (Some(known), Some(l)) = (s.registry.known.get_mut(id), reg.get(id)) {
                    known.acknowledged = Some(l.fingerprint.clone());
                }
            }
            Ok(())
        })?;
        self.registry_changed();
        Ok(())
    }

    pub fn add_custom_root(&self, path: &str) -> Result<(), String> {
        let canonical = discovery::validate_custom_root(path)?;
        let text = canonical.to_string_lossy().into_owned();
        self.mutate(|s, _| {
            if s.registry.custom_roots.contains(&text) {
                return Ok(());
            }
            if s.registry.custom_roots.len() >= MAX_CUSTOM_ROOTS {
                return Err(format!("at most {MAX_CUSTOM_ROOTS} custom roots"));
            }
            s.registry.custom_roots.push(text.clone());
            Ok(())
        })?;
        self.rescan()?;
        self.restart_watch();
        Ok(())
    }

    pub fn remove_custom_root(&self, path: &str) -> Result<(), String> {
        self.mutate(|s, _| {
            s.registry.custom_roots.retain(|r| r != path);
            Ok(())
        })?;
        self.rescan()?;
        self.restart_watch();
        Ok(())
    }

    pub fn add_synthetic(&self, spec: SyntheticSpec) -> Result<String, String> {
        spec.validate()?;
        let id = spec.launcher_id();
        self.mutate(|s, _| {
            if !s.registry.synthetic.contains(&spec) {
                if s.registry.synthetic.len() >= MAX_SYNTHETIC {
                    return Err("too many custom connections".into());
                }
                s.registry.synthetic.push(spec.clone());
            }
            Ok(())
        })?;
        self.rescan()?;
        Ok(id)
    }

    pub fn remove_synthetic(&self, launcher_id: &str) -> Result<(), String> {
        self.mutate(|s, _| {
            s.registry
                .synthetic
                .retain(|x| x.launcher_id() != launcher_id);
            Ok(())
        })?;
        self.rescan()?;
        Ok(())
    }

    // ---- workspaces ------------------------------------------------------

    fn view(&self, w: &Workspace, data: &Data) -> WorkspaceView {
        let members = w
            .members
            .iter()
            .map(|m| {
                let entry = data
                    .registry
                    .effective(&m.launcher_id, &data.state.registry);
                MemberView {
                    member: m.clone(),
                    status: policy::member_status(m, &data.registry, &data.state.registry),
                    effective_type: entry.as_ref().map(|e| e.effective_type),
                    origin_label: entry.as_ref().map(|e| e.launcher.origin_label.clone()),
                    interpreter: entry
                        .as_ref()
                        .map(|e| e.launcher.classification.interpreter)
                        .unwrap_or(false),
                    review_pending: entry.as_ref().map(|e| e.review_pending).unwrap_or(false),
                }
            })
            .collect();
        WorkspaceView {
            id: w.id.clone(),
            name: w.name.clone(),
            icon: w.icon.clone(),
            created_at: w.created_at,
            updated_at: w.updated_at,
            startup_policy: w.startup_policy,
            layout: w.layout.clone(),
            staged_import: w.staged_import,
            members,
        }
    }

    pub fn workspaces(&self) -> (Vec<WorkspaceView>, Option<String>) {
        let data = self.data.lock().unwrap();
        (
            data.state
                .workspaces
                .iter()
                .map(|w| self.view(w, &data))
                .collect(),
            data.state.active_workspace.clone(),
        )
    }

    fn changed(&self) {
        self.bus.emit(AppEvent::Workspaces);
    }

    pub fn create_workspace(&self, name: &str) -> Result<String, String> {
        let name = validate_name(name)?;
        let id = new_id("ws");
        self.mutate(|s, _| {
            if s.workspaces.len() >= MAX_WORKSPACES {
                return Err(format!("at most {MAX_WORKSPACES} workspaces"));
            }
            s.workspaces.push(Workspace::new(id.clone(), name));
            s.active_workspace = Some(id.clone());
            Ok(())
        })?;
        self.changed();
        Ok(id)
    }

    fn with_workspace<R>(
        &self,
        id: &str,
        f: impl FnOnce(&mut Workspace, &Registry, &RegistryState) -> Result<R, String>,
    ) -> Result<R, String> {
        check_id(id)?;
        let result = self.mutate(|s, reg| {
            let registry_state = s.registry.clone();
            let w = s
                .workspaces
                .iter_mut()
                .find(|w| w.id == id)
                .ok_or("workspace not found")?;
            let r = f(w, reg, &registry_state)?;
            w.touch();
            Ok(r)
        })?;
        self.changed();
        Ok(result)
    }

    pub fn rename_workspace(&self, id: &str, name: &str) -> Result<(), String> {
        let name = validate_name(name)?;
        self.with_workspace(id, |w, _, _| {
            w.name = name;
            Ok(())
        })
    }

    pub fn delete_workspace(&self, id: &str) -> Result<(), String> {
        check_id(id)?;
        self.mutate(|s, _| {
            let before = s.workspaces.len();
            s.workspaces.retain(|w| w.id != id);
            if s.workspaces.len() == before {
                return Err("workspace not found".into());
            }
            if s.active_workspace.as_deref() == Some(id) {
                s.active_workspace = s.workspaces.first().map(|w| w.id.clone());
            }
            Ok(())
        })?;
        self.sessions.close_workspace(id);
        self.changed();
        Ok(())
    }

    pub fn set_active(&self, id: Option<&str>) -> Result<(), String> {
        if let Some(id) = id {
            check_id(id)?;
        }
        self.mutate(|s, _| {
            if let Some(id) = id {
                if !s.workspaces.iter().any(|w| w.id == id) {
                    return Err("workspace not found".into());
                }
            }
            s.active_workspace = id.map(str::to_string);
            Ok(())
        })?;
        self.changed();
        Ok(())
    }

    pub fn set_policy(&self, id: &str, policy: StartupPolicy) -> Result<(), String> {
        self.with_workspace(id, |w, _, _| {
            w.startup_policy = policy;
            Ok(())
        })
    }

    pub fn accept_import(&self, id: &str) -> Result<(), String> {
        self.with_workspace(id, |w, _, _| {
            w.staged_import = false;
            Ok(())
        })
    }

    /// Pin a launcher into a workspace: the explicit authorization of its
    /// current fingerprint for this member (FR-013, FR-015).
    pub fn add_member(
        &self,
        workspace_id: &str,
        launcher_id: &str,
        fingerprint: &str,
        title: Option<String>,
    ) -> Result<String, String> {
        check_id(launcher_id)?;
        let title = validate_title(title)?;
        let member_id = new_id("m");
        let result = self.with_workspace(workspace_id, |w, reg, rs| {
            if w.members.len() >= MAX_MEMBERS {
                return Err(format!("at most {MAX_MEMBERS} members per workspace"));
            }
            let entry = reg.effective(launcher_id, rs).ok_or("launcher not found")?;
            if entry.launcher.fingerprint != fingerprint {
                return Err("the launcher changed since it was displayed; review it again".into());
            }
            let member_type =
                MemberType::from_launcher(entry.effective_type).ok_or(if entry.review_pending {
                    "review this launcher before adding it"
                } else {
                    "ToolOnize cannot run this launcher"
                })?;
            let program = entry
                .launcher
                .descriptor
                .program
                .clone()
                .unwrap_or_default();
            let detach_expected = matches!(
                crate::classify::program_name(&program).as_str(),
                "tmux" | "screen" | "zellij" | "byobu"
            ) || entry.launcher.descriptor.args.iter().any(|a| a == "tmux");
            w.members.push(Member {
                id: member_id.clone(),
                launcher_id: launcher_id.to_string(),
                member_type,
                title,
                authorized_fingerprint: Some(fingerprint.to_string()),
                authorized_at: Some(now_ms()),
                auto_restart: false,
                detach_expected,
                added_at: now_ms(),
                portable: None,
            });
            Ok(())
        });
        self.audit.record(
            "member.authorize",
            if result.is_ok() {
                Outcome::Recorded
            } else {
                Outcome::Denied
            },
            Record {
                workspace_id: Some(workspace_id),
                member_id: Some(&member_id),
                launcher_id: Some(launcher_id),
                fingerprint: Some(fingerprint),
                detail: result.as_ref().err().map(String::as_str),
            },
        );
        result.map(|_| member_id)
    }

    /// Re-authorize a member after review (changed or imported members).
    pub fn authorize_member(
        &self,
        workspace_id: &str,
        member_id: &str,
        fingerprint: &str,
    ) -> Result<(), String> {
        let result = self.with_workspace(workspace_id, |w, reg, rs| {
            let member = w.member_mut(member_id).ok_or("member not found")?;
            let entry = reg
                .effective(&member.launcher_id, rs)
                .ok_or("the launcher is not present on this machine")?;
            if entry.launcher.fingerprint != fingerprint {
                return Err("the launcher changed since it was displayed; review it again".into());
            }
            let member_type =
                MemberType::from_launcher(entry.effective_type).ok_or(if entry.review_pending {
                    "review this launcher in the Launchers view first"
                } else {
                    "ToolOnize cannot run this launcher"
                })?;
            member.member_type = member_type;
            member.authorized_fingerprint = Some(fingerprint.to_string());
            member.authorized_at = Some(now_ms());
            Ok(member.launcher_id.clone())
        });
        self.audit.record(
            "member.reauthorize",
            if result.is_ok() {
                Outcome::Recorded
            } else {
                Outcome::Denied
            },
            Record {
                workspace_id: Some(workspace_id),
                member_id: Some(member_id),
                launcher_id: result.as_ref().ok().map(String::as_str),
                fingerprint: Some(fingerprint),
                detail: result.as_ref().err().map(String::as_str),
            },
        );
        result.map(|_| ())
    }

    pub fn remove_member(&self, workspace_id: &str, member_id: &str) -> Result<(), String> {
        self.with_workspace(workspace_id, |w, _, _| {
            let before = w.members.len();
            w.members.retain(|m| m.id != member_id);
            if before == w.members.len() {
                return Err("member not found".into());
            }
            if w.layout.focused_member.as_deref() == Some(member_id) {
                w.layout.focused_member = None;
            }
            Ok(())
        })?;
        if let Some(info) = self.sessions.find_by_member(workspace_id, member_id) {
            let _ = self.sessions.close(&info.session_id);
        }
        Ok(())
    }

    pub fn update_member(
        &self,
        workspace_id: &str,
        member_id: &str,
        patch: MemberPatch,
    ) -> Result<(), String> {
        let title = match patch.title {
            Some(t) => Some(validate_title(Some(t))?),
            None => None,
        };
        let renamed = title.is_some();
        let shown = self.with_workspace(workspace_id, |w, reg, rs| {
            let m = w.member_mut(member_id).ok_or("member not found")?;
            if let Some(t) = title {
                m.title = t;
            }
            if let Some(v) = patch.auto_restart {
                m.auto_restart = v;
            }
            if let Some(v) = patch.detach_expected {
                m.detach_expected = v;
            }
            // Title a running session should show: the member title, or the
            // launcher's alias/name when the title was reset.
            let launcher = reg.effective(&m.launcher_id, rs);
            Ok(m.title.clone().unwrap_or_else(|| {
                launcher
                    .map(|e| e.decision.alias.unwrap_or(e.launcher.display_name))
                    .unwrap_or_else(|| m.launcher_id.clone())
            }))
        })?;
        if renamed {
            if let Some(info) = self.sessions.find_by_member(workspace_id, member_id) {
                let _ = self.sessions.rename(&info.session_id, &shown);
            }
        }
        Ok(())
    }

    pub fn save_layout(
        &self,
        workspace_id: &str,
        mode: LayoutMode,
        json: Option<Value>,
        focused_member: Option<String>,
    ) -> Result<(), String> {
        self.with_workspace(workspace_id, |w, _, _| {
            let ids: Vec<String> = w.members.iter().map(|m| m.id.clone()).collect();
            if let Some(json) = &json {
                validate_layout(json, &ids)?;
            }
            if let Some(f) = &focused_member {
                if !ids.contains(f) {
                    return Err("focused member is not part of the workspace".into());
                }
            }
            w.layout = LayoutState {
                mode,
                json,
                focused_member,
            };
            Ok(())
        })
    }

    // ---- execution -------------------------------------------------------

    fn plan(&self, workspace_id: &str, member_id: &str) -> Result<LaunchPlan, String> {
        check_id(workspace_id)?;
        check_id(member_id)?;
        let data = self.data.lock().unwrap();
        let w = data
            .state
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .ok_or("workspace not found")?;
        policy::plan_start(w, member_id, &data.registry, &data.state.registry)
    }

    fn audit_plan(
        &self,
        action: &str,
        ws: &str,
        member: &str,
        result: &Result<LaunchPlan, String>,
    ) {
        let (outcome, launcher, fp, detail) = match result {
            Ok(p) => (
                Outcome::Allowed,
                Some(p.launcher_id.as_str()),
                Some(p.fingerprint.as_str()),
                p.interpreter
                    .then_some("interpreter execution (authorized)"),
            ),
            Err(e) => (Outcome::Denied, None, None, Some(e.as_str())),
        };
        self.audit.record(
            action,
            outcome,
            Record {
                workspace_id: Some(ws),
                member_id: Some(member),
                launcher_id: launcher,
                fingerprint: fp,
                detail,
            },
        );
    }

    pub fn start_session(
        &self,
        workspace_id: &str,
        member_id: &str,
        rows: u16,
        cols: u16,
    ) -> Result<SessionInfo, String> {
        let result = self.plan(workspace_id, member_id).and_then(|p| {
            if p.mode == LaunchMode::Pty {
                Ok(p)
            } else {
                Err("external applications are launched, not embedded".into())
            }
        });
        self.audit_plan("session.start", workspace_id, member_id, &result);
        let plan = result?;
        self.sessions
            .start(SessionPlan {
                workspace_id: workspace_id.to_string(),
                member_id: member_id.to_string(),
                title: plan.title,
                spec: plan.spec,
                remote: plan.remote,
                detach_expected: plan.detach_expected,
                dims: PtyDims::clamped(rows, cols),
            })
            .map_err(|e| e.to_string())
    }

    fn replan_session(
        &self,
        session_id: &str,
        action: &str,
    ) -> Result<(SessionInfo, LaunchPlan), String> {
        let info = self.sessions.info(session_id).map_err(|e| e.to_string())?;
        let result = self.plan(&info.workspace_id, &info.member_id);
        self.audit_plan(action, &info.workspace_id, &info.member_id, &result);
        Ok((info, result?))
    }

    pub fn restart_session(&self, session_id: &str) -> Result<SessionInfo, String> {
        let (_, plan) = self.replan_session(session_id, "session.restart")?;
        self.sessions
            .restart(session_id, plan.spec)
            .map_err(|e| e.to_string())
    }

    pub fn reconnect_session(&self, session_id: &str) -> Result<SessionInfo, String> {
        let (_, plan) = self.replan_session(session_id, "session.reconnect")?;
        self.sessions
            .reconnect(session_id, plan.spec)
            .map_err(|e| e.to_string())
    }

    pub fn attach(
        &self,
        session_id: &str,
        from_offset: u64,
        sink: Arc<dyn SessionSink>,
    ) -> Result<crate::session::AttachInfo, String> {
        self.sessions
            .attach(session_id, from_offset, sink)
            .map_err(|e| e.to_string())
    }

    pub fn launch_external(&self, workspace_id: &str, member_id: &str) -> Result<u32, String> {
        let result = self.plan(workspace_id, member_id).and_then(|p| {
            if p.mode == LaunchMode::External {
                Ok(p)
            } else {
                Err("terminal members run embedded".into())
            }
        });
        self.audit_plan("external.launch", workspace_id, member_id, &result);
        let plan = result?;
        match external::launch_detached(&plan) {
            Ok(pid) => {
                self.notice("info", format!("Launched {}", plan.title));
                Ok(pid)
            }
            Err(error) => {
                self.audit.record(
                    "external.launch",
                    Outcome::Failed,
                    Record {
                        workspace_id: Some(workspace_id),
                        member_id: Some(member_id),
                        launcher_id: Some(&plan.launcher_id),
                        detail: Some(&error),
                        ..Default::default()
                    },
                );
                Err(error)
            }
        }
    }

    /// Startup policy executor (FR-024). Failures isolate per member.
    pub fn open_workspace(
        &self,
        workspace_id: &str,
        choice: Option<OpenChoice>,
    ) -> Result<OpenResult, String> {
        check_id(workspace_id)?;
        let (policy, members, staged) = {
            let data = self.data.lock().unwrap();
            let w = data
                .state
                .workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .ok_or("workspace not found")?;
            (
                w.startup_policy,
                w.members
                    .iter()
                    .map(|m| (m.id.clone(), m.member_type))
                    .collect::<Vec<_>>(),
                w.staged_import,
            )
        };
        self.set_active(Some(workspace_id))?;
        let first_open = self.opened.lock().unwrap().insert(workspace_id.to_string());
        if choice.is_none() && !first_open {
            return Ok(OpenResult {
                needs_choice: false,
                outcomes: vec![],
            });
        }
        let choice = match (policy, choice) {
            (_, Some(c)) => c,
            (StartupPolicy::Ask, None) if !members.is_empty() && !staged => {
                return Ok(OpenResult {
                    needs_choice: true,
                    outcomes: vec![],
                })
            }
            (StartupPolicy::Ask, None) | (StartupPolicy::RestoreOnly, None) => {
                OpenChoice::default()
            }
            (StartupPolicy::RestoreTerminals, None) => OpenChoice {
                start_terminals: true,
                launch_external: false,
            },
            (StartupPolicy::RestoreAll, None) => OpenChoice {
                start_terminals: true,
                launch_external: true,
            },
        };
        let mut outcomes = Vec::new();
        for (member_id, member_type) in members {
            let pty = member_type.runs_in_pty();
            let wanted = if pty {
                choice.start_terminals
            } else {
                choice.launch_external
            };
            let (outcome, reason) = if !wanted {
                ("skipped", None)
            } else if pty
                && self
                    .sessions
                    .find_by_member(workspace_id, &member_id)
                    .is_some()
            {
                ("running", None)
            } else {
                let result = if pty {
                    self.start_session(workspace_id, &member_id, 24, 80)
                        .map(|_| ())
                } else {
                    self.launch_external(workspace_id, &member_id).map(|_| ())
                };
                match result {
                    Ok(()) => ("started", None),
                    Err(e) => ("denied", Some(e)),
                }
            };
            outcomes.push(MemberOutcome {
                member_id,
                outcome: outcome.into(),
                reason,
            });
        }
        Ok(OpenResult {
            needs_choice: false,
            outcomes,
        })
    }

    fn on_session(&self, info: &SessionInfo) {
        self.bus.emit(AppEvent::Session { info: info.clone() });
        if !matches!(
            info.process_state,
            crate::session::ProcessState::Exited { .. }
        ) {
            return;
        }
        let auto = {
            let data = self.data.lock().unwrap();
            data.state
                .workspaces
                .iter()
                .find(|w| w.id == info.workspace_id)
                .and_then(|w| w.member(&info.member_id))
                .map(|m| m.auto_restart)
                .unwrap_or(false)
        };
        if !auto {
            return;
        }
        // Opt-in auto restart (FR-035), bounded to avoid restart storms.
        let allowed = {
            let mut restarts = self.restarts.lock().unwrap();
            let history = restarts.entry(info.session_id.clone()).or_default();
            history.retain(|t| t.elapsed() < Duration::from_secs(60));
            if history.len() >= 5 {
                false
            } else {
                history.push(Instant::now());
                true
            }
        };
        if !allowed {
            self.notice(
                "warning",
                format!("{} keeps exiting; automatic restart paused", info.title),
            );
            return;
        }
        if let Some(core) = self.arc() {
            let id = info.session_id.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(1));
                let _ = core.restart_session(&id);
            });
        }
    }

    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        self.sessions.close(session_id).map_err(|e| e.to_string())?;
        self.bus.emit(AppEvent::SessionClosed {
            session_id: session_id.to_string(),
        });
        Ok(())
    }

    // ---- import / export -------------------------------------------------

    pub fn export_workspace(
        &self,
        workspace_id: &str,
        machine_bound: bool,
    ) -> Result<String, String> {
        check_id(workspace_id)?;
        let data = self.data.lock().unwrap();
        let w = data
            .state
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .ok_or("workspace not found")?;
        let names = |id: &str| data.registry.get(id).map(|l| l.display_name.clone());
        transfer::export(w, &names, machine_bound)
            .map_err(|findings| format!("export refused: {}", findings.join("; ")))
    }

    pub fn import_workspace(&self, text: &str) -> Result<ImportReport, String> {
        let imported = {
            let data = self.data.lock().unwrap();
            let exists = |id: &str| data.registry.get(id).is_some();
            transfer::import(text, &transfer::Resolver { exists: &exists })?
        };
        let report = imported.report.clone();
        self.mutate(|s, _| {
            if s.workspaces.len() >= MAX_WORKSPACES {
                return Err(format!("at most {MAX_WORKSPACES} workspaces"));
            }
            for spec in &imported.synthetic {
                if !s.registry.synthetic.contains(spec)
                    && s.registry.synthetic.len() < MAX_SYNTHETIC
                {
                    s.registry.synthetic.push(spec.clone());
                }
            }
            s.workspaces.push(imported.workspace.clone());
            Ok(())
        })?;
        self.audit.record(
            "workspace.import",
            Outcome::Recorded,
            Record {
                workspace_id: Some(&report.workspace_id),
                detail: Some("imported into review staging; nothing authorized"),
                ..Default::default()
            },
        );
        if !imported.synthetic.is_empty() {
            self.rescan()?;
        }
        self.changed();
        Ok(report)
    }

    // ---- settings / misc -------------------------------------------------

    pub fn settings(&self) -> Settings {
        self.data.lock().unwrap().settings.clone()
    }

    pub fn update_settings(&self, settings: Settings) -> Result<(), String> {
        settings.validate()?;
        let watch_changed = {
            let mut data = self.data.lock().unwrap();
            self.config_store
                .save("settings", SETTINGS_FORMAT, SETTINGS_SCHEMA, &settings)?;
            let changed = data.settings.live_updates != settings.live_updates;
            self.sessions
                .set_retention(settings.replay_buffer_mib as usize * 1024 * 1024);
            data.settings = settings;
            changed
        };
        if watch_changed {
            self.restart_watch();
        }
        self.bus.emit(AppEvent::Settings);
        Ok(())
    }

    pub fn audit_recent(&self, limit: usize) -> Vec<AuditEntry> {
        self.audit.recent(limit.min(500))
    }

    pub fn open_url(&self, url: &str) -> Result<(), String> {
        let result = external::open_url(url);
        self.audit.record(
            "url.open",
            if result.is_ok() {
                Outcome::Allowed
            } else {
                Outcome::Denied
            },
            Record {
                detail: result.as_ref().err().map(String::as_str),
                ..Default::default()
            },
        );
        result
    }

    pub fn shutdown(&self) {
        *self.watch.lock().unwrap() = None;
        self.sessions.shutdown();
    }

    #[cfg(test)]
    pub fn registry_entry(&self, id: &str) -> Option<RegistryEntry> {
        let data = self.data.lock().unwrap();
        data.registry.effective(id, &data.state.registry)
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
