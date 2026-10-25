//! Crash-safe local persistence (FR-051/052, NFR-005, ADR-006).
//!
//! Every committed mutation writes a complete snapshot atomically:
//! temp file -> fsync -> rotate generations -> rename -> fsync dir. Each
//! snapshot carries a schema version and a SHA-256 checksum. Loading takes
//! the newest generation that verifies; if that is not the latest file, the
//! loss window is reported (FS-03) and the corrupt file is kept aside for
//! inspection. A failed write leaves the previous state untouched (FS-08).

use crate::model::{hex, now_ms};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

const GENERATIONS: usize = 3;
pub const MAX_STATE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct Envelope {
    format: String,
    schema_version: u32,
    saved_at: u64,
    checksum: String,
    payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct LoadReport {
    /// Which generation was used (0 = latest).
    pub generation: usize,
    pub recovered: bool,
    /// Human-readable description of what could not be loaded.
    pub problems: Vec<String>,
    /// Timestamp of the snapshot that was restored.
    pub restored_saved_at: Option<u64>,
    pub migrated_from: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

fn checksum(payload: &Value) -> String {
    let bytes = serde_json::to_vec(payload).expect("JSON value serializes");
    hex(&Sha256::digest(bytes))
}

/// Migration hook: transforms a payload from `from` to `from + 1`.
pub type Migration = fn(Value) -> Result<Value, String>;

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, name: &str, generation: usize) -> PathBuf {
        if generation == 0 {
            self.dir.join(format!("{name}.json"))
        } else {
            self.dir.join(format!("{name}.json.{generation}"))
        }
    }

    /// Atomically persist `value` as the newest generation of `name`.
    pub fn save<T: Serialize>(
        &self,
        name: &str,
        format: &str,
        schema_version: u32,
        value: &T,
    ) -> Result<(), String> {
        let payload = serde_json::to_value(value).map_err(|e| e.to_string())?;
        let envelope = Envelope {
            format: format.to_string(),
            schema_version,
            saved_at: now_ms(),
            checksum: checksum(&payload),
            payload,
        };
        let bytes = serde_json::to_vec_pretty(&envelope).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err("state is too large to save".into());
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("cannot create state dir: {e}"))?;
        let tmp = self.dir.join(format!(".{name}.json.tmp"));
        let write = || -> std::io::Result<()> {
            let mut file = std::fs::File::create(&tmp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            Ok(())
        };
        if let Err(error) = write() {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!(
                "could not write state (nothing was changed): {error}"
            ));
        }
        // Rotate: .2 <- .1 <- latest. Each rename is atomic; a crash between
        // them leaves a loadable older generation.
        for g in (1..GENERATIONS).rev() {
            let from = self.path(name, g - 1);
            if from.exists() {
                let _ = std::fs::rename(&from, self.path(name, g));
            }
        }
        std::fs::rename(&tmp, self.path(name, 0)).map_err(|e| {
            let _ = std::fs::rename(self.path(name, 1), self.path(name, 0));
            format!("could not commit state: {e}")
        })?;
        sync_dir(&self.dir);
        Ok(())
    }

    /// Load the newest valid generation, migrating forward as needed.
    pub fn load<T: DeserializeOwned>(
        &self,
        name: &str,
        format: &str,
        current_version: u32,
        migrations: &[(u32, Migration)],
    ) -> (Option<T>, LoadReport) {
        let mut report = LoadReport::default();
        let mut any_file = false;
        for generation in 0..GENERATIONS {
            let path = self.path(name, generation);
            if !path.exists() {
                continue;
            }
            any_file = true;
            match self.read_generation(&path, format, current_version, migrations) {
                Ok((value, saved_at, migrated)) => {
                    report.generation = generation;
                    report.recovered = !report.problems.is_empty();
                    report.restored_saved_at = Some(saved_at);
                    report.migrated_from = migrated;
                    if report.recovered {
                        report.problems.push(format!(
                            "restored the snapshot saved at {saved_at} (ms since epoch); later changes were lost"
                        ));
                    }
                    return (Some(value), report);
                }
                Err(problem) => {
                    report.problems.push(format!(
                        "{}: {problem}",
                        path.file_name().unwrap().to_string_lossy()
                    ));
                    if generation == 0 {
                        let aside = self.dir.join(format!("{name}.json.corrupt-{}", now_ms()));
                        let _ = std::fs::copy(&path, aside);
                    }
                }
            }
        }
        report.recovered = any_file;
        (None, report)
    }

    fn read_generation<T: DeserializeOwned>(
        &self,
        path: &Path,
        format: &str,
        current_version: u32,
        migrations: &[(u32, Migration)],
    ) -> Result<(T, u64, Option<u32>), String> {
        let bytes = crate::discovery::walk::read_bounded(path, MAX_STATE_BYTES)?;
        let envelope: Envelope =
            serde_json::from_slice(&bytes).map_err(|e| format!("unreadable: {e}"))?;
        if envelope.format != format {
            return Err(format!("unexpected format {}", envelope.format));
        }
        if checksum(&envelope.payload) != envelope.checksum {
            return Err("checksum mismatch (corrupted)".into());
        }
        if envelope.schema_version > current_version {
            return Err(format!(
                "written by a newer ToolOnize (schema {}); refusing to downgrade",
                envelope.schema_version
            ));
        }
        let mut payload = envelope.payload;
        let mut version = envelope.schema_version;
        let migrated = (version < current_version).then_some(version);
        while version < current_version {
            let step = migrations
                .iter()
                .find(|(from, _)| *from == version)
                .ok_or(format!("no migration from schema {version}"))?;
            payload = (step.1)(payload)?;
            version += 1;
        }
        let value = serde_json::from_value(payload).map_err(|e| format!("invalid content: {e}"))?;
        Ok((value, envelope.saved_at, migrated))
    }
}

#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

/// Append-only, size-rotated JSON-lines file (audit log).
pub fn append_line(path: &Path, line: &str, max_bytes: u64) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::metadata(path)
        .map(|m| m.len() > max_bytes)
        .unwrap_or(false)
    {
        let rotated = path.with_extension("log.1");
        let _ = std::fs::rename(path, rotated);
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")
}

/// Per-platform storage locations (ARCHITECTURE §12). `TOOLONIZE_STATE_DIR`
/// / `TOOLONIZE_CONFIG_DIR` override them (tests, portable use).
pub fn default_dirs() -> (PathBuf, PathBuf) {
    let env_dir = |k: &str| {
        std::env::var_os(k)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    let state = env_dir("TOOLONIZE_STATE_DIR");
    let config = env_dir("TOOLONIZE_CONFIG_DIR");
    if let (Some(s), Some(c)) = (&state, &config) {
        return (s.clone(), c.clone());
    }
    #[cfg(windows)]
    let (base_state, base_config) = {
        let appdata = crate::discovery::windows::known_folder(
            &windows_sys::Win32::UI::Shell::FOLDERID_RoamingAppData,
        )
        .unwrap_or_else(|| PathBuf::from("."));
        let root = appdata.join("toolonize");
        (root.join("state"), root)
    };
    #[cfg(not(windows))]
    let (base_state, base_config) = {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let xdg = |k: &str, fallback: &str| {
            std::env::var_os(k)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| home.join(fallback))
        };
        (
            xdg("XDG_STATE_HOME", ".local/state").join("toolonize"),
            xdg("XDG_CONFIG_HOME", ".config").join("toolonize"),
        )
    };
    (state.unwrap_or(base_state), config.unwrap_or(base_config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
    struct Doc {
        n: u32,
        name: String,
    }

    fn load(store: &Store) -> (Option<Doc>, LoadReport) {
        store.load("state", "test-state", 2, &[(1, migrate_v1)])
    }

    fn migrate_v1(mut v: Value) -> Result<Value, String> {
        let old = v.get("count").cloned().ok_or("missing count")?;
        let obj = v.as_object_mut().ok_or("not an object")?;
        obj.remove("count");
        obj.insert("n".into(), old);
        obj.insert("name".into(), json!("migrated"));
        Ok(v)
    }

    #[test]
    fn save_load_round_trip_t_persist_001() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let doc = Doc {
            n: 1,
            name: "a".into(),
        };
        store.save("state", "test-state", 2, &doc).unwrap();
        let (loaded, report) = load(&store);
        assert_eq!(loaded, Some(doc));
        assert!(!report.recovered);
        assert_eq!(report.generation, 0);
    }

    #[test]
    fn corruption_falls_back_with_loss_report_fs_03() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        store
            .save(
                "state",
                "test-state",
                2,
                &Doc {
                    n: 1,
                    name: "old".into(),
                },
            )
            .unwrap();
        store
            .save(
                "state",
                "test-state",
                2,
                &Doc {
                    n: 2,
                    name: "new".into(),
                },
            )
            .unwrap();
        // Bit flip inside the payload of the newest generation.
        let path = dir.path().join("state.json");
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\"new\"", "\"nex\"");
        std::fs::write(&path, text).unwrap();
        let (loaded, report) = load(&store);
        assert_eq!(loaded.unwrap().name, "old");
        assert!(report.recovered);
        assert_eq!(report.generation, 1);
        assert!(report.problems.iter().any(|p| p.contains("checksum")));
        assert!(std::fs::read_dir(dir.path()).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("corrupt")));
    }

    #[test]
    fn truncation_and_garbage_never_silently_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        store
            .save(
                "state",
                "test-state",
                2,
                &Doc {
                    n: 1,
                    name: "a".into(),
                },
            )
            .unwrap();
        let path = dir.path().join("state.json");
        let bytes = std::fs::read(&path).unwrap();
        for cut in [0, 1, bytes.len() / 2, bytes.len() - 2] {
            std::fs::write(&path, &bytes[..cut]).unwrap();
            for g in 1..GENERATIONS {
                let _ = std::fs::remove_file(dir.path().join(format!("state.json.{g}")));
            }
            let (loaded, report) = load(&store);
            assert!(loaded.is_none(), "cut {cut}");
            assert!(report.recovered && !report.problems.is_empty());
        }
    }

    #[test]
    fn kill_mid_write_leaves_consistent_state_t_persist_004() {
        // Simulate a crash after the temp file was written but before the
        // rename: the committed snapshot is still the previous one.
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        store
            .save(
                "state",
                "test-state",
                2,
                &Doc {
                    n: 1,
                    name: "committed".into(),
                },
            )
            .unwrap();
        std::fs::write(dir.path().join(".state.json.tmp"), b"{\"partial").unwrap();
        let (loaded, report) = load(&store);
        assert_eq!(loaded.unwrap().name, "committed");
        assert!(!report.recovered);
        // And a crash between rotation and commit (latest missing).
        std::fs::rename(
            dir.path().join("state.json"),
            dir.path().join("state.json.1"),
        )
        .unwrap();
        let (loaded, _) = load(&store);
        assert_eq!(loaded.unwrap().name, "committed");
    }

    #[test]
    fn migration_and_downgrade_refusal() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        store
            .save("state", "test-state", 1, &json!({"count": 7}))
            .unwrap();
        let (loaded, report) = load(&store);
        assert_eq!(
            loaded,
            Some(Doc {
                n: 7,
                name: "migrated".into()
            })
        );
        assert_eq!(report.migrated_from, Some(1));
        store
            .save("state", "test-state", 9, &json!({"n": 1, "name": "future"}))
            .unwrap();
        let (loaded, report) = load(&store);
        assert_eq!(
            loaded.unwrap().name,
            "migrated",
            "falls back to older generation"
        );
        assert!(report.problems.iter().any(|p| p.contains("newer")));
    }

    #[test]
    fn write_failure_changes_nothing_fs_08() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        store
            .save(
                "state",
                "test-state",
                2,
                &Doc {
                    n: 1,
                    name: "safe".into(),
                },
            )
            .unwrap();
        // Make the temp path a directory so the write fails.
        std::fs::create_dir(dir.path().join(".state.json.tmp")).unwrap();
        assert!(store
            .save(
                "state",
                "test-state",
                2,
                &Doc {
                    n: 2,
                    name: "lost".into()
                }
            )
            .is_err());
        let (loaded, _) = load(&store);
        assert_eq!(loaded.unwrap().name, "safe");
    }
}
