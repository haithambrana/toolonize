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
use crate::sync::Locked;
use crate::transfer::{self, ImportReport};
use crate::workspace::{
    validate_color, validate_dir, validate_layout, validate_name, validate_title, LayoutMode,
    LayoutState, Member, MemberType, StartupPolicy, Workspace, MAX_MEMBERS, MAX_WORKSPACES,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

mod backup;
pub use backup::TemplateSummary;
mod execution;
mod import_export;
mod launchers;
mod misc;
mod notes;
pub use notes::{NotesView, TrashSummary};
mod workspaces;

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
    /// Saved workspace shapes (ADR-014).
    #[serde(default)]
    pub templates: Vec<crate::backup::Template>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    Session {
        info: SessionInfo,
    },
    SessionClosed {
        session_id: String,
    },
    Registry {
        diff: Diff,
    },
    Workspaces,
    Settings,
    Watch {
        status: watch::WatchStatus,
    },
    Notice {
        level: String,
        message: String,
    },
    /// SSH auto-reconnect progress for a session (`active: false` = stopped).
    Reconnect {
        session_id: String,
        active: bool,
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
    },
}

pub trait EventSink: Send + Sync {
    fn send(&self, event: &AppEvent) -> bool;
}

#[derive(Default)]
struct Bus {
    sinks: Mutex<Vec<Arc<dyn EventSink>>>,
    /// Core-side listeners (window title); kept when the UI resubscribes.
    internal: Mutex<Vec<Arc<dyn EventSink>>>,
}

impl Bus {
    fn emit(&self, event: AppEvent) {
        let internal: Vec<_> = self.internal.locked().clone();
        for sink in &internal {
            sink.send(&event);
        }
        let sinks: Vec<_> = self.sinks.locked().clone();
        let mut dead = Vec::new();
        for (index, sink) in sinks.iter().enumerate() {
            if !sink.send(&event) {
                dead.push(index);
            }
        }
        if !dead.is_empty() {
            let mut sinks = self.sinks.locked();
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
    pub color: Option<String>,
    pub project_dir: Option<String>,
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
    /// Palette id; an empty string clears the tag.
    #[serde(default)]
    pub color: Option<String>,
    /// Existing absolute folder; an empty string clears it.
    #[serde(default)]
    pub start_dir: Option<String>,
    /// Font size change for this terminal, -6..=12.
    #[serde(default)]
    pub zoom: Option<i8>,
}

/// SSH auto-reconnect bookkeeping per session.
#[derive(Debug, Clone, Copy)]
struct Reconnect {
    attempt: u32,
    /// Bumped on every schedule/cancel so stale timers do nothing.
    token: u64,
}

pub const RECONNECT_MAX_ATTEMPTS: u32 = 8;
/// A connection that stayed up this long starts the backoff from scratch.
const RECONNECT_STABLE_MS: u64 = 60_000;

/// Backoff before reconnect attempt `n` (1-based): 2, 4, 8, 16, 30, 30, ... s.
pub fn reconnect_delay(attempt: u32) -> Duration {
    Duration::from_secs((1u64 << attempt.min(5)).min(30))
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
    reconnects: Mutex<HashMap<String, Reconnect>>,
    /// Sticky notes, one file per workspace (ADR-012), loaded on first use.
    notes_store: Store,
    notes: Mutex<HashMap<String, crate::notes::WorkspaceNotes>>,
    /// Serializes read-modify-write of notes files.
    notes_write: Mutex<()>,
    /// Recent desktop notifications (rate limit).
    notified: Mutex<std::collections::VecDeque<Instant>>,
    reconnect_token: std::sync::atomic::AtomicU64,
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
        crate::logging::info(
            "startup",
            &format!(
                "ToolOnize {} on {}-{}; state generation {}, recovered: {}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH,
                load_report.generation,
                load_report.recovered
            ),
        );
        for problem in &load_report.problems {
            crate::logging::warn("startup", problem);
        }
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
                reconnects: Mutex::new(HashMap::new()),
                notes_store: Store::new(state_dir.join("notes")),
                notes: Mutex::new(HashMap::new()),
                notes_write: Mutex::new(()),
                notified: Mutex::new(std::collections::VecDeque::new()),
                reconnect_token: std::sync::atomic::AtomicU64::new(0),
                opened: Mutex::new(std::collections::HashSet::new()),
                me,
            }
        })
    }

    /// The name a member is shown with: its title, else the launcher's
    /// alias or name.
    fn member_title(&self, m: &Member, data: &Data) -> String {
        m.title.clone().unwrap_or_else(|| {
            data.registry
                .effective(&m.launcher_id, &data.state.registry)
                .map(|e| e.decision.alias.unwrap_or(e.launcher.display_name))
                .unwrap_or_else(|| m.launcher_id.clone())
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
        *self.bus.sinks.locked() = vec![sink];
    }

    /// A listener inside the app process that survives UI resubscription.
    pub fn add_internal_sink(&self, sink: Arc<dyn EventSink>) {
        self.bus.internal.locked().push(sink);
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
        let mut data = self.data.locked();
        let mut next = data.state.clone();
        let result = f(&mut next, &data.registry)?;
        if next != data.state {
            self.store
                .save("state", STATE_FORMAT, STATE_SCHEMA, &next)?;
            data.state = next;
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "../app_tests.rs"]
mod tests;
