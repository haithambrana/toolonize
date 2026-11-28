//! Open ToolOnize when the user logs in (ADR-017). With session memory
//! (ADR-015) this brings back the last session after a reboot.
//!
//! Uses each platform's per-user mechanism and nothing else:
//! - Linux: an XDG autostart entry `~/.config/autostart/toolonize.desktop`;
//! - Windows: the value `ToolOnize` under
//!   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`;
//! - macOS: `~/Library/LaunchAgents/com.crmonize.toolonize.plist`.
//!
//! The entry only ever names ToolOnize's own executable (plus
//! `--autostart`); it is written from the running executable's path, never
//! from input.

use std::path::{Path, PathBuf};

pub const AUTOSTART_FLAG: &str = "--autostart";

/// The executable to start at login: the AppImage file when running from
/// one (the mounted path changes every run), else this executable.
pub fn launch_target() -> Option<PathBuf> {
    if let Some(appimage) = std::env::var_os("APPIMAGE").map(PathBuf::from) {
        if appimage.is_absolute() && appimage.is_file() {
            return Some(appimage);
        }
    }
    std::env::current_exe().ok()
}

/// Quote an argument for a desktop entry `Exec` key (Desktop Entry spec:
/// double quotes; `"`, `` ` ``, `$` and `\` escaped with a backslash; `%`
/// doubled).
pub fn desktop_exec_quote(arg: &str) -> String {
    let mut out = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=ToolOnize\n\
         Comment=Open ToolOnize at login and continue the last session\n\
         Exec={} {AUTOSTART_FLAG}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        desktop_exec_quote(&exe.to_string_lossy())
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn launch_agent(exe: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\"><dict>\n\
         <key>Label</key><string>com.crmonize.toolonize</string>\n\
         <key>ProgramArguments</key><array><string>{}</string><string>{AUTOSTART_FLAG}</string></array>\n\
         <key>RunAtLoad</key><true/>\n\
         </dict></plist>\n",
        xml_escape(&exe.to_string_lossy())
    )
}

/// Windows Run value: the quoted executable plus the flag.
pub fn run_value(exe: &Path) -> String {
    format!("\"{}\" {AUTOSTART_FLAG}", exe.display())
}

/// Write `contents` to `path` atomically (temp file, then rename).
fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let dir = path.parent().ok_or("no folder for the login entry")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let tmp = dir.join(".toolonize-autostart.tmp");
    std::fs::write(&tmp, contents)
        .and_then(|_| std::fs::rename(&tmp, path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("could not write {}: {e}", path.display())
        })
}

/// A file-based login entry (Linux desktop file, macOS launch agent).
pub struct FileEntry {
    pub path: PathBuf,
    pub contents: String,
}

impl FileEntry {
    /// Enabled = the entry exists and starts this executable.
    pub fn is_enabled(&self) -> bool {
        std::fs::read_to_string(&self.path).is_ok_and(|c| c == self.contents)
    }

    pub fn set(&self, enabled: bool) -> Result<(), String> {
        if enabled {
            write_atomic(&self.path, &self.contents)
        } else {
            match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("could not remove {}: {e}", self.path.display())),
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn file_entry(exe: &Path) -> Option<FileEntry> {
    let env = crate::discovery::linux::LinuxEnv::from_process();
    Some(FileEntry {
        path: env.config_home()?.join("autostart/toolonize.desktop"),
        contents: desktop_entry(exe),
    })
}

#[cfg(target_os = "macos")]
fn file_entry(exe: &Path) -> Option<FileEntry> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(FileEntry {
        path: home.join("Library/LaunchAgents/com.crmonize.toolonize.plist"),
        contents: launch_agent(exe),
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn file_entry(_exe: &Path) -> Option<FileEntry> {
    None
}

/// Is ToolOnize set to open at login (for this executable)?
pub fn is_enabled() -> bool {
    let Some(exe) = launch_target() else {
        return false;
    };
    #[cfg(windows)]
    {
        windows_run::get().is_some_and(|v| v == run_value(&exe))
    }
    #[cfg(not(windows))]
    {
        file_entry(&exe).is_some_and(|e| e.is_enabled())
    }
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let exe = launch_target().ok_or("could not find ToolOnize's own executable")?;
    #[cfg(windows)]
    {
        if enabled {
            windows_run::set(&run_value(&exe))
        } else {
            windows_run::remove()
        }
    }
    #[cfg(not(windows))]
    {
        file_entry(&exe)
            .ok_or("opening at login is not supported here")?
            .set(enabled)
    }
}

#[cfg(windows)]
mod windows_run {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
    };

    const KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const NAME: &str = "ToolOnize";
    /// ERROR_FILE_NOT_FOUND: the value is not there.
    const NOT_FOUND: u32 = 2;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn get() -> Option<String> {
        let (key, name) = (wide(KEY), wide(NAME));
        let mut buf = vec![0u16; 2048];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY: valid NUL-terminated strings and a buffer of `size` bytes.
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if rc != ERROR_SUCCESS {
            return None;
        }
        let len = (size as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..len.min(buf.len())]))
    }

    pub fn set(value: &str) -> Result<(), String> {
        let (key, name, data) = (wide(KEY), wide(NAME), wide(value));
        // SAFETY: NUL-terminated key, name and REG_SZ data of the given size.
        let rc = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        if rc == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("could not set the Run value (error {rc})"))
        }
    }

    pub fn remove() -> Result<(), String> {
        let (key, name) = (wide(KEY), wide(NAME));
        // SAFETY: NUL-terminated key and value names.
        let rc = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
        if rc == ERROR_SUCCESS || rc == NOT_FOUND {
            Ok(())
        } else {
            Err(format!("could not remove the Run value (error {rc})"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_quoting_follows_the_desktop_entry_spec() {
        assert_eq!(
            desktop_exec_quote("/opt/ToolOnize/toolonize"),
            "\"/opt/ToolOnize/toolonize\""
        );
        assert_eq!(
            desktop_exec_quote("/home/a b/$x`y\"z\\w%u"),
            "\"/home/a b/\\$x\\`y\\\"z\\\\w%%u\""
        );
        let entry = desktop_entry(Path::new("/usr/bin/toolonize"));
        assert!(entry.contains("Exec=\"/usr/bin/toolonize\" --autostart\n"));
        assert!(entry.starts_with("[Desktop Entry]\nType=Application\n"));
        assert!(!entry.contains("Terminal=true"));
    }

    #[test]
    fn launch_agent_and_run_value_name_only_this_program() {
        let plist = launch_agent(Path::new("/Applications/Tool<&>.app/x"));
        assert!(plist.contains("<string>/Applications/Tool&lt;&amp;&gt;.app/x</string>"));
        assert!(plist.contains("<string>--autostart</string>"));
        assert_eq!(
            run_value(Path::new("C:\\Program Files\\ToolOnize\\toolonize.exe")),
            "\"C:\\Program Files\\ToolOnize\\toolonize.exe\" --autostart"
        );
    }

    #[test]
    fn file_entry_enable_disable_and_detect_stale_entries() {
        let dir = tempfile::tempdir().unwrap();
        let entry = FileEntry {
            path: dir.path().join("autostart/toolonize.desktop"),
            contents: desktop_entry(Path::new("/usr/bin/toolonize")),
        };
        assert!(!entry.is_enabled());
        entry.set(true).unwrap();
        assert!(entry.is_enabled());
        assert!(!dir
            .path()
            .join("autostart/.toolonize-autostart.tmp")
            .exists());
        // An entry for another (old) location is not "enabled" for this one.
        let moved = FileEntry {
            path: entry.path.clone(),
            contents: desktop_entry(Path::new("/opt/new/toolonize")),
        };
        assert!(!moved.is_enabled());
        moved.set(true).unwrap();
        assert!(moved.is_enabled());
        moved.set(false).unwrap();
        assert!(!entry.path.exists());
        moved.set(false).unwrap(); // already off: fine
    }
}
