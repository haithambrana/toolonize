//! Local, redacted audit log of execution decisions (SEC-006, T-SEC-007).
//!
//! Records who/what/outcome for start, launch, review and authorization
//! decisions. Never records argv, environment, or terminal output; absolute
//! paths inside reasons are replaced with `<path>`.

use crate::model::now_ms;
use crate::persist::append_line;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

const RECENT: usize = 500;
const MAX_LOG_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Allowed,
    Denied,
    Failed,
    Recorded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditEntry {
    pub at: u64,
    pub action: String,
    pub outcome: Outcome,
    pub workspace_id: Option<String>,
    pub member_id: Option<String>,
    pub launcher_id: Option<String>,
    pub fingerprint: Option<String>,
    pub detail: Option<String>,
}

/// Replace absolute-path-looking tokens with `<path>`.
pub fn redact(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let w = word.trim_matches(|c: char| "()[]{}'\",;".contains(c));
            let looks_like_path = w.starts_with('/')
                || w.starts_with("\\\\")
                || (w.len() > 2
                    && w.as_bytes()[1] == b':'
                    && (w.as_bytes()[2] == b'\\' || w.as_bytes()[2] == b'/'));
            if looks_like_path {
                word.replace(w, "<path>")
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub struct Audit {
    path: Option<PathBuf>,
    recent: Mutex<VecDeque<AuditEntry>>,
}

#[derive(Default)]
pub struct Record<'a> {
    pub workspace_id: Option<&'a str>,
    pub member_id: Option<&'a str>,
    pub launcher_id: Option<&'a str>,
    pub fingerprint: Option<&'a str>,
    pub detail: Option<&'a str>,
}

impl Audit {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            recent: Mutex::new(VecDeque::new()),
        }
    }

    pub fn record(&self, action: &str, outcome: Outcome, r: Record<'_>) {
        let entry = AuditEntry {
            at: now_ms(),
            action: action.to_string(),
            outcome,
            workspace_id: r.workspace_id.map(str::to_string),
            member_id: r.member_id.map(str::to_string),
            launcher_id: r.launcher_id.map(redact),
            fingerprint: r.fingerprint.map(|f| f.chars().take(12).collect()),
            detail: r.detail.map(redact),
        };
        if let Some(path) = &self.path {
            if let Ok(line) = serde_json::to_string(&entry) {
                let _ = append_line(path, &line, MAX_LOG_BYTES);
            }
        }
        let mut recent = self.recent.lock().unwrap();
        if recent.len() >= RECENT {
            recent.pop_front();
        }
        recent.push_back(entry);
    }

    pub fn recent(&self, limit: usize) -> Vec<AuditEntry> {
        let recent = self.recent.lock().unwrap();
        recent.iter().rev().take(limit).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_paths() {
        assert_eq!(
            redact("working directory does not exist: /srv/example/private"),
            "working directory does not exist: <path>"
        );
        assert_eq!(
            redact("C:\\Users\\Example\\x.exe missing"),
            "<path> missing"
        );
        assert_eq!(redact("plain reason"), "plain reason");
    }

    #[test]
    fn log_never_contains_secret_env_t_sec_007() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.log");
        let audit = Audit::new(Some(path.clone()));
        std::env::set_var("TOOLONIZE_FIXTURE_SECRET", "dummy-secret-value-123");
        audit.record(
            "session.start",
            Outcome::Denied,
            Record {
                workspace_id: Some("ws_1"),
                member_id: Some("m1"),
                launcher_id: Some("linux:app:org.example.Tool.desktop"),
                fingerprint: Some("0123456789abcdef0123"),
                detail: Some("not authorized"),
            },
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("dummy-secret-value-123"));
        assert!(text.contains("\"fingerprint\":\"0123456789ab\""));
        assert_eq!(audit.recent(10).len(), 1);
    }
}
