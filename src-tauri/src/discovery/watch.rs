//! Change watching for launcher roots (FR-005).
//!
//! Uses the OS-native backend (inotify / ReadDirectoryChangesW via `notify`).
//! Every event, overflow, or rescan hint collapses into one debounced
//! "rescan" callback; the registry diff decides what actually changed.
//! Failure to watch degrades to manual rescan with a visible status (FS-01).

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WatchStatus {
    Live { roots: usize },
    Degraded { reason: String },
    Off,
}

pub struct WatchHandle {
    _watcher: Option<RecommendedWatcher>,
    status: Arc<Mutex<WatchStatus>>,
}

impl WatchHandle {
    pub fn status(&self) -> WatchStatus {
        self.status.lock().unwrap().clone()
    }
}

fn nearest_existing(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent();
    while let Some(p) = current {
        if p.is_dir() {
            return Some(p.to_path_buf());
        }
        current = p.parent();
    }
    None
}

pub fn start(roots: Vec<PathBuf>, on_change: Arc<dyn Fn() + Send + Sync>) -> WatchHandle {
    let status = Arc::new(Mutex::new(WatchStatus::Off));
    let (tx, rx) = mpsc::channel::<bool>();
    let error_status = status.clone();
    let watcher =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
            Ok(_) => {
                let _ = tx.send(true);
            }
            Err(error) => {
                *error_status.lock().unwrap() = WatchStatus::Degraded {
                    reason: error.to_string(),
                };
                // Overflow and similar errors: a full rescan is the safe answer.
                let _ = tx.send(true);
            }
        });
    let mut watcher = match watcher {
        Ok(w) => w,
        Err(error) => {
            *status.lock().unwrap() = WatchStatus::Degraded {
                reason: format!("watch service unavailable: {error}"),
            };
            return WatchHandle {
                _watcher: None,
                status,
            };
        }
    };

    let mut watched = 0;
    let mut failures = Vec::new();
    let mut ancestors = Vec::new();
    for root in &roots {
        if root.is_dir() {
            match watcher.watch(root, RecursiveMode::Recursive) {
                Ok(()) => watched += 1,
                Err(error) => failures.push(format!("{}: {error}", root.display())),
            }
        } else if let Some(ancestor) = nearest_existing(root) {
            // Notice the root being created later.
            if !ancestors.contains(&ancestor)
                && watcher
                    .watch(&ancestor, RecursiveMode::NonRecursive)
                    .is_ok()
            {
                ancestors.push(ancestor);
            }
        }
    }
    *status.lock().unwrap() = if failures.is_empty() {
        WatchStatus::Live { roots: watched }
    } else {
        WatchStatus::Degraded {
            reason: failures.join("; "),
        }
    };

    std::thread::Builder::new()
        .name("launcher-watch-debounce".into())
        .spawn(move || loop {
            match rx.recv() {
                Ok(_) => {}
                Err(_) => return,
            }
            // Coalesce bursts (package installs touch many files).
            loop {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(_) => continue,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            on_change();
        })
        .expect("spawn watch debounce thread");

    WatchHandle {
        _watcher: Some(watcher),
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    fn wait_for(counter: &AtomicUsize, at_least: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if counter.load(Ordering::SeqCst) >= at_least {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        false
    }

    #[test]
    fn add_modify_remove_trigger_debounced_rescans_t_disc_lnx_010() {
        let dir = tempfile::tempdir().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        let handle = start(
            vec![dir.path().to_path_buf()],
            Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
            }),
        );
        assert_eq!(handle.status(), WatchStatus::Live { roots: 1 });
        let file = dir.path().join("org.example.New.desktop");
        std::fs::write(&file, "[Desktop Entry]\n").unwrap();
        assert!(wait_for(&counter, 1), "add noticed");
        std::thread::sleep(DEBOUNCE * 2);
        let after_add = counter.load(Ordering::SeqCst);
        std::fs::write(&file, "[Desktop Entry]\nName=Changed\n").unwrap();
        assert!(wait_for(&counter, after_add + 1), "modify noticed");
        std::thread::sleep(DEBOUNCE * 2);
        let after_modify = counter.load(Ordering::SeqCst);
        std::fs::remove_file(&file).unwrap();
        assert!(wait_for(&counter, after_modify + 1), "remove noticed");
    }

    #[test]
    fn burst_is_coalesced() {
        let dir = tempfile::tempdir().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        let _handle = start(
            vec![dir.path().to_path_buf()],
            Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
            }),
        );
        for i in 0..50 {
            std::fs::write(dir.path().join(format!("{i}.desktop")), "x").unwrap();
        }
        assert!(wait_for(&counter, 1));
        std::thread::sleep(DEBOUNCE * 3);
        assert!(counter.load(Ordering::SeqCst) <= 3, "burst coalesced");
    }

    #[test]
    fn missing_root_watches_ancestor_for_creation() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("applications");
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        let _handle = start(
            vec![missing.clone()],
            Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
            }),
        );
        std::fs::create_dir(&missing).unwrap();
        assert!(wait_for(&counter, 1), "root creation noticed");
    }
}
