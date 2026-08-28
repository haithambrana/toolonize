pub mod ping;
#[cfg(feature = "spike")]
pub mod spike;
pub mod terminal;

pub use ping::ping;

/// Explicit registry of allowed production commands.
/// M3 extends M1 with terminal lifecycle commands (typed, capability-scoped).
/// No generic exec / raw shell string commands are ever registered.
pub const ALLOWED_COMMANDS: &[&str] = &[
    "ping",
    "terminal_profiles",
    "terminal_start",
    "terminal_list",
    "terminal_attach",
    "terminal_detach",
    "terminal_hide",
    "terminal_show",
    "terminal_write",
    "terminal_resize",
    "terminal_ack",
    "terminal_close",
    "terminal_restart",
    "terminal_poll",
    "terminal_replay",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_commands_are_expected() {
        assert!(ALLOWED_COMMANDS.contains(&"ping"));
        assert!(ALLOWED_COMMANDS.contains(&"terminal_profiles"));
        assert!(ALLOWED_COMMANDS.contains(&"terminal_start"));
        assert!(ALLOWED_COMMANDS.contains(&"terminal_list"));
        assert!(ALLOWED_COMMANDS.contains(&"terminal_write"));
        assert!(ALLOWED_COMMANDS.contains(&"terminal_resize"));
        assert!(ALLOWED_COMMANDS.contains(&"terminal_ack"));
        assert!(ALLOWED_COMMANDS.contains(&"terminal_close"));
        assert!(ALLOWED_COMMANDS.contains(&"terminal_restart"));
        // Exactly 15 commands (1 ping + 14 terminal)
        assert_eq!(ALLOWED_COMMANDS.len(), 15);
    }

    #[test]
    fn unknown_command_not_in_allowlist() {
        assert!(!ALLOWED_COMMANDS.contains(&"unknown"));
        assert!(!ALLOWED_COMMANDS.contains(&"exec"));
        assert!(!ALLOWED_COMMANDS.contains(&"shell"));
        assert!(!ALLOWED_COMMANDS.contains(&"open"));
        assert!(!ALLOWED_COMMANDS.contains(&"raw_exec"));
    }

    #[test]
    fn no_raw_execution_commands() {
        for cmd in ALLOWED_COMMANDS {
            assert!(!cmd.contains("exec"), "raw exec leaked: {cmd}");
            assert!(!cmd.contains("shell"), "shell exec leaked: {cmd}");
        }
    }
}
