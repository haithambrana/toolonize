pub mod direct;
pub mod portable;

use anyhow::Result;
use std::collections::HashMap;

/// Trait isolating PTY backend choice (ADR-004).
pub trait PtyBackend: Send {
    fn name(&self) -> &'static str;
    /// Spawn a child in a PTY. Returns handle.
    fn spawn(
        &mut self,
        cmd: &str,
        args: &[&str],
        rows: u16,
        cols: u16,
    ) -> Result<Box<dyn PtyHandle>>;

    /// Spawn with invalid executable to test error handling.
    fn spawn_invalid(&mut self) -> Result<Box<dyn PtyHandle>> {
        self.spawn("/nonexistent/invalid_executable_xyz_12345", &[], 24, 80)
    }
}

pub trait PtyHandle: Send {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn write(&mut self, data: &[u8]) -> Result<usize>;
    fn resize(&mut self, rows: u16, cols: u16) -> Result<()>;
    fn get_size(&self) -> Result<(u16, u16)>;
    fn kill(&mut self) -> Result<()>;
    fn wait(&mut self) -> Result<Option<i32>>; // exit code if exited
    fn is_alive(&mut self) -> bool;
    fn backend_name(&self) -> &'static str;
}

/// Simple registry for comparative testing.
pub fn all_backends() -> Vec<Box<dyn PtyBackend>> {
    let mut v: Vec<Box<dyn PtyBackend>> = Vec::new();
    v.push(Box::new(portable::PortableBackend::new()));
    v.push(Box::new(direct::DirectBackend::new()));
    v
}

/// Result for a single backend + scenario.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScenarioResult {
    pub backend: String,
    pub scenario: String,
    pub status: String, // PASS, FAIL, BLOCKED, NOT_VERIFIED
    pub details: String,
    pub duration_ms: u128,
    pub extra: HashMap<String, String>,
}
