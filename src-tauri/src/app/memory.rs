//! Session memory (ADR-015): continue after closing the app or a power loss.
//!
//! A background thread keeps `<state>/memory/` up to date: the list of
//! running terminals (with folder and size) within about a second of any
//! change, the cleaned output tail of each terminal every 15 seconds, and
//! the window's place. At the next start the terminals that were running
//! are started again through the normal execution policy, each showing its
//! saved output first.

use super::*;
use crate::memory::{
    prelude, snapshot_text, term_store_name, MemoryIndex, RunningTerminal, TerminalMemory,
    WindowState, INDEX_FORMAT, MAX_RAW_TAIL, MEMORY_SCHEMA, TERM_FORMAT, WINDOW_FORMAT,
};
use crate::session::ProcessState;
use std::collections::HashSet;
use std::sync::atomic::Ordering;

const INDEX_NAME: &str = "session";
const WINDOW_NAME: &str = "window";
const TICK: Duration = Duration::from_secs(1);
/// Content snapshots every this many ticks.
const CONTENT_EVERY: u32 = 15;
/// More than this many terminals are not restarted automatically.
const MAX_RESUME: usize = 64;

#[derive(Default)]
pub(super) struct MemoryRuntime {
    index: MemoryIndex,
    /// store name -> session end offset at the last content save
    saved_offsets: HashMap<String, u64>,
    /// store names whose saved output was already shown in this run
    resumed: HashSet<String>,
    shutting_down: bool,
    window_saved: Option<WindowState>,
    window_pending: Option<WindowState>,
}

/// What happened at startup, shown once by the UI.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ResumeReport {
    /// The previous run did not end normally (crash, power loss, killed).
    pub crashed: bool,
    pub restored: usize,
    /// Commands that are not restarted automatically (side effects).
    pub left_stopped: Vec<String>,
    /// "<name>: <reason>" for terminals the policy refused.
    pub failed: Vec<String>,
}

fn running_state(state: &ProcessState) -> bool {
    matches!(
        state,
        ProcessState::Starting
            | ProcessState::Running
            | ProcessState::Restarting
            | ProcessState::Reconnecting
    )
}

impl Core {
    /// Terminals running right now, as they would be restored.
    fn running_now(&self) -> Vec<RunningTerminal> {
        self.sessions
            .list()
            .into_iter()
            .filter(|i| running_state(&i.process_state))
            .map(|i| RunningTerminal {
                cwd: if i.remote {
                    None
                } else {
                    i.pid.and_then(crate::session::process_cwd)
                },
                workspace_id: i.workspace_id,
                member_id: i.member_id,
                rows: i.rows,
                cols: i.cols,
            })
            .collect()
    }

    fn save_index(&self, m: &MemoryRuntime) {
        if let Err(error) =
            self.memory_store
                .save(INDEX_NAME, INDEX_FORMAT, MEMORY_SCHEMA, &m.index)
        {
            crate::logging::warn(
                "memory",
                &format!("could not save the session list: {error}"),
            );
        }
    }

    /// Save the running list if it changed.
    pub fn save_running(&self) {
        self.save_running_inner(false);
    }

    fn save_running_inner(&self, final_save: bool) {
        let running = self.running_now();
        let mut m = self.memory.locked();
        if m.shutting_down && !final_save {
            return;
        }
        if m.index.running == running {
            return;
        }
        m.index.running = running;
        m.index.saved_at = now_ms();
        self.save_index(&m);
    }

    /// Save the output tail of every terminal whose output changed.
    pub fn snapshot_content(&self) {
        self.snapshot_content_inner(false);
    }

    fn snapshot_content_inner(&self, final_save: bool) {
        if !self.data.locked().settings.remember_terminal_content {
            return;
        }
        for info in self.sessions.list() {
            let Some(name) = term_store_name(&info.workspace_id, &info.member_id) else {
                continue;
            };
            if self.memory.locked().saved_offsets.get(&name) == Some(&info.end_offset) {
                continue;
            }
            let Ok(raw) = self.sessions.tail(&info.session_id, MAX_RAW_TAIL) else {
                continue;
            };
            let saved = TerminalMemory {
                text: snapshot_text(&raw),
                saved_at: now_ms(),
                cols: info.cols,
            };
            let mut m = self.memory.locked();
            // Closed meanwhile (its memory was forgotten), or shutting down.
            if (m.shutting_down && !final_save) || self.sessions.info(&info.session_id).is_err() {
                continue;
            }
            match self
                .memory_store
                .save(&name, TERM_FORMAT, MEMORY_SCHEMA, &saved)
            {
                Ok(()) => {
                    m.saved_offsets.insert(name, info.end_offset);
                }
                Err(error) => crate::logging::warn(
                    "memory",
                    &format!("could not save a terminal's output: {error}"),
                ),
            }
        }
    }

    /// Saved output to show before a member's first output in this run.
    /// Returns the bytes and the width they were written at.
    pub(super) fn take_prelude(&self, workspace_id: &str, member_id: &str) -> (Vec<u8>, u16) {
        let Some(name) = term_store_name(workspace_id, member_id) else {
            return (Vec::new(), 0);
        };
        if !self.data.locked().settings.remember_terminal_content
            || !self.memory.locked().resumed.insert(name.clone())
        {
            return (Vec::new(), 0);
        }
        let (saved, _) =
            self.memory_store
                .load::<TerminalMemory>(&name, TERM_FORMAT, MEMORY_SCHEMA, &[]);
        saved
            // Width 0 = unknown (older file): shown at the current width.
            .map(|s| {
                let cols = if s.cols == 0 {
                    0
                } else {
                    s.cols.clamp(20, 500)
                };
                (prelude(&s), cols)
            })
            .unwrap_or_default()
    }

    /// The user closed this terminal (or removed it): forget its output.
    pub(super) fn forget_member_memory(&self, workspace_id: &str, member_id: &str) {
        if let Some(name) = term_store_name(workspace_id, member_id) {
            let mut m = self.memory.locked();
            m.saved_offsets.remove(&name);
            self.memory_store.remove(&name);
        }
        self.memory_dirty.store(true, Ordering::Relaxed);
    }

    /// Delete every saved terminal output. Returns how many were deleted.
    pub fn forget_terminal_content(&self) -> usize {
        let mut m = self.memory.locked();
        m.saved_offsets.clear();
        let names = self.term_files();
        for name in &names {
            self.memory_store.remove(name);
        }
        names.len()
    }

    /// Store names of saved terminal outputs on disk.
    fn term_files(&self) -> Vec<String> {
        let Ok(dir) = std::fs::read_dir(self.memory_store.dir()) else {
            return Vec::new();
        };
        let mut names: Vec<String> = dir
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter_map(|f| {
                f.strip_prefix("term-")
                    .and_then(|r| r.split(".json").next())
                    .map(|base| format!("term-{base}"))
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Remove saved outputs of members that no longer exist.
    fn collect_memory_garbage(&self) {
        let keep: HashSet<String> = {
            let data = self.data.locked();
            data.state
                .workspaces
                .iter()
                .flat_map(|w| {
                    w.members
                        .iter()
                        .filter_map(|m| term_store_name(&w.id, &m.id))
                })
                .collect()
        };
        for name in self.term_files() {
            if !keep.contains(&name) {
                self.memory_store.remove(&name);
            }
        }
    }

    /// Start again what was running when the app last closed or the machine
    /// lost power. Called once at startup, before the window opens.
    pub fn resume_session(&self) -> ResumeReport {
        prepare_private_dir(self.memory_store.dir());
        let (index, _) =
            self.memory_store
                .load::<MemoryIndex>(INDEX_NAME, INDEX_FORMAT, MEMORY_SCHEMA, &[]);
        let had_index = index.is_some();
        let index = index.unwrap_or_default();
        let window_saved = self.saved_window();
        {
            // From now on a sudden end of this run is detectable.
            let mut m = self.memory.locked();
            m.index = MemoryIndex {
                running: index.running.clone(),
                clean_exit: false,
                saved_at: now_ms(),
            };
            m.window_saved = window_saved;
            self.save_index(&m);
        }
        self.collect_memory_garbage();
        let mut report = ResumeReport {
            crashed: had_index && !index.clean_exit && !index.running.is_empty(),
            ..Default::default()
        };
        if self.data.locked().settings.restore_terminals {
            for t in index.running.iter().take(MAX_RESUME) {
                if check_id(&t.workspace_id).is_err() || check_id(&t.member_id).is_err() {
                    continue;
                }
                let found = {
                    let data = self.data.locked();
                    data.state
                        .workspaces
                        .iter()
                        .find(|w| w.id == t.workspace_id && !w.staged_import)
                        .and_then(|w| w.member(&t.member_id))
                        .map(|m| (m.member_type, m.auto_restart, self.member_title(m, &data)))
                };
                let Some((kind, auto_restart, title)) = found else {
                    continue;
                };
                // Shells and SSH sessions are safe to start again; a command
                // can have side effects, so it waits for the user unless it
                // is marked "restart automatically".
                let eligible = matches!(kind, MemberType::Terminal | MemberType::RemoteCommand)
                    || (kind == MemberType::LocalCommand && auto_restart);
                if !eligible {
                    report.left_stopped.push(title);
                    continue;
                }
                match self.start_session_with(
                    &t.workspace_id,
                    &t.member_id,
                    t.rows,
                    t.cols,
                    t.cwd.as_deref(),
                ) {
                    Ok(_) => {
                        report.restored += 1;
                        self.opened.locked().insert(t.workspace_id.clone());
                    }
                    Err(error) => report.failed.push(format!("{title}: {error}")),
                }
            }
        }
        if report.restored > 0 || !report.failed.is_empty() {
            let detail = format!(
                "{} terminal(s) restored{}",
                report.restored,
                if report.crashed {
                    " after an abrupt end"
                } else {
                    ""
                }
            );
            self.audit.record(
                "session.resume",
                Outcome::Recorded,
                Record {
                    detail: Some(&detail),
                    ..Default::default()
                },
            );
        }
        crate::logging::info(
            "memory",
            &format!(
                "resume: crashed={} restored={} left_stopped={} failed={}",
                report.crashed,
                report.restored,
                report.left_stopped.len(),
                report.failed.len()
            ),
        );
        *self.resume_report.locked() = Some(report.clone());
        report
    }

    /// Keep the memory files current until the app exits.
    pub fn start_memory(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let spawned = std::thread::Builder::new()
            .name("session-memory".into())
            .spawn(move || {
                let mut tick = 0u32;
                loop {
                    std::thread::sleep(TICK);
                    let Some(core) = weak.upgrade() else { break };
                    if core.memory.locked().shutting_down {
                        break;
                    }
                    tick = tick.wrapping_add(1);
                    let periodic = tick % CONTENT_EVERY == 0;
                    // Periodic too: a `cd` changes the folder without an event.
                    if core.memory_dirty.swap(false, Ordering::Relaxed) || periodic {
                        core.save_running();
                    }
                    if periodic {
                        core.snapshot_content();
                    }
                    core.flush_window();
                }
            });
        if let Err(error) = spawned {
            crate::logging::error("memory", &format!("could not start: {error}"));
        }
    }

    /// Final save at a normal exit, before terminals are stopped.
    pub(super) fn memory_shutdown(&self) {
        self.save_running_inner(true);
        self.snapshot_content_inner(true);
        self.flush_window();
        let mut m = self.memory.locked();
        m.shutting_down = true;
        m.index.clean_exit = true;
        m.index.saved_at = now_ms();
        self.save_index(&m);
    }

    pub fn saved_window(&self) -> Option<WindowState> {
        self.memory_store
            .load::<WindowState>(WINDOW_NAME, WINDOW_FORMAT, MEMORY_SCHEMA, &[])
            .0
    }

    /// The window moved or was resized (saved within a second).
    pub fn note_window(&self, state: WindowState) {
        self.memory.locked().window_pending = Some(state);
    }

    /// The window was (un)maximized: keep the normal size and place.
    pub fn note_window_maximized(&self, maximized: bool) {
        let mut m = self.memory.locked();
        if let Some(base) = m.window_pending.or(m.window_saved) {
            m.window_pending = Some(WindowState { maximized, ..base });
        }
    }

    pub(super) fn flush_window(&self) {
        let mut m = self.memory.locked();
        let Some(pending) = m.window_pending.take() else {
            return;
        };
        if m.window_saved == Some(pending) {
            return;
        }
        match self
            .memory_store
            .save(WINDOW_NAME, WINDOW_FORMAT, MEMORY_SCHEMA, &pending)
        {
            Ok(()) => m.window_saved = Some(pending),
            Err(error) => {
                crate::logging::warn("memory", &format!("could not save the window: {error}"))
            }
        }
    }

    /// Counts for diagnostics (no names, no content).
    pub(super) fn memory_summary(&self) -> (usize, usize) {
        (
            self.memory.locked().index.running.len(),
            self.term_files().len(),
        )
    }
}

/// Saved terminal output is private: the folder is readable by this user
/// only (Unix; on Windows the per-user profile folder already is).
fn prepare_private_dir(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
}
