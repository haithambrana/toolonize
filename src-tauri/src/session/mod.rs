//! Terminal sessions: process lifecycle independent of any view (FR-031).

mod manager;
pub mod output;
pub mod state;

pub use manager::{
    AttachInfo, Listener, SessionError, SessionEvent, SessionInfo, SessionManager, SessionPlan,
    SessionSink, MAX_WRITE,
};
pub use output::FlowLimits;
pub use state::{ProcessState, ViewEvent, ViewState};

/// Current working folder of a local process, where the OS exposes it.
/// Linux reads `/proc/<pid>/cwd`; other platforms return None (the frontend
/// then relies on OSC 7 reports from the shell).
pub fn process_cwd(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_link(format!("/proc/{pid}/cwd"))
            .ok()
            .filter(|p| p.is_absolute())
            .map(|p| p.to_string_lossy().into_owned())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

#[cfg(all(test, target_os = "linux"))]
mod cwd_tests {
    #[test]
    fn reads_the_folder_of_a_running_process() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .current_dir(dir.path())
            .spawn()
            .unwrap();
        let cwd = super::process_cwd(child.id());
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(
            cwd.map(std::path::PathBuf::from)
                .map(|p| p.canonicalize().unwrap()),
            Some(dir.path().canonicalize().unwrap())
        );
        assert_eq!(super::process_cwd(u32::MAX), None);
    }
}
