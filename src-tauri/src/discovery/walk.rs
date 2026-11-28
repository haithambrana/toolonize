//! Bounded, read-only directory walk shared by all launcher sources.
//!
//! Never follows a symlink whose target leaves the declared root
//! (T-DISC-05); such entries are reported so they can be flagged.

use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct WalkLimits {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_file_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// A candidate file inside the root (possibly via an in-root symlink).
    File { path: PathBuf, relative: String },
    /// A symlink whose target escapes the root (not followed).
    Escaping { path: PathBuf, relative: String },
}

#[derive(Debug, Default)]
pub struct WalkResult {
    pub found: Vec<Found>,
    pub truncated: bool,
    pub errors: Vec<String>,
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Walk `root` collecting files whose extension matches `ext`
/// (case-insensitive), depth-first in sorted order for determinism.
pub fn walk(root: &Path, ext: &str, limits: WalkLimits) -> WalkResult {
    let mut result = WalkResult::default();
    let Ok(canonical_root) = std::fs::canonicalize(root) else {
        return result;
    };
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut seen = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                result
                    .errors
                    .push(format!("{}: {error}", relative(root, &dir)));
                continue;
            }
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        let mut subdirs = Vec::new();
        for path in paths {
            seen += 1;
            if seen > limits.max_entries {
                result.truncated = true;
                return result;
            }
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            let rel = relative(root, &path);
            let matches_ext = path
                .extension()
                .map(|e| e.to_string_lossy().eq_ignore_ascii_case(ext))
                .unwrap_or(false);
            if meta.file_type().is_symlink() {
                let inside = std::fs::canonicalize(&path)
                    .map(|target| target.starts_with(&canonical_root))
                    .unwrap_or(false);
                if !inside {
                    if matches_ext {
                        result.found.push(Found::Escaping {
                            path,
                            relative: rel,
                        });
                    }
                    continue;
                }
                let Ok(target_meta) = std::fs::metadata(&path) else {
                    continue;
                };
                if target_meta.is_dir() {
                    // In-root directory symlinks could loop; skip them.
                    continue;
                }
                if matches_ext && target_meta.is_file() {
                    result.found.push(Found::File {
                        path,
                        relative: rel,
                    });
                }
            } else if meta.is_dir() {
                if depth + 1 < limits.max_depth {
                    subdirs.push((path, depth + 1));
                } else {
                    result.truncated = true;
                }
            } else if meta.is_file() && matches_ext {
                result.found.push(Found::File {
                    path,
                    relative: rel,
                });
            }
        }
        subdirs.reverse();
        stack.extend(subdirs);
    }
    result
}

/// Read at most `max` bytes; `Err` when the file is larger.
pub fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err(format!("file exceeds {max} bytes"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: WalkLimits = WalkLimits {
        max_depth: 3,
        max_entries: 100,
        max_file_bytes: 1024,
    };

    #[test]
    fn finds_nested_files_in_order() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sub/deeper")).unwrap();
        std::fs::write(dir.path().join("b.desktop"), "").unwrap();
        std::fs::write(dir.path().join("a.desktop"), "").unwrap();
        std::fs::write(dir.path().join("sub/c.DESKTOP"), "").unwrap();
        std::fs::write(dir.path().join("sub/ignored.txt"), "").unwrap();
        std::fs::write(dir.path().join("sub/deeper/d.desktop"), "").unwrap();
        let result = walk(dir.path(), "desktop", LIMITS);
        let rels: Vec<_> = result
            .found
            .iter()
            .map(|f| match f {
                Found::File { relative, .. } => relative.clone(),
                Found::Escaping { relative, .. } => format!("!{relative}"),
            })
            .collect();
        assert_eq!(
            rels,
            vec![
                "a.desktop",
                "b.desktop",
                "sub/c.DESKTOP",
                "sub/deeper/d.desktop"
            ]
        );
        assert!(!result.truncated);
    }

    #[test]
    fn depth_and_count_limits_truncate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("1/2/3/4")).unwrap();
        std::fs::write(dir.path().join("1/2/3/4/deep.desktop"), "").unwrap();
        let result = walk(dir.path(), "desktop", LIMITS);
        assert!(result.found.is_empty());
        assert!(result.truncated);

        let dir = tempfile::tempdir().unwrap();
        for i in 0..20 {
            std::fs::write(dir.path().join(format!("{i}.desktop")), "").unwrap();
        }
        let result = walk(
            dir.path(),
            "desktop",
            WalkLimits {
                max_entries: 5,
                ..LIMITS
            },
        );
        assert_eq!(result.found.len(), 5);
        assert!(result.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn escaping_symlink_is_flagged_not_followed_t_sec_003() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.desktop"), "[Desktop Entry]").unwrap();
        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.desktop"),
            root.path().join("link.desktop"),
        )
        .unwrap();
        std::fs::write(root.path().join("real.desktop"), "").unwrap();
        std::os::unix::fs::symlink(
            root.path().join("real.desktop"),
            root.path().join("inner.desktop"),
        )
        .unwrap();
        let result = walk(root.path(), "desktop", LIMITS);
        assert!(result.found.contains(&Found::Escaping {
            path: root.path().join("link.desktop"),
            relative: "link.desktop".into()
        }));
        assert!(result.found.contains(&Found::File {
            path: root.path().join("inner.desktop"),
            relative: "inner.desktop".into()
        }));
    }

    #[test]
    fn bounded_read() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(&p, vec![b'x'; 2000]).unwrap();
        assert!(read_bounded(&p, 1024).is_err());
        assert_eq!(read_bounded(&p, 4096).unwrap().len(), 2000);
    }
}
