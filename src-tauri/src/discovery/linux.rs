//! Linux launcher sources (FR-001, FR-007, FR-008).
//!
//! Registered XDG application entries (incl. Flatpak/Snap exports), the
//! user's Desktop directory resolved via xdg-user-dirs, and opt-in custom
//! roots. Read-only metadata discovery: nothing is executed (FR-004).

use super::exec::{expand, tokenize, ExpandContext};
use super::keyfile::{locale_candidates, KeyFile, MAX_FILE_BYTES};
use super::walk::{read_bounded, walk, Found, WalkLimits};
use super::{root_tag, SourceReport};
use crate::model::{Descriptor, DescriptorKind, OriginKind, ParseStatus, Platform, RawLauncher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const APP_LIMITS: WalkLimits = WalkLimits {
    max_depth: 4,
    max_entries: 10_000,
    max_file_bytes: MAX_FILE_BYTES,
};
pub const CUSTOM_LIMITS: WalkLimits = WalkLimits {
    max_depth: 6,
    max_entries: 2_000,
    max_file_bytes: MAX_FILE_BYTES,
};

/// The process environment that discovery depends on, injectable for tests.
#[derive(Debug, Clone, Default)]
pub struct LinuxEnv {
    pub home: Option<PathBuf>,
    pub xdg_data_home: Option<String>,
    pub xdg_data_dirs: Option<String>,
    pub xdg_config_home: Option<String>,
    pub current_desktop: Option<String>,
    pub locale: Option<String>,
    pub path: Option<String>,
}

impl LinuxEnv {
    pub fn from_process() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        Self {
            home: var("HOME").map(PathBuf::from),
            xdg_data_home: var("XDG_DATA_HOME"),
            xdg_data_dirs: var("XDG_DATA_DIRS"),
            xdg_config_home: var("XDG_CONFIG_HOME"),
            current_desktop: var("XDG_CURRENT_DESKTOP"),
            locale: var("LC_ALL")
                .or_else(|| var("LC_MESSAGES"))
                .or_else(|| var("LANG")),
            path: var("PATH"),
        }
    }

    fn data_home(&self) -> Option<PathBuf> {
        match &self.xdg_data_home {
            Some(p) if Path::new(p).is_absolute() => Some(PathBuf::from(p)),
            _ => self.home.as_ref().map(|h| h.join(".local/share")),
        }
    }

    fn config_home(&self) -> Option<PathBuf> {
        match &self.xdg_config_home {
            Some(p) if Path::new(p).is_absolute() => Some(PathBuf::from(p)),
            _ => self.home.as_ref().map(|h| h.join(".config")),
        }
    }

    fn data_dirs(&self) -> Vec<PathBuf> {
        let raw = self
            .xdg_data_dirs
            .clone()
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        raw.split(':')
            .filter(|p| Path::new(p).is_absolute())
            .map(PathBuf::from)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRoot {
    pub path: PathBuf,
    pub origin: OriginKind,
}

fn origin_for(path: &Path, user: bool) -> OriginKind {
    let text = path.to_string_lossy();
    if text.contains("/flatpak/exports/") {
        OriginKind::Flatpak
    } else if text.starts_with("/var/lib/snapd/desktop") {
        OriginKind::Snap
    } else if user {
        OriginKind::XdgUser
    } else {
        OriginKind::XdgSystem
    }
}

/// `applications/` roots in precedence order (user first), de-duplicated,
/// with Flatpak/Snap export dirs appended when not already listed.
pub fn application_roots(env: &LinuxEnv) -> Vec<AppRoot> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |path: PathBuf, user: bool| {
        let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if seen.insert(key) {
            roots.push(AppRoot {
                origin: origin_for(&path, user),
                path,
            });
        }
    };
    if let Some(home) = env.data_home() {
        push(home.join("applications"), true);
    }
    for dir in env.data_dirs() {
        push(dir.join("applications"), false);
    }
    if let Some(home) = env.data_home() {
        push(home.join("flatpak/exports/share/applications"), true);
    }
    push(
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        false,
    );
    push(PathBuf::from("/var/lib/snapd/desktop/applications"), false);
    roots
}

/// Resolve `XDG_DESKTOP_DIR` by parsing `user-dirs.dirs` directly (never by
/// running the eval-based helper). No hard-coded `~/Desktop` fallback.
pub fn desktop_dir(env: &LinuxEnv) -> Option<PathBuf> {
    let config = env.config_home()?;
    let bytes = read_bounded(&config.join("user-dirs.dirs"), 16 * 1024).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    parse_user_dirs(&text, env.home.as_deref()?, "XDG_DESKTOP_DIR")
}

pub fn parse_user_dirs(text: &str, home: &Path, key: &str) -> Option<PathBuf> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        let v = v.trim();
        let v = v.strip_prefix('"')?.strip_suffix('"')?;
        if v.contains(['`', '"']) || v.contains("$(") {
            return None;
        }
        // Spec: either "$HOME/relative" or an absolute path. "$HOME" alone
        // means the directory is disabled.
        let path = if let Some(rest) = v.strip_prefix("$HOME") {
            let rest = rest.strip_prefix('/')?;
            if rest.is_empty() {
                return None;
            }
            home.join(rest)
        } else if v.starts_with('/') {
            PathBuf::from(v)
        } else {
            return None;
        };
        if path
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return None;
        }
        return Some(path);
    }
    None
}

fn is_executable(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                meta.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                true
            }
        }
        _ => false,
    }
}

/// Availability check only (stat + mode). Never executes (FR-004).
pub fn find_program(name: &str, path_var: Option<&str>) -> Option<PathBuf> {
    if name.contains('/') {
        let p = PathBuf::from(name);
        return is_executable(&p).then_some(p);
    }
    path_var?
        .split(':')
        .filter(|d| Path::new(d).is_absolute())
        .map(|d| Path::new(d).join(name))
        .find(|p| is_executable(p))
}

struct EntryContext<'a> {
    env: &'a LinuxEnv,
    locales: Vec<String>,
}

enum ParsedEntry {
    Launcher(Box<RawLauncher>),
    /// Hidden=true: a tombstone that shadows lower-precedence entries.
    Tombstone,
    /// Type=Directory and similar: not a launcher.
    Skip,
}

fn blocked_record(
    id: String,
    origin: OriginKind,
    root: &Path,
    relative: &str,
    path: &Path,
    issue: String,
) -> RawLauncher {
    RawLauncher {
        launcher_id: id,
        display_name: relative.rsplit('/').next().unwrap_or(relative).to_string(),
        platform: Platform::Linux,
        origin,
        origin_root: root.to_string_lossy().into_owned(),
        relative_path: relative.to_string(),
        descriptor: Descriptor {
            kind: DescriptorKind::ExecTemplate,
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
    }
}

fn parse_entry(
    ctx: &EntryContext<'_>,
    id: String,
    origin: OriginKind,
    root: &Path,
    relative: &str,
    path: &Path,
) -> ParsedEntry {
    let blocked = |issue: String| {
        ParsedEntry::Launcher(Box::new(blocked_record(
            id.clone(),
            origin,
            root,
            relative,
            path,
            issue,
        )))
    };
    let bytes = match read_bounded(path, MAX_FILE_BYTES) {
        Ok(bytes) => bytes,
        Err(error) => return blocked(error),
    };
    let file = match KeyFile::parse_bytes(&bytes) {
        Ok(file) => file,
        Err(error) => return blocked(error.to_string()),
    };
    let Some(group) = file.group("Desktop Entry") else {
        return blocked("missing [Desktop Entry] group".into());
    };
    if group.boolean("Hidden") == Some(true) {
        return ParsedEntry::Tombstone;
    }
    let mut issues = Vec::new();
    match group.string("Type").as_deref() {
        Some("Application") => {}
        Some("Directory") => return ParsedEntry::Skip,
        Some("Link") => return blocked("Type=Link entries are not launchable".into()),
        Some(other) => return blocked(format!("unsupported Type={other}")),
        None => issues.push("missing Type key".to_string()),
    }
    let name = group
        .localized("Name", &ctx.locales)
        .filter(|n| !n.trim().is_empty());
    let display_name = match &name {
        Some(n) => n.chars().take(200).collect(),
        None => {
            issues.push("missing Name".into());
            relative.to_string()
        }
    };
    let icon = group.string("Icon");
    let terminal = group.boolean("Terminal").unwrap_or(false);
    let working_dir = group.string("Path").filter(|p| !p.is_empty());
    let try_exec = group.string("TryExec").filter(|p| !p.is_empty());
    let path_text = path.to_string_lossy().into_owned();

    let (kind, program, args, exec_template, path_resolved) = match group.string("Exec") {
        Some(exec) => {
            let expansion = expand(
                &tokenize(&exec),
                &ExpandContext {
                    icon: icon.as_deref(),
                    name: name.as_deref(),
                    desktop_file: Some(&path_text),
                },
            );
            issues.extend(expansion.issues);
            let mut argv = expansion.argv.into_iter();
            let program = argv.next();
            let path_resolved = program.as_deref().is_some_and(|p| !p.contains('/'));
            (
                DescriptorKind::ExecTemplate,
                program,
                argv.collect(),
                Some(exec),
                path_resolved,
            )
        }
        None if group.boolean("DBusActivatable") == Some(true) => {
            issues.push("DBusActivatable without Exec: ToolOnize launches via Exec only".into());
            (DescriptorKind::DbusActivatable, None, vec![], None, false)
        }
        None => {
            issues.push("missing Exec".into());
            (DescriptorKind::ExecTemplate, None, vec![], None, false)
        }
    };
    if let Some(dir) = &working_dir {
        if !Path::new(dir).is_absolute() {
            issues.push("Path is not absolute".into());
        }
    }

    let mut parse_status = if issues.is_empty() {
        ParseStatus::Ok
    } else {
        ParseStatus::NeedsReview
    };
    if parse_status == ParseStatus::Ok {
        if let Some(try_exec) = &try_exec {
            if find_program(try_exec, ctx.env.path.as_deref()).is_none() {
                parse_status = ParseStatus::Unavailable;
                issues.push(format!("TryExec target not found: {try_exec}"));
            }
        }
    }

    let desktops: Vec<String> = ctx
        .env
        .current_desktop
        .as_deref()
        .unwrap_or("")
        .split(':')
        .filter(|d| !d.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let only = group.list("OnlyShowIn");
    let not = group.list("NotShowIn");
    let mut not_shown_here = false;
    if !desktops.is_empty() {
        let hit = |list: &[String]| {
            list.iter()
                .any(|d| desktops.contains(&d.to_ascii_lowercase()))
        };
        if (!only.is_empty() && !hit(&only)) || hit(&not) {
            not_shown_here = true;
        }
    }

    ParsedEntry::Launcher(Box::new(RawLauncher {
        launcher_id: id,
        display_name,
        platform: Platform::Linux,
        origin,
        origin_root: root.to_string_lossy().into_owned(),
        relative_path: relative.to_string(),
        descriptor: Descriptor {
            kind,
            program,
            args,
            raw_args: None,
            exec_template,
            working_dir,
            terminal,
            try_exec,
            icon,
            path_resolved,
            source_path: path_text,
        },
        parse_status,
        issues,
        no_display: group.boolean("NoDisplay") == Some(true),
        not_shown_here,
        comment: group
            .localized("Comment", &ctx.locales)
            .map(|c| c.chars().take(300).collect()),
    }))
}

fn context(env: &LinuxEnv) -> EntryContext<'_> {
    EntryContext {
        env,
        locales: locale_candidates(env.locale.as_deref()),
    }
}

/// Registered application entries with spec precedence (FR-001).
pub fn scan_applications(env: &LinuxEnv) -> (Vec<RawLauncher>, Vec<SourceReport>) {
    let ctx = context(env);
    let mut out = Vec::new();
    let mut reports = Vec::new();
    let mut claimed: HashSet<String> = HashSet::new();
    for root in application_roots(env) {
        if !root.path.is_dir() {
            continue;
        }
        let walked = walk(&root.path, "desktop", APP_LIMITS);
        let mut count = 0;
        for found in walked.found {
            let (path, relative, escaping) = match found {
                Found::File { path, relative } => (path, relative, false),
                Found::Escaping { path, relative } => (path, relative, true),
            };
            let desktop_id = relative.replace('/', "-");
            if !claimed.insert(desktop_id.clone()) {
                continue; // shadowed by a higher-precedence entry
            }
            let id = format!("linux:app:{desktop_id}");
            if escaping {
                out.push(blocked_record(
                    id,
                    root.origin,
                    &root.path,
                    &relative,
                    &path,
                    "symlink points outside its root (not followed)".into(),
                ));
                count += 1;
                continue;
            }
            match parse_entry(&ctx, id, root.origin, &root.path, &relative, &path) {
                ParsedEntry::Launcher(l) => {
                    out.push(*l);
                    count += 1;
                }
                ParsedEntry::Tombstone | ParsedEntry::Skip => {}
            }
        }
        reports.push(SourceReport {
            origin: root.origin,
            root: root.path.to_string_lossy().into_owned(),
            entries: count,
            truncated: walked.truncated,
            errors: walked.errors,
        });
    }
    (out, reports)
}

/// Bounded scan of a non-registered root (Desktop dir or custom root).
/// Identity is source-root + relative path: there is no Desktop File ID.
pub fn scan_loose_root(
    env: &LinuxEnv,
    root: &Path,
    origin: OriginKind,
    limits: WalkLimits,
) -> (Vec<RawLauncher>, SourceReport) {
    let ctx = context(env);
    let mut out = Vec::new();
    let walked = if root.is_dir() {
        walk(root, "desktop", limits)
    } else {
        Default::default()
    };
    let prefix = match origin {
        OriginKind::DesktopDir => "linux:desktop".to_string(),
        _ => format!("linux:custom:{}", root_tag(root)),
    };
    for found in walked.found {
        let (path, relative, escaping) = match found {
            Found::File { path, relative } => (path, relative, false),
            Found::Escaping { path, relative } => (path, relative, true),
        };
        let id = format!("{prefix}:{relative}");
        if escaping {
            out.push(blocked_record(
                id,
                origin,
                root,
                &relative,
                &path,
                "symlink points outside its root (not followed)".into(),
            ));
            continue;
        }
        if let ParsedEntry::Launcher(l) = parse_entry(&ctx, id, origin, root, &relative, &path) {
            out.push(*l);
        }
    }
    let mut errors = walked.errors;
    if walked.truncated {
        errors.push("scan stopped at the depth/count limit".into());
    }
    let report = SourceReport {
        origin,
        root: root.to_string_lossy().into_owned(),
        entries: out.len(),
        truncated: walked.truncated,
        errors,
    };
    (out, report)
}

#[cfg(test)]
#[path = "linux_tests.rs"]
mod tests;
