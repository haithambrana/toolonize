pub mod ping;
#[cfg(feature = "spike")]
pub mod spike;

pub use ping::ping;

/// Explicit registry of allowed M1 custom commands.
/// This list is the single source of truth for the capability surface.
///
/// M1 must expose exactly one custom command: `ping` (semantic `app::ping`).
pub const ALLOWED_COMMANDS: &[&str] = &["ping"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_commands_contains_only_ping() {
        assert_eq!(ALLOWED_COMMANDS, &["ping"]);
        assert_eq!(ALLOWED_COMMANDS.len(), 1);
    }

    #[test]
    fn unknown_command_not_in_allowlist() {
        assert!(!ALLOWED_COMMANDS.contains(&"unknown"));
        assert!(!ALLOWED_COMMANDS.contains(&"exec"));
        assert!(!ALLOWED_COMMANDS.contains(&"shell"));
        assert!(!ALLOWED_COMMANDS.contains(&"open"));
    }
}
