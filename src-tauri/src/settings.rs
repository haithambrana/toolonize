//! User settings (FR-053, FR-054). ToolOnize has no network features, so
//! there are no telemetry or update switches to opt into.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SETTINGS_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
    HighContrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionPreference {
    #[default]
    System,
    Reduced,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    #[default]
    Block,
    Underline,
    Bar,
}

/// Terminal color schemes the UI knows ("auto" follows the app theme).
pub const SCHEMES: &[&str] = &[
    "auto",
    "solarized_dark",
    "solarized_light",
    "dracula",
    "nord",
    "gruvbox_dark",
    "one_dark",
    "tango_light",
];

pub const MAX_SNIPPETS: usize = 100;
pub const MAX_SNIPPET_BYTES: usize = 8 * 1024;

/// A saved piece of text the user can paste into a terminal. It is pasted
/// like clipboard text (multi-line warning applies) and never executed by
/// ToolOnize itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub motion: MotionPreference,
    pub font_size: u8,
    /// xterm.js scrollback lines per terminal (NFR-003 cap).
    pub scrollback_lines: u32,
    /// Server-side replay window per session, in MiB.
    pub replay_buffer_mib: u8,
    /// Warn before pasting multi-line text (FR-033).
    pub paste_warning: bool,
    /// Launcher id preferred for "New terminal".
    pub default_shell: Option<String>,
    /// Action id -> key binding overrides (FR-054).
    pub keymap: BTreeMap<String, String>,
    /// Watch launcher roots for changes (FR-005).
    pub live_updates: bool,
    pub confirm_close_running: bool,
    /// Copy terminal selections to the clipboard as soon as they are made.
    pub copy_on_select: bool,
    /// Let programs in a terminal (incl. over SSH, e.g. tmux/vim) write to
    /// the local clipboard with OSC 52. Reading the clipboard is never allowed.
    pub osc52_clipboard: bool,
    /// Reconnect SSH members automatically when the connection drops
    /// (ssh exit status 255), with backoff and a bounded number of attempts.
    pub ssh_auto_reconnect: bool,
    /// CSS font-family list for terminals; empty = built-in default.
    pub terminal_font_family: String,
    pub cursor_style: CursorStyle,
    pub cursor_blink: bool,
    /// One of [`SCHEMES`].
    pub terminal_scheme: String,
    /// Desktop notifications when a terminal you are not looking at exits,
    /// disconnects or rings the bell.
    pub desktop_notifications: bool,
    pub snippets: Vec<Snippet>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            motion: MotionPreference::System,
            font_size: 13,
            scrollback_lines: 10_000,
            replay_buffer_mib: 4,
            paste_warning: true,
            default_shell: None,
            keymap: BTreeMap::new(),
            live_updates: true,
            confirm_close_running: true,
            copy_on_select: false,
            osc52_clipboard: true,
            ssh_auto_reconnect: true,
            terminal_font_family: String::new(),
            cursor_style: CursorStyle::Block,
            cursor_blink: true,
            terminal_scheme: "auto".into(),
            desktop_notifications: true,
            snippets: Vec::new(),
        }
    }
}

const ACTIONS: &[&str] = &[
    "palette",
    "workspace.next",
    "workspace.prev",
    "mode.grid",
    "mode.focus",
    "mode.tabs",
    "mode.master_stack",
    "terminal.search",
    "terminal.search_all",
    "notes.terminal",
    "notes.project",
    "terminal.split_right",
    "terminal.split_down",
    "terminal.zoom_in",
    "terminal.zoom_out",
    "terminal.zoom_reset",
    "terminal.broadcast",
    "terminal.snippets",
    "view.presentation",
    "terminal.copy",
    "terminal.paste",
    "terminal.new",
    "member.next",
    "member.prev",
    "member.rename",
    "terminal.select_all",
    "focus.toggle",
    "view.launchers",
    "view.settings",
];

fn valid_binding(binding: &str) -> bool {
    if binding.is_empty() || binding.len() > 40 {
        return false;
    }
    let parts: Vec<&str> = binding.split('+').collect();
    let (key, mods) = parts.split_last().expect("split yields one part");
    mods.iter()
        .all(|m| matches!(*m, "Ctrl" | "Shift" | "Alt" | "Meta"))
        && !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "[]\\;',./`-=".contains(c))
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(8..=32).contains(&self.font_size) {
            return Err("font size must be between 8 and 32".into());
        }
        if !(500..=200_000).contains(&self.scrollback_lines) {
            return Err("scrollback must be between 500 and 200000 lines".into());
        }
        if !(2..=64).contains(&self.replay_buffer_mib) {
            return Err("replay buffer must be between 2 and 64 MiB".into());
        }
        if self.keymap.len() > ACTIONS.len() {
            return Err("too many key bindings".into());
        }
        for (action, binding) in &self.keymap {
            if !ACTIONS.contains(&action.as_str()) {
                return Err(format!("unknown action {action}"));
            }
            if !valid_binding(binding) {
                return Err(format!("invalid key binding {binding}"));
            }
        }
        let mut seen = std::collections::HashSet::new();
        for binding in self.keymap.values() {
            if !seen.insert(binding) {
                return Err(format!("{binding} is bound to more than one action"));
            }
        }
        if self.terminal_font_family.len() > 200
            || !self
                .terminal_font_family
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || " ,.'\"_-".contains(c))
        {
            return Err(
                "font family may contain letters, digits, spaces, commas and quotes".into(),
            );
        }
        if !SCHEMES.contains(&self.terminal_scheme.as_str()) {
            return Err(format!("unknown color scheme {}", self.terminal_scheme));
        }
        if self.snippets.len() > MAX_SNIPPETS {
            return Err(format!("at most {MAX_SNIPPETS} snippets"));
        }
        for s in &self.snippets {
            let name = s.name.trim();
            if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
                return Err("snippet names are 1-60 characters on one line".into());
            }
            if s.text.is_empty() || s.text.len() > MAX_SNIPPET_BYTES || s.text.contains('\0') {
                return Err(format!(
                    "snippet “{name}” must hold 1 byte to {} KiB of text",
                    MAX_SNIPPET_BYTES / 1024
                ));
            }
        }
        if let Some(shell) = &self.default_shell {
            if !crate::model::valid_id(shell) {
                return Err("invalid default shell".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn rejects_out_of_range_and_bad_bindings() {
        let mut s = Settings {
            font_size: 2,
            ..Default::default()
        };
        assert!(s.validate().is_err());
        s.font_size = 13;
        s.keymap.insert("palette".into(), "Ctrl+Shift+P".into());
        assert!(s.validate().is_ok());
        s.keymap.insert("mode.grid".into(), "Ctrl+Shift+P".into());
        assert!(s.validate().is_err(), "conflict");
        s.keymap.remove("mode.grid");
        s.keymap.insert("exec".into(), "Ctrl+E".into());
        assert!(s.validate().is_err(), "unknown action");
        s.keymap.remove("exec");
        s.keymap.insert("mode.grid".into(), "Hyper+G".into());
        assert!(s.validate().is_err());
    }

    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = serde_json::from_str("{\"font_size\": 15}").unwrap();
        assert_eq!(s.font_size, 15);
        assert!(s.paste_warning);
    }

    #[test]
    fn appearance_and_snippets_are_validated() {
        let ok = Settings {
            terminal_font_family: "\"Fira Code\", monospace".into(),
            terminal_scheme: "dracula".into(),
            snippets: vec![Snippet {
                name: "Tail logs".into(),
                text: "tail -f /var/log/syslog".into(),
            }],
            ..Default::default()
        };
        assert!(ok.validate().is_ok());
        let bad_font = Settings {
            terminal_font_family: "x; background:url(evil)".into(),
            ..Default::default()
        };
        assert!(bad_font.validate().is_err());
        let bad_scheme = Settings {
            terminal_scheme: "neon".into(),
            ..Default::default()
        };
        assert!(bad_scheme.validate().is_err());
        let bad_snippet = Settings {
            snippets: vec![Snippet {
                name: "two\nlines".into(),
                text: "x".into(),
            }],
            ..Default::default()
        };
        assert!(bad_snippet.validate().is_err());
        let too_long = Settings {
            snippets: vec![Snippet {
                name: "big".into(),
                text: "x".repeat(MAX_SNIPPET_BYTES + 1),
            }],
            ..Default::default()
        };
        assert!(too_long.validate().is_err());
    }
}
