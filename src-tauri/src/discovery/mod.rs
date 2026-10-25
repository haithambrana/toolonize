//! Launcher discovery: read-only metadata pipelines per platform.
//!
//! Discovery never executes, resolves scripts, or touches the network
//! (FR-004). Adapters return raw records; classification and registry
//! diffing happen in shared code.

pub mod exec;
pub mod keyfile;
pub mod linux;
pub mod lnk;
pub mod walk;
pub mod watch;
pub mod windows;

use crate::model::{hex, OriginKind, RawLauncher};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceReport {
    pub origin: OriginKind,
    pub root: String,
    pub entries: usize,
    pub truncated: bool,
    pub errors: Vec<String>,
}

/// Short stable tag for a root path, used to scope custom-root identities.
pub fn root_tag(root: &Path) -> String {
    hex(&Sha256::digest(root.to_string_lossy().as_bytes()))[..10].to_string()
}

#[derive(Debug, Clone, Default)]
pub struct DiscoveryConfig {
    pub custom_roots: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct ScanOutput {
    pub launchers: Vec<RawLauncher>,
    pub reports: Vec<SourceReport>,
}

/// One full scan of every source on this platform.
pub fn scan_all(config: &DiscoveryConfig) -> ScanOutput {
    let mut out = ScanOutput::default();
    if cfg!(windows) {
        for (origin, root) in windows::known_folder_roots() {
            let (found, report) = windows::scan_lnk_root(
                &root,
                origin,
                windows::LNK_LIMITS,
                &windows::fs_probe,
                &windows::process_env,
            );
            out.launchers.extend(found);
            out.reports.push(report);
        }
        for root in &config.custom_roots {
            let (found, report) = windows::scan_lnk_root(
                root,
                OriginKind::CustomRoot,
                linux::CUSTOM_LIMITS,
                &windows::fs_probe,
                &windows::process_env,
            );
            out.launchers.extend(found);
            out.reports.push(report);
        }
    } else {
        let env = linux::LinuxEnv::from_process();
        let (apps, reports) = linux::scan_applications(&env);
        out.launchers.extend(apps);
        out.reports.extend(reports);
        if let Some(desktop) = linux::desktop_dir(&env) {
            let (found, report) = linux::scan_loose_root(
                &env,
                &desktop,
                OriginKind::DesktopDir,
                linux::CUSTOM_LIMITS,
            );
            out.launchers.extend(found);
            out.reports.push(report);
        }
        for root in &config.custom_roots {
            let (found, report) =
                linux::scan_loose_root(&env, root, OriginKind::CustomRoot, linux::CUSTOM_LIMITS);
            out.launchers.extend(found);
            out.reports.push(report);
        }
    }
    out
}

/// Directories the change watcher should observe on this platform.
pub fn watch_roots(config: &DiscoveryConfig) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(windows) {
        roots.extend(windows::known_folder_roots().into_iter().map(|(_, p)| p));
    } else {
        let env = linux::LinuxEnv::from_process();
        roots.extend(linux::application_roots(&env).into_iter().map(|r| r.path));
        if let Some(desktop) = linux::desktop_dir(&env) {
            roots.push(desktop);
        }
    }
    roots.extend(config.custom_roots.iter().cloned());
    roots
}

/// Validate a user-declared custom root (FR-008): absolute, existing
/// directory, not a filesystem root or the whole home directory.
pub fn validate_custom_root(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    if !p.is_absolute() {
        return Err("custom root must be an absolute path".into());
    }
    let canonical = std::fs::canonicalize(&p).map_err(|e| format!("cannot open folder: {e}"))?;
    if !canonical.is_dir() {
        return Err("custom root must be a folder".into());
    }
    if canonical.parent().is_none() || canonical.components().count() <= 1 {
        return Err("a filesystem root cannot be a launcher root".into());
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .and_then(|h| std::fs::canonicalize(h).ok());
    if home.as_deref() == Some(canonical.as_path()) {
        return Err("choose a specific folder, not the whole home directory".into());
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_root_validation() {
        assert!(validate_custom_root("relative/path").is_err());
        assert!(validate_custom_root("/").is_err());
        let dir = tempfile::tempdir().unwrap();
        assert!(validate_custom_root(&dir.path().to_string_lossy()).is_ok());
        let file = dir.path().join("f");
        std::fs::write(&file, "").unwrap();
        assert!(validate_custom_root(&file.to_string_lossy()).is_err());
    }

    #[test]
    fn root_tags_are_stable_and_distinct() {
        assert_eq!(root_tag(Path::new("/a")), root_tag(Path::new("/a")));
        assert_ne!(root_tag(Path::new("/a")), root_tag(Path::new("/b")));
    }
}
