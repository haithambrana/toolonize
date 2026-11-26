//! Sticky notes: one note per terminal member and one project note per
//! workspace (ADR-012). Notes are the user's own text and stay on this
//! machine: they are never written to the audit log, the diagnostic log or
//! workspace exports.
//!
//! Durability: each workspace's notes live in their own file, saved through
//! [`crate::persist::Store`] (temp file, fsync, atomic rename, fsync of the
//! folder, checksummed generations), so a power cut leaves either the new or
//! the previous version — never a torn file. Text is never silently dropped:
//! removing a member moves its note into the project note, and deleting a
//! workspace moves its notes into a bounded "recently deleted" list.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const NOTES_FORMAT: &str = "toolonize-notes";
pub const NOTES_SCHEMA: u32 = 1;
pub const TRASH_FORMAT: &str = "toolonize-notes-trash";
pub const MAX_NOTE_BYTES: usize = 256 * 1024;
pub const MAX_WORKSPACE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TRASH: usize = 50;
pub const TRASH_DAYS: u64 = 30;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub text: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceNotes {
    #[serde(default)]
    pub project: Note,
    /// member id -> note
    #[serde(default)]
    pub members: BTreeMap<String, Note>,
}

impl WorkspaceNotes {
    pub fn is_empty(&self) -> bool {
        self.project.text.is_empty() && self.members.values().all(|n| n.text.is_empty())
    }

    fn total_bytes(&self) -> usize {
        self.project.text.len() + self.members.values().map(|n| n.text.len()).sum::<usize>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashedMember {
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashEntry {
    pub id: String,
    pub workspace_name: String,
    pub deleted_at: u64,
    pub project: String,
    pub members: Vec<TrashedMember>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trash {
    #[serde(default)]
    pub entries: Vec<TrashEntry>,
}

impl Trash {
    /// Drop entries older than [`TRASH_DAYS`] and keep at most [`MAX_TRASH`].
    pub fn prune(&mut self, now_ms: u64) {
        let max_age = TRASH_DAYS * 24 * 3600 * 1000;
        self.entries
            .retain(|e| now_ms.saturating_sub(e.deleted_at) <= max_age);
        self.entries
            .sort_by_key(|e| std::cmp::Reverse(e.deleted_at));
        self.entries.truncate(MAX_TRASH);
    }
}

/// Note text as stored: NUL removed (the only character we refuse), size
/// bounded.
pub fn clean_text(text: &str) -> Result<String, String> {
    let text: String = text.chars().filter(|&c| c != '\0').collect();
    if text.len() > MAX_NOTE_BYTES {
        return Err(format!(
            "a note can hold at most {} KiB",
            MAX_NOTE_BYTES / 1024
        ));
    }
    Ok(text)
}

/// Set (or clear, with empty text) one note. `member` None = project note.
pub fn set_note(
    notes: &mut WorkspaceNotes,
    member: Option<&str>,
    text: String,
    now_ms: u64,
) -> Result<u64, String> {
    let mut next = notes.clone();
    match member {
        None => {
            next.project = Note {
                text,
                updated_at: now_ms,
            }
        }
        Some(id) if text.is_empty() => {
            next.members.remove(id);
        }
        Some(id) => {
            next.members.insert(
                id.to_string(),
                Note {
                    text,
                    updated_at: now_ms,
                },
            );
        }
    }
    if next.total_bytes() > MAX_WORKSPACE_BYTES {
        return Err(format!(
            "the notes of one workspace can hold at most {} MiB",
            MAX_WORKSPACE_BYTES / 1024 / 1024
        ));
    }
    *notes = next;
    Ok(now_ms)
}

/// A removed member's note is appended to the project note instead of
/// disappearing. Returns true when something moved.
pub fn move_member_note_to_project(
    notes: &mut WorkspaceNotes,
    member: &str,
    title: &str,
    now_ms: u64,
) -> bool {
    let Some(note) = notes.members.remove(member) else {
        return false;
    };
    if note.text.trim().is_empty() {
        return false;
    }
    let mut project = notes.project.text.clone();
    if !project.is_empty() && !project.ends_with('\n') {
        project.push('\n');
    }
    if !project.is_empty() {
        project.push('\n');
    }
    project.push_str(&format!("— Note from the removed terminal “{title}” —\n"));
    project.push_str(&note.text);
    notes.project = Note {
        text: project,
        updated_at: now_ms,
    };
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_clear_and_bounds() {
        let mut n = WorkspaceNotes::default();
        set_note(&mut n, Some("m1"), "deploy with --dry-run first".into(), 5).unwrap();
        set_note(&mut n, None, "release on Thursday".into(), 6).unwrap();
        assert_eq!(n.members["m1"].updated_at, 5);
        set_note(&mut n, Some("m1"), String::new(), 7).unwrap();
        assert!(!n.members.contains_key("m1"));
        assert!(clean_text(&"x".repeat(MAX_NOTE_BYTES + 1)).is_err());
        assert_eq!(clean_text("a\0b").unwrap(), "ab");
        // The per-workspace cap is enforced without changing what was there.
        let big = "y".repeat(MAX_NOTE_BYTES);
        for i in 0..15 {
            set_note(&mut n, Some(&format!("m{i}")), big.clone(), 8).unwrap();
        }
        let before = n.clone();
        assert!(set_note(&mut n, Some("m99"), big.clone(), 9).is_err());
        assert_eq!(n, before);
    }

    #[test]
    fn removed_member_note_moves_to_project() {
        let mut n = WorkspaceNotes::default();
        set_note(&mut n, None, "project plan".into(), 1).unwrap();
        set_note(
            &mut n,
            Some("m1"),
            "server password is in the vault".into(),
            2,
        )
        .unwrap();
        assert!(move_member_note_to_project(&mut n, "m1", "Build server", 3));
        assert!(n.members.is_empty());
        assert!(n
            .project
            .text
            .starts_with("project plan\n\n— Note from the removed terminal “Build server” —\n"));
        assert!(n.project.text.ends_with("server password is in the vault"));
        assert!(!move_member_note_to_project(&mut n, "m1", "x", 4));
    }

    #[test]
    fn trash_is_bounded_by_age_and_count() {
        let day = 24 * 3600 * 1000;
        let mut t = Trash::default();
        for i in 0..60u64 {
            t.entries.push(TrashEntry {
                id: format!("t{i}"),
                workspace_name: "W".into(),
                deleted_at: 100 * day + i,
                project: "p".into(),
                members: vec![],
            });
        }
        t.entries.push(TrashEntry {
            id: "old".into(),
            workspace_name: "W".into(),
            deleted_at: 1,
            project: "p".into(),
            members: vec![],
        });
        t.prune(100 * day + 100);
        assert_eq!(t.entries.len(), MAX_TRASH);
        assert!(t.entries.iter().all(|e| e.id != "old"));
        assert_eq!(t.entries[0].id, "t59", "newest first");
    }
}
