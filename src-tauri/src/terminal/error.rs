use serde::Serialize;

/// Sanitized error exposed to the frontend. Never contains local paths,
/// usernames, hostnames, or credential material.
#[derive(Debug, Clone, Serialize)]
pub struct TerminalError {
    pub code: String,
    pub message: String,
}

impl TerminalError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::new("INVALID_INPUT", msg)
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", msg)
    }

    pub fn backend(msg: impl Into<String>) -> Self {
        Self::new("BACKEND_ERROR", msg)
    }

    pub fn transport(msg: impl Into<String>) -> Self {
        Self::new("TRANSPORT_ERROR", msg)
    }

    pub fn illegal_transition(msg: impl Into<String>) -> Self {
        Self::new("ILLEGAL_TRANSITION", msg)
    }

    /// Map anyhow / std errors to sanitized forms — strip any path-like content.
    pub fn from_internal(_err: impl std::fmt::Display) -> Self {
        // Intentionally do not leak internal error strings that may contain paths.
        Self::new("INTERNAL_ERROR", "terminal operation failed")
    }
}

impl std::fmt::Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for TerminalError {}

/// Result alias used throughout the terminal subsystem.
pub type TerminalResult<T> = Result<T, TerminalError>;
