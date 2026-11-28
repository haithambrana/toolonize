//! Local diagnostic log (`<state dir>/toolonize.log`, rotated at 1 MiB, one
//! previous file kept). It records what happened (startup, scans, session
//! state changes, errors), never what was typed or shown: no command
//! arguments, environment, terminal output, clipboard or note contents — the
//! same rule as the audit log. Home paths are written as `~`.

use crate::sync::Locked;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_BYTES: u64 = 1024 * 1024;
const MAX_LINE: usize = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

struct Logger {
    path: PathBuf,
    home: Option<String>,
    lock: Mutex<()>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Start logging into `dir`. Later calls are ignored (one log per process).
pub fn init(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .ok()
        .filter(|h| h.len() > 1);
    let _ = LOGGER.set(Logger {
        path: dir.join("toolonize.log"),
        home,
        lock: Mutex::new(()),
    });
}

pub fn log_path() -> Option<PathBuf> {
    LOGGER.get().map(|l| l.path.clone())
}

/// One line, control characters removed, home folder shown as `~`, capped.
pub fn sanitize(message: &str, home: Option<&str>) -> String {
    let mut text: String = message
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if let Some(home) = home {
        text = text.replace(home, "~");
    }
    if text.len() > MAX_LINE {
        let mut cut = MAX_LINE;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push('…');
    }
    text
}

pub fn write(level: Level, component: &str, message: &str) {
    let Some(logger) = LOGGER.get() else { return };
    let _guard = logger.lock.locked();
    if fs::metadata(&logger.path).map(|m| m.len()).unwrap_or(0) > MAX_BYTES {
        let _ = fs::rename(&logger.path, logger.path.with_extension("log.1"));
    }
    let line = format!(
        "{} {} [{}] {}\n",
        crate::model::now_ms(),
        level.as_str(),
        component,
        sanitize(message, logger.home.as_deref())
    );
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&logger.path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

pub fn info(component: &str, message: &str) {
    write(Level::Info, component, message);
}

pub fn warn(component: &str, message: &str) {
    write(Level::Warn, component, message);
}

pub fn error(component: &str, message: &str) {
    write(Level::Error, component, message);
}

/// The last `max` lines of the current (and, if needed, previous) log file.
pub fn tail(max: usize) -> Vec<String> {
    let Some(logger) = LOGGER.get() else {
        return Vec::new();
    };
    let _guard = logger.lock.locked();
    let mut lines: Vec<String> = Vec::new();
    for path in [logger.path.with_extension("log.1"), logger.path.clone()] {
        if let Ok(text) = fs::read_to_string(&path) {
            lines.extend(text.lines().map(str::to_string));
        }
    }
    let start = lines.len().saturating_sub(max);
    lines.split_off(start)
}

/// Record panics in the log before the default hook runs.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        error("panic", &format!("{payload} at {location}"));
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_single_redacted_and_bounded() {
        let s = sanitize("open /home/alice/x\nnext\x1b[31m", Some("/home/alice"));
        assert_eq!(s, "open ~/x next [31m");
        let long = sanitize(&"é".repeat(1000), None);
        assert!(long.len() <= MAX_LINE + '…'.len_utf8());
        assert!(long.ends_with('…'));
    }
}
