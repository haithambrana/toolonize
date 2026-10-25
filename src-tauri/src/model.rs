//! Shared domain types (DOMAIN_MODEL.md). Pure data: nothing here executes.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Linux,
    Windows,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    XdgUser,
    XdgSystem,
    Flatpak,
    Snap,
    DesktopDir,
    CustomRoot,
    ProgramsUser,
    ProgramsCommon,
    DesktopUser,
    DesktopPublic,
    Synthetic,
}

impl OriginKind {
    pub fn label(self) -> &'static str {
        match self {
            OriginKind::XdgUser => "User applications",
            OriginKind::XdgSystem => "System applications",
            OriginKind::Flatpak => "Flatpak export",
            OriginKind::Snap => "Snap export",
            OriginKind::DesktopDir => "Desktop folder",
            OriginKind::CustomRoot => "Custom root",
            OriginKind::ProgramsUser => "Start Menu (user)",
            OriginKind::ProgramsCommon => "Start Menu (all users)",
            OriginKind::DesktopUser => "Desktop (user)",
            OriginKind::DesktopPublic => "Desktop (public)",
            OriginKind::Synthetic => "Built-in shell",
        }
    }

    /// Locations any user process can write to without the package system:
    /// entries there default to Needs Review (T-DISC-07).
    pub fn conservative(self) -> bool {
        matches!(
            self,
            OriginKind::DesktopDir
                | OriginKind::CustomRoot
                | OriginKind::DesktopUser
                | OriginKind::DesktopPublic
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptorKind {
    ExecTemplate,
    ShellLink,
    DbusActivatable,
    SyntheticSystem,
}

/// Parsed, execution-relevant metadata of a launcher. Built only by adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Descriptor {
    pub kind: DescriptorKind,
    /// Executable path or bare name (resolved via PATH at launch time).
    pub program: Option<String>,
    /// Discrete argv after the program (Linux Exec expansion / synthetic).
    pub args: Vec<String>,
    /// Windows Shell Link command-line argument string, passed through
    /// unmodified; `None` elsewhere.
    pub raw_args: Option<String>,
    /// Original Exec template (display + fingerprint).
    pub exec_template: Option<String>,
    pub working_dir: Option<String>,
    /// Desktop Entry `Terminal=true`.
    pub terminal: bool,
    pub try_exec: Option<String>,
    pub icon: Option<String>,
    /// Bare program name: resolution happens via PATH at launch (T-DISC-06).
    pub path_resolved: bool,
    /// The descriptor file this came from (empty for synthetic launchers).
    pub source_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LauncherType {
    EmbeddedTerminal,
    LocalCommand,
    RemoteSsh,
    ExternalGui,
    Unknown,
}

impl LauncherType {
    pub fn runs_in_pty(self) -> bool {
        matches!(
            self,
            LauncherType::EmbeddedTerminal | LauncherType::LocalCommand | LauncherType::RemoteSsh
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Classification {
    /// What the classifier believes this launcher is.
    pub proposed: LauncherType,
    pub confidence: Confidence,
    pub evidence: Vec<String>,
    /// Explicit interpreter execution (shell -c, cmd /c, .ps1 ...): a
    /// higher-power launcher the review UI must call out.
    pub interpreter: bool,
    /// A human must review this exact descriptor before it can execute.
    pub needs_review: bool,
    /// Malformed/unsupported: can never be approved.
    pub blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseStatus {
    Ok,
    NeedsReview,
    Unavailable,
}

/// Raw output of an adapter, before classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLauncher {
    pub launcher_id: String,
    pub display_name: String,
    pub platform: Platform,
    pub origin: OriginKind,
    pub origin_root: String,
    pub relative_path: String,
    pub descriptor: Descriptor,
    pub parse_status: ParseStatus,
    pub issues: Vec<String>,
    pub no_display: bool,
    pub not_shown_here: bool,
    pub comment: Option<String>,
}

/// A normalized, classified registry record (inert data).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launcher {
    pub launcher_id: String,
    pub display_name: String,
    pub platform: Platform,
    pub origin: OriginKind,
    pub origin_label: String,
    pub origin_root: String,
    pub relative_path: String,
    pub descriptor: Descriptor,
    pub fingerprint: String,
    pub parse_status: ParseStatus,
    pub issues: Vec<String>,
    pub classification: Classification,
    pub no_display: bool,
    pub not_shown_here: bool,
    pub comment: Option<String>,
}

/// Hash over every execution-relevant field (FR-015). Any change here
/// suspends prior authorization.
pub fn descriptor_fingerprint(origin: OriginKind, launcher_id: &str, d: &Descriptor) -> String {
    #[derive(Serialize)]
    struct Covered<'a> {
        v: u8,
        launcher_id: &'a str,
        origin: OriginKind,
        kind: DescriptorKind,
        program: &'a Option<String>,
        args: &'a [String],
        raw_args: &'a Option<String>,
        exec_template: &'a Option<String>,
        working_dir: &'a Option<String>,
        terminal: bool,
        source_path: &'a str,
    }
    let covered = Covered {
        v: 1,
        launcher_id,
        origin,
        kind: d.kind,
        program: &d.program,
        args: &d.args,
        raw_args: &d.raw_args,
        exec_template: &d.exec_template,
        working_dir: &d.working_dir,
        terminal: d.terminal,
        source_path: &d.source_path,
    };
    let bytes = serde_json::to_vec(&covered).expect("fingerprint input serializes");
    hex(&Sha256::digest(bytes))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Opaque random-looking identifier (no machine data).
pub fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let digest = Sha256::digest(format!("{prefix}:{nanos}:{n}:{}", std::process::id()));
    format!("{prefix}_{}", &hex(&digest)[..16])
}

/// Identifier grammar for ids crossing the IPC boundary.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 512
        && !id.contains("..")
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-.:/@+ ".contains(c) || !c.is_ascii())
        && !id.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> Descriptor {
        Descriptor {
            kind: DescriptorKind::ExecTemplate,
            program: Some("example-tool".into()),
            args: vec!["--flag".into()],
            raw_args: None,
            exec_template: Some("example-tool --flag %U".into()),
            working_dir: None,
            terminal: false,
            try_exec: None,
            icon: Some("example".into()),
            path_resolved: true,
            source_path: "/fixture/applications/org.example.Tool.desktop".into(),
        }
    }

    #[test]
    fn fingerprint_covers_exec_but_not_icon() {
        let base = descriptor();
        let fp = descriptor_fingerprint(OriginKind::XdgUser, "id", &base);
        let mut icon = base.clone();
        icon.icon = Some("other".into());
        assert_eq!(fp, descriptor_fingerprint(OriginKind::XdgUser, "id", &icon));
        let mut exec = base.clone();
        exec.args.push("--evil".into());
        assert_ne!(fp, descriptor_fingerprint(OriginKind::XdgUser, "id", &exec));
        let mut cwd = base.clone();
        cwd.working_dir = Some("/tmp".into());
        assert_ne!(fp, descriptor_fingerprint(OriginKind::XdgUser, "id", &cwd));
        assert_ne!(
            fp,
            descriptor_fingerprint(OriginKind::XdgSystem, "id", &base)
        );
    }

    #[test]
    fn ids_are_unique_and_valid() {
        let a = new_id("ws");
        let b = new_id("ws");
        assert_ne!(a, b);
        assert!(valid_id(&a));
        assert!(!valid_id("../etc"));
        assert!(!valid_id(""));
        assert!(!valid_id("a\nb"));
    }
}
