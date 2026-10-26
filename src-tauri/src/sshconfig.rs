//! Host aliases from the user's OpenSSH client configuration (ADR-011).
//!
//! Read only on an explicit user action. Only `~/.ssh/config` and files it
//! `Include`s from inside `~/.ssh` are opened; keys, `known_hosts` and
//! everything else in that folder are never read. Only `Host` aliases leave
//! this module — no host names, users, ports, proxy commands or other
//! options — and each alias must pass the same grammar as a typed SSH
//! launcher (`shells::valid_host`), so it can never become an ssh option.

use crate::shells::valid_host;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_CONFIG_BYTES: u64 = 256 * 1024;
pub const MAX_HOSTS: usize = 500;
const MAX_FILES: usize = 32;
const MAX_INCLUDE_DEPTH: u8 = 4;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HostList {
    pub hosts: Vec<String>,
    /// The config file that was read (shown to the user).
    pub source: String,
    /// Things that were skipped, in plain words.
    pub notes: Vec<String>,
}

/// `~/.ssh` (`%USERPROFILE%\.ssh` on Windows).
pub fn ssh_dir() -> Option<PathBuf> {
    let home = if cfg!(windows) {
        std::env::var_os("USERPROFILE")
    } else {
        std::env::var_os("HOME")
    };
    home.filter(|h| !h.is_empty())
        .map(|h| PathBuf::from(h).join(".ssh"))
}

pub fn read_hosts(ssh_dir: &Path) -> HostList {
    let config = ssh_dir.join("config");
    let mut out = HostList {
        source: config.to_string_lossy().into_owned(),
        ..Default::default()
    };
    if !config.is_file() {
        out.notes.push("no ssh config file found".into());
        return out;
    }
    let root = match fs::canonicalize(ssh_dir) {
        Ok(root) => root,
        Err(_) => {
            out.notes.push("the ssh folder could not be opened".into());
            return out;
        }
    };
    let mut reader = Reader {
        root,
        files: 0,
        out,
    };
    reader.file(&config, 0);
    reader.out.notes.dedup();
    reader.out
}

struct Reader {
    root: PathBuf,
    files: usize,
    out: HostList,
}

impl Reader {
    fn file(&mut self, path: &Path, depth: u8) {
        if self.files >= MAX_FILES {
            self.note("too many included files; the rest were skipped");
            return;
        }
        // Stay inside ~/.ssh even through symlinks.
        let real = match fs::canonicalize(path) {
            Ok(p) if p.starts_with(&self.root) => p,
            Ok(_) => {
                self.note("an Include outside the ssh folder was skipped");
                return;
            }
            Err(_) => return,
        };
        let meta = match fs::metadata(&real) {
            Ok(m) if m.is_file() => m,
            _ => return,
        };
        if meta.len() > MAX_CONFIG_BYTES {
            self.note("a config file larger than 256 KiB was skipped");
            return;
        }
        self.files += 1;
        let Ok(bytes) = fs::read(&real) else { return };
        let text = String::from_utf8_lossy(&bytes);
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (keyword, rest) = split_keyword(line);
            match keyword.to_ascii_lowercase().as_str() {
                "host" => {
                    for token in rest.split_whitespace() {
                        self.host(token.trim_matches('"'));
                    }
                }
                "include" if depth < MAX_INCLUDE_DEPTH => {
                    for pattern in rest.split_whitespace() {
                        for included in self.expand(pattern.trim_matches('"')) {
                            self.file(&included, depth + 1);
                        }
                    }
                }
                "include" => self.note("Include nesting is too deep; skipped"),
                _ => {}
            }
        }
    }

    fn host(&mut self, token: &str) {
        // Patterns (*, ?), negations (!) and anything outside the launcher
        // grammar are not connectable aliases.
        if !valid_host(token) || self.out.hosts.iter().any(|h| h == token) {
            return;
        }
        if self.out.hosts.len() >= MAX_HOSTS {
            self.note("more than 500 hosts; the rest were skipped");
            return;
        }
        self.out.hosts.push(token.to_string());
    }

    /// Resolve an Include pattern the way ssh does for the user config:
    /// relative paths are under ~/.ssh, `~/` is the home folder. Wildcards
    /// are supported in the file name only; matches are sorted.
    fn expand(&mut self, pattern: &str) -> Vec<PathBuf> {
        let home = self.root.parent().map(Path::to_path_buf);
        let path = if let Some(rest) = pattern.strip_prefix("~/") {
            match home {
                Some(h) => h.join(rest),
                None => return Vec::new(),
            }
        } else {
            let p = PathBuf::from(pattern);
            if p.is_absolute() {
                p
            } else {
                self.root.join(p)
            }
        };
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return Vec::new();
        };
        if !name.contains(['*', '?']) {
            return vec![path];
        }
        let Some(dir) = path.parent() else {
            return Vec::new();
        };
        if dir.to_string_lossy().contains(['*', '?']) {
            self.note("Include wildcards in folder names are not supported; skipped");
            return Vec::new();
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| wildcard(&name, &e.file_name().to_string_lossy()))
            .map(|e| e.path())
            .collect();
        found.sort();
        found
    }

    fn note(&mut self, text: &str) {
        if !self.out.notes.iter().any(|n| n == text) {
            self.out.notes.push(text.into());
        }
    }
}

/// `Keyword value`, `Keyword=value` or `Keyword = value`.
fn split_keyword(line: &str) -> (&str, &str) {
    let end = line
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(line.len());
    let rest = line[end..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    (&line[..end], rest)
}

/// Glob match with `*` and `?` (no character classes).
fn wildcard(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    let (mut pi, mut ni) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join(".ssh")).unwrap();
        home
    }

    #[test]
    fn extracts_only_host_aliases() {
        let home = dir();
        let ssh = home.path().join(".ssh");
        fs::write(
            ssh.join("config"),
            "# comment\n\
             Host build.example.com staging\n  HostName 192.0.2.10\n  User alice\n  ProxyCommand nc %h %p\n\
             Host *.internal !secret prod-?\n\
             host=db1\n\
             HOST web \"quoted\"\n\
             Host -oProxyCommand=evil\n\
             Match host db1\n  User bob\n\
             Host staging\n",
        )
        .unwrap();
        let list = read_hosts(&ssh);
        assert_eq!(
            list.hosts,
            vec!["build.example.com", "staging", "db1", "web", "quoted"]
        );
        let all = format!("{list:?}");
        for secret in ["192.0.2.10", "alice", "nc %h", "bob", "evil"] {
            assert!(!all.contains(secret), "{secret} leaked");
        }
    }

    #[test]
    fn follows_includes_inside_the_ssh_folder_only() {
        let home = dir();
        let ssh = home.path().join(".ssh");
        fs::create_dir(ssh.join("config.d")).unwrap();
        fs::write(ssh.join("config.d/b.conf"), "Host bravo\n").unwrap();
        fs::write(ssh.join("config.d/a.conf"), "Host alpha\n").unwrap();
        fs::write(ssh.join("config.d/notes.txt"), "Host nope\n").unwrap();
        fs::write(home.path().join("outside"), "Host outside\n").unwrap();
        fs::write(
            ssh.join("config"),
            "Include config.d/*.conf\nInclude ~/outside\nInclude /etc/hostname\nHost main\n",
        )
        .unwrap();
        let list = read_hosts(&ssh);
        assert_eq!(list.hosts, vec!["alpha", "bravo", "main"]);
        assert!(list
            .notes
            .iter()
            .any(|n| n.contains("outside the ssh folder")));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_escape_and_include_loops_stop() {
        let home = dir();
        let ssh = home.path().join(".ssh");
        fs::write(home.path().join("private"), "Host leaked\n").unwrap();
        std::os::unix::fs::symlink(home.path().join("private"), ssh.join("link")).unwrap();
        fs::write(ssh.join("loop"), "Include loop\nHost looped\n").unwrap();
        fs::write(ssh.join("config"), "Include link\nInclude loop\n").unwrap();
        let list = read_hosts(&ssh);
        assert_eq!(list.hosts, vec!["looped"]);
    }

    #[test]
    fn missing_or_oversized_config() {
        let home = dir();
        let ssh = home.path().join(".ssh");
        assert!(read_hosts(&ssh).hosts.is_empty());
        let big = "Host a\n".repeat((MAX_CONFIG_BYTES as usize / 7) + 10);
        fs::write(ssh.join("config"), big).unwrap();
        let list = read_hosts(&ssh);
        assert!(list.hosts.is_empty());
        assert!(list.notes.iter().any(|n| n.contains("256 KiB")));
    }

    #[test]
    fn wildcard_matching() {
        assert!(wildcard("*.conf", "a.conf"));
        assert!(wildcard("a?c", "abc"));
        assert!(!wildcard("*.conf", "a.txt"));
        assert!(wildcard("*", "anything"));
    }
}
