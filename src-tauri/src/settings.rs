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
    "terminal.copy",
    "terminal.paste",
    "terminal.new",
    "member.next",
    "member.prev",
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
}
