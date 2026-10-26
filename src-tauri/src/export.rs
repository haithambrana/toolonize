//! Saving a terminal's scrollback as a plain-text file.
//!
//! The frontend sends only the text and a display name. The core picks the
//! folder (the user's Downloads folder) and builds a sanitized, unique file
//! name, so the WebView can never choose a path or overwrite a file.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MAX_EXPORT_BYTES: usize = 32 * 1024 * 1024;

/// The user's Downloads folder: `XDG_DOWNLOAD_DIR` (Linux), the Downloads
/// Known Folder (Windows); falls back to `~/Downloads`, then home.
pub fn downloads_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(dir) = crate::discovery::windows::known_folder(
            &windows_sys::Win32::UI::Shell::FOLDERID_Downloads,
        ) {
            return Some(dir);
        }
    }
    #[cfg(not(windows))]
    {
        use crate::discovery::linux::{parse_user_dirs, LinuxEnv};
        let env = LinuxEnv::from_process();
        if let (Some(config), Some(home)) = (env.config_home(), env.home.as_deref()) {
            if let Ok(bytes) =
                crate::discovery::walk::read_bounded(&config.join("user-dirs.dirs"), 16 * 1024)
            {
                let text = String::from_utf8_lossy(&bytes);
                if let Some(dir) = parse_user_dirs(&text, home, "XDG_DOWNLOAD_DIR") {
                    if dir.is_dir() {
                        return Some(dir);
                    }
                }
            }
        }
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)?;
    let downloads = home.join("Downloads");
    Some(if downloads.is_dir() { downloads } else { home })
}

/// Letters, digits, `-` and `_` only; at most 40 characters.
fn slug(hint: &str) -> String {
    let mut out = String::new();
    for c in hint.chars() {
        if out.len() >= 40 {
            break;
        }
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "terminal".into()
    } else {
        out
    }
}

/// `YYYYMMDD-HHMMSS` (UTC) without a date library.
fn stamp(now_ms: u64) -> String {
    let secs = now_ms / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil-from-days (H. Hinnant), valid for the Unix era.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

pub fn export_text(
    dir: &Path,
    name_hint: &str,
    text: &str,
    now_ms: u64,
) -> Result<PathBuf, String> {
    if text.len() > MAX_EXPORT_BYTES {
        return Err("the output is larger than 32 MiB".into());
    }
    let base = format!("toolonize-{}-{}", slug(name_hint), stamp(now_ms));
    for n in 1..100 {
        let name = if n == 1 {
            format!("{base}.txt")
        } else {
            format!("{base}-{n}.txt")
        };
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(text.as_bytes())
                    .and_then(|_| file.sync_all())
                    .map_err(|e| format!("could not write {}: {e}", path.display()))?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("could not create a file in {}: {e}", dir.display())),
        }
    }
    Err("too many exports with the same name".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitized_and_unique() {
        let dir = tempfile::tempdir().unwrap();
        let t = 1_767_225_600_000; // 2026-01-01T00:00:00Z
        let a = export_text(dir.path(), "../../etc/passwd", "one", t).unwrap();
        let b = export_text(dir.path(), "../../etc/passwd", "two", t).unwrap();
        assert_eq!(a.parent().unwrap(), dir.path());
        assert_eq!(
            a.file_name().unwrap().to_string_lossy(),
            "toolonize-etc-passwd-20260101-000000.txt"
        );
        assert!(b.to_string_lossy().ends_with("-2.txt"));
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "two");
        assert_eq!(slug("Build server: tail -f"), "Build-server-tail-f");
        assert_eq!(slug("..."), "terminal");
    }

    #[test]
    fn size_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let big = "x".repeat(MAX_EXPORT_BYTES + 1);
        assert!(export_text(dir.path(), "t", &big, 0).is_err());
    }

    #[test]
    fn stamps() {
        assert_eq!(stamp(0), "19700101-000000");
        assert_eq!(stamp(951_782_400_000 + 3_723_000), "20000229-010203");
    }
}
