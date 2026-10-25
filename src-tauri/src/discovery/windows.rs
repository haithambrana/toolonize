//! Windows launcher sources (FR-002): Known Folders + `.lnk` shortcuts.
//!
//! Roots come from `SHGetKnownFolderPath` at runtime (never hard-coded).
//! Shortcuts are read with the stored-metadata policy in [`super::lnk`]:
//! `IShellLink::Resolve` is never called. Per-user and all-users copies of the
//! same shortcut are both retained with provenance (no dedup on an
//! unverified precedence assumption).

use super::lnk::{expand_env, parse, ShellLink, MAX_LNK_BYTES};
use super::walk::{read_bounded, walk, Found, WalkLimits};
use super::{root_tag, SourceReport};
use crate::model::{Descriptor, DescriptorKind, OriginKind, ParseStatus, Platform, RawLauncher};
use std::path::{Path, PathBuf};

pub const LNK_LIMITS: WalkLimits = WalkLimits {
    max_depth: 8,
    max_entries: 5_000,
    max_file_bytes: MAX_LNK_BYTES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    File,
    Directory,
    Missing,
}

/// Looks at a local target without executing or resolving anything.
pub type Probe = dyn Fn(&str) -> TargetKind;
pub type EnvLookup = dyn Fn(&str) -> Option<String>;

pub fn fs_probe(path: &str) -> TargetKind {
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => TargetKind::Directory,
        Ok(_) => TargetKind::File,
        Err(_) => TargetKind::Missing,
    }
}

pub fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn is_unc(path: &str) -> bool {
    path.starts_with("\\\\") || path.starts_with("//")
}

fn file_stem(relative: &str) -> String {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    name.strip_suffix(".lnk")
        .or_else(|| name.strip_suffix(".LNK"))
        .unwrap_or(name)
        .to_string()
}

fn origin_tag(origin: OriginKind) -> &'static str {
    match origin {
        OriginKind::ProgramsUser => "programs_user",
        OriginKind::ProgramsCommon => "programs_common",
        OriginKind::DesktopUser => "desktop_user",
        OriginKind::DesktopPublic => "desktop_public",
        _ => "custom",
    }
}

pub fn launcher_id(origin: OriginKind, root: &Path, relative: &str) -> String {
    let rel = relative.to_lowercase();
    match origin {
        OriginKind::CustomRoot => format!("win:custom:{}:{rel}", root_tag(root)),
        other => format!("win:{}:{rel}", origin_tag(other)),
    }
}

fn join_windows(dir: &str, rel: &str) -> String {
    let dir = dir.trim_end_matches(['\\', '/']);
    format!("{dir}\\{rel}")
}

/// Turn stored link fields into a raw launcher record.
pub fn link_to_raw(
    link: &ShellLink,
    origin: OriginKind,
    root: &Path,
    relative: &str,
    link_path: &Path,
    probe: &Probe,
    env: &EnvLookup,
) -> RawLauncher {
    let mut issues = Vec::new();
    let mut status = ParseStatus::Ok;

    let expanded_env_target = link.env_target.as_deref().and_then(|t| {
        let (value, unknown) = expand_env(t, env);
        unknown.is_empty().then_some(value)
    });
    let link_dir = link_path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let program = if let Some(net) = &link.network_path {
        issues.push("network (UNC) target: not inspected, review required".into());
        Some(net.clone())
    } else if let Some(t) = expanded_env_target.or_else(|| link.local_base_path.clone()) {
        Some(t)
    } else if let Some(rel) = &link.relative_path {
        Some(join_windows(&link_dir, rel))
    } else {
        if link.has_id_list {
            issues.push("target is a shell namespace item without a file system path".into());
        } else {
            issues.push("shortcut has no target".into());
        }
        None
    };

    if let Some(target) = program.as_deref() {
        if is_unc(target) {
            if link.network_path.is_none() {
                issues.push("network (UNC) target: not inspected, review required".into());
            }
        } else {
            let lower = target.to_ascii_lowercase();
            if lower.ends_with(".lnk") {
                issues.push("shortcut points to another shortcut (chain)".into());
            } else {
                match probe(target) {
                    TargetKind::File => {}
                    TargetKind::Directory => issues.push("shortcut points to a folder".into()),
                    TargetKind::Missing => {
                        status = ParseStatus::Unavailable;
                        issues.push("target not found (moved or deleted)".into());
                    }
                }
            }
        }
    }
    let working_dir = link
        .working_dir
        .as_deref()
        .filter(|w| !w.is_empty())
        .map(|w| expand_env(w, env).0);
    if working_dir.as_deref().is_some_and(is_unc) {
        issues.push("network (UNC) working directory".into());
    }
    if status == ParseStatus::Ok && !issues.is_empty() {
        status = ParseStatus::NeedsReview;
    }
    if status == ParseStatus::Unavailable && issues.len() > 1 {
        status = ParseStatus::NeedsReview;
    }

    RawLauncher {
        launcher_id: launcher_id(origin, root, relative),
        display_name: file_stem(relative),
        platform: Platform::Windows,
        origin,
        origin_root: root.to_string_lossy().into_owned(),
        relative_path: relative.to_string(),
        descriptor: Descriptor {
            kind: DescriptorKind::ShellLink,
            program,
            args: vec![],
            raw_args: link.arguments.clone().filter(|a| !a.is_empty()),
            exec_template: None,
            working_dir,
            terminal: false,
            try_exec: None,
            icon: link.icon_location.clone().filter(|i| !i.is_empty()),
            path_resolved: false,
            source_path: link_path.to_string_lossy().into_owned(),
        },
        parse_status: status,
        issues,
        no_display: false,
        not_shown_here: false,
        comment: link.name.clone().filter(|n| !n.is_empty()),
    }
}

pub fn scan_lnk_root(
    root: &Path,
    origin: OriginKind,
    limits: WalkLimits,
    probe: &Probe,
    env: &EnvLookup,
) -> (Vec<RawLauncher>, SourceReport) {
    let walked = if root.is_dir() {
        walk(root, "lnk", limits)
    } else {
        Default::default()
    };
    let mut out = Vec::new();
    for found in walked.found {
        let (path, relative, escaping) = match found {
            Found::File { path, relative } => (path, relative, false),
            Found::Escaping { path, relative } => (path, relative, true),
        };
        let blocked = |issue: String| RawLauncher {
            launcher_id: launcher_id(origin, root, &relative),
            display_name: file_stem(&relative),
            platform: Platform::Windows,
            origin,
            origin_root: root.to_string_lossy().into_owned(),
            relative_path: relative.clone(),
            descriptor: Descriptor {
                kind: DescriptorKind::ShellLink,
                program: None,
                args: vec![],
                raw_args: None,
                exec_template: None,
                working_dir: None,
                terminal: false,
                try_exec: None,
                icon: None,
                path_resolved: false,
                source_path: path.to_string_lossy().into_owned(),
            },
            parse_status: ParseStatus::NeedsReview,
            issues: vec![issue],
            no_display: false,
            not_shown_here: false,
            comment: None,
        };
        if escaping {
            out.push(blocked(
                "link points outside its root (not followed)".into(),
            ));
            continue;
        }
        match read_bounded(&path, limits.max_file_bytes)
            .map_err(|e| e.to_string())
            .and_then(|bytes| parse(&bytes).map_err(|e| e.to_string()))
        {
            Ok(link) => out.push(link_to_raw(
                &link, origin, root, &relative, &path, probe, env,
            )),
            Err(error) => out.push(blocked(error)),
        }
    }
    let report = SourceReport {
        origin,
        root: root.to_string_lossy().into_owned(),
        entries: out.len(),
        truncated: walked.truncated,
        errors: walked.errors,
    };
    (out, report)
}

/// Known Folder roots resolved at runtime (T-DISC-WIN-001).
#[cfg(windows)]
pub fn known_folder_roots() -> Vec<(OriginKind, PathBuf)> {
    use windows_sys::Win32::UI::Shell::{
        FOLDERID_CommonPrograms, FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_PublicDesktop,
    };
    [
        (OriginKind::ProgramsUser, &FOLDERID_Programs),
        (OriginKind::ProgramsCommon, &FOLDERID_CommonPrograms),
        (OriginKind::DesktopUser, &FOLDERID_Desktop),
        (OriginKind::DesktopPublic, &FOLDERID_PublicDesktop),
    ]
    .into_iter()
    .filter_map(|(origin, id)| known_folder(id).map(|p| (origin, p)))
    .collect()
}

#[cfg(windows)]
pub fn known_folder(id: &windows_sys::core::GUID) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
    let mut raw: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: `raw` receives a CoTaskMem-allocated, NUL-terminated wide
    // string on success, which is copied and then freed exactly once.
    unsafe {
        let hr = SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut raw);
        if hr != 0 || raw.is_null() {
            if !raw.is_null() {
                CoTaskMemFree(raw as _);
            }
            return None;
        }
        let mut len = 0;
        while *raw.add(len) != 0 {
            len += 1;
        }
        let path = std::ffi::OsString::from_wide(std::slice::from_raw_parts(raw, len));
        CoTaskMemFree(raw as _);
        Some(PathBuf::from(path))
    }
}

#[cfg(not(windows))]
pub fn known_folder_roots() -> Vec<(OriginKind, PathBuf)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::super::lnk::build::{build, LnkSpec};
    use super::*;

    fn probe(path: &str) -> TargetKind {
        match path {
            p if p.ends_with("example.exe") || p.ends_with("pwsh.exe") => TargetKind::File,
            p if p.ends_with("\\Projects") => TargetKind::Directory,
            _ => TargetKind::Missing,
        }
    }

    fn env(name: &str) -> Option<String> {
        (name == "ProgramFiles").then(|| "C:\\Program Files".into())
    }

    fn write(dir: &Path, rel: &str, spec: LnkSpec<'_>) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, build(&spec)).unwrap();
    }

    fn fixture_root() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "Example/Example Tool.lnk",
            LnkSpec {
                local_base_path: Some("C:\\Program Files\\Example\\example.exe"),
                arguments: Some("--profile work"),
                working_dir: Some("%ProgramFiles%\\Example"),
                ..Default::default()
            },
        );
        write(
            root,
            "Env Target.lnk",
            LnkSpec {
                env_target: Some("%ProgramFiles%\\Example\\example.exe"),
                local_base_path: Some("C:\\Old\\example.old"),
                ..Default::default()
            },
        );
        write(
            root,
            "Moved.lnk",
            LnkSpec {
                local_base_path: Some("C:\\Gone\\missing.exe"),
                ..Default::default()
            },
        );
        write(
            root,
            "Chain.lnk",
            LnkSpec {
                local_base_path: Some("C:\\Users\\Example\\Desktop\\Other.lnk"),
                ..Default::default()
            },
        );
        write(
            root,
            "Folder.lnk",
            LnkSpec {
                local_base_path: Some("C:\\Users\\Example\\Projects"),
                ..Default::default()
            },
        );
        write(
            root,
            "Network.lnk",
            LnkSpec {
                network: Some(("\\\\fileserver.example.com\\tools", "example.exe")),
                ..Default::default()
            },
        );
        write(
            root,
            "Packaged.lnk",
            LnkSpec {
                id_list: true,
                ..Default::default()
            },
        );
        std::fs::write(root.join("Garbage.lnk"), b"definitely not a shell link").unwrap();
        dir
    }

    fn scan(root: &Path, origin: OriginKind) -> Vec<RawLauncher> {
        scan_lnk_root(root, origin, LNK_LIMITS, &probe, &env).0
    }

    fn find<'a>(all: &'a [RawLauncher], name: &str) -> &'a RawLauncher {
        all.iter().find(|l| l.display_name == name).unwrap()
    }

    #[test]
    fn normalized_record_from_lnk_t_disc_win_002() {
        let dir = fixture_root();
        let all = scan(dir.path(), OriginKind::ProgramsUser);
        let tool = find(&all, "Example Tool");
        assert_eq!(tool.parse_status, ParseStatus::Ok, "{:?}", tool.issues);
        assert_eq!(tool.platform, Platform::Windows);
        assert_eq!(
            tool.launcher_id,
            "win:programs_user:example/example tool.lnk"
        );
        assert_eq!(
            tool.descriptor.program.as_deref(),
            Some("C:\\Program Files\\Example\\example.exe")
        );
        assert_eq!(tool.descriptor.raw_args.as_deref(), Some("--profile work"));
        assert!(
            tool.descriptor.args.is_empty(),
            "no argv splitting of link strings"
        );
        assert_eq!(
            tool.descriptor.working_dir.as_deref(),
            Some("C:\\Program Files\\Example")
        );
        let env_target = find(&all, "Env Target");
        assert_eq!(
            env_target.descriptor.program.as_deref(),
            Some("C:\\Program Files\\Example\\example.exe")
        );
    }

    #[test]
    fn moved_dead_chain_folder_network_need_review_t_disc_win_003_004() {
        let dir = fixture_root();
        let all = scan(dir.path(), OriginKind::ProgramsUser);
        assert_eq!(find(&all, "Moved").parse_status, ParseStatus::Unavailable);
        for name in ["Chain", "Folder", "Network", "Packaged", "Garbage"] {
            let l = find(&all, name);
            assert_eq!(
                l.parse_status,
                ParseStatus::NeedsReview,
                "{name}: {:?}",
                l.issues
            );
            assert!(!l.issues.is_empty());
        }
    }

    #[test]
    fn duplicate_user_common_pair_retained_with_provenance_t_disc_win_006() {
        let user = fixture_root();
        let common = fixture_root();
        let mut all = scan(user.path(), OriginKind::ProgramsUser);
        all.extend(scan(common.path(), OriginKind::ProgramsCommon));
        let copies: Vec<_> = all
            .iter()
            .filter(|l| l.display_name == "Example Tool")
            .collect();
        assert_eq!(copies.len(), 2, "both records kept");
        assert_ne!(copies[0].launcher_id, copies[1].launcher_id);
        assert_ne!(copies[0].origin, copies[1].origin);
    }

    #[test]
    fn oversized_link_bounded_t_disc_win_007() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Huge.lnk"), vec![0u8; MAX_LNK_BYTES + 10]).unwrap();
        let all = scan(dir.path(), OriginKind::DesktopUser);
        assert_eq!(all[0].parse_status, ParseStatus::NeedsReview);
    }

    #[test]
    fn custom_root_ids_are_scoped_t_disc_win_009() {
        let dir = fixture_root();
        let all = scan(dir.path(), OriginKind::CustomRoot);
        assert!(all.iter().all(|l| l.launcher_id.starts_with("win:custom:")));
        assert!(all.iter().all(|l| l.origin == OriginKind::CustomRoot));
    }

    #[cfg(not(windows))]
    #[test]
    fn known_folders_are_runtime_only_t_disc_win_001() {
        assert!(
            known_folder_roots().is_empty(),
            "no hard-coded Windows paths"
        );
    }

    #[cfg(windows)]
    #[test]
    fn known_folders_resolve_at_runtime_t_disc_win_001() {
        let roots = known_folder_roots();
        assert!(roots.iter().any(|(o, _)| *o == OriginKind::ProgramsUser));
        assert!(roots.iter().all(|(_, p)| p.is_absolute()));
    }
}
