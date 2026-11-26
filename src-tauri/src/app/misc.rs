//! Settings, audit log, ssh hosts, output export, URLs and shutdown.

use super::*;

impl Core {
    pub fn settings(&self) -> Settings {
        self.data.locked().settings.clone()
    }

    pub fn update_settings(&self, settings: Settings) -> Result<(), String> {
        settings.validate()?;
        let watch_changed = {
            let mut data = self.data.locked();
            self.config_store
                .save("settings", SETTINGS_FORMAT, SETTINGS_SCHEMA, &settings)?;
            let changed = data.settings.live_updates != settings.live_updates;
            self.sessions
                .set_retention(settings.replay_buffer_mib as usize * 1024 * 1024);
            data.settings = settings;
            changed
        };
        if watch_changed {
            self.restart_watch();
        }
        self.bus.emit(AppEvent::Settings);
        Ok(())
    }

    pub fn audit_recent(&self, limit: usize) -> Vec<AuditEntry> {
        self.audit.recent(limit.min(500))
    }

    pub fn ssh_hosts(&self) -> crate::sshconfig::HostList {
        let list = match crate::sshconfig::ssh_dir() {
            Some(dir) => crate::sshconfig::read_hosts(&dir),
            None => crate::sshconfig::HostList {
                notes: vec!["no home folder".into()],
                ..Default::default()
            },
        };
        self.audit.record(
            "ssh.hosts.read",
            Outcome::Recorded,
            Record {
                detail: Some(&format!("{} host aliases", list.hosts.len())),
                ..Default::default()
            },
        );
        list
    }

    pub fn export_scrollback(&self, name: &str, text: &str) -> Result<String, String> {
        let dir = crate::export::downloads_dir().ok_or("no Downloads or home folder")?;
        let path = crate::export::export_text(&dir, name, text, now_ms())?;
        self.audit.record(
            "scrollback.export",
            Outcome::Recorded,
            Record {
                detail: Some(&format!("{} bytes", text.len())),
                ..Default::default()
            },
        );
        Ok(path.to_string_lossy().into_owned())
    }

    pub fn open_url(&self, url: &str) -> Result<(), String> {
        let result = external::open_url(url);
        self.audit.record(
            "url.open",
            if result.is_ok() {
                Outcome::Allowed
            } else {
                Outcome::Denied
            },
            Record {
                detail: result.as_ref().err().map(String::as_str),
                ..Default::default()
            },
        );
        result
    }

    pub fn shutdown(&self) {
        *self.watch.locked() = None;
        self.sessions.shutdown();
    }

    #[cfg(test)]
    pub fn registry_entry(&self, id: &str) -> Option<RegistryEntry> {
        let data = self.data.locked();
        data.registry.effective(id, &data.state.registry)
    }

    /// A plain-text report for bug reports: versions, settings, counts and
    /// recent log lines. No workspace or terminal names, commands, output,
    /// notes or host names; home paths appear as `~`.
    pub fn diagnostics_report(&self) -> String {
        use std::fmt::Write as _;
        let data = self.data.locked();
        let mut out = String::new();
        let _ = writeln!(out, "ToolOnize diagnostics");
        let _ = writeln!(out, "version: {}", env!("CARGO_PKG_VERSION"));
        let _ = writeln!(
            out,
            "platform: {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let _ = writeln!(out, "pty backend: {}", self.startup_info().backend);
        let _ = writeln!(
            out,
            "state: generation {}, recovered {}, problems {}",
            self.load_report.generation,
            self.load_report.recovered,
            self.load_report.problems.len()
        );
        let _ = writeln!(out, "\n[settings]");
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&data.settings).unwrap_or_default()
        );
        let _ = writeln!(out, "\n[workspaces]");
        let members: usize = data.state.workspaces.iter().map(|w| w.members.len()).sum();
        let _ = writeln!(
            out,
            "workspaces: {}, members: {}, staged imports: {}",
            data.state.workspaces.len(),
            members,
            data.state
                .workspaces
                .iter()
                .filter(|w| w.staged_import)
                .count()
        );
        let mut by_type: std::collections::BTreeMap<String, usize> = Default::default();
        for m in data.state.workspaces.iter().flat_map(|w| &w.members) {
            *by_type.entry(format!("{:?}", m.member_type)).or_default() += 1;
        }
        let _ = writeln!(out, "members by type: {by_type:?}");
        let _ = writeln!(out, "\n[launchers]");
        let view = data.registry.entries(&data.state.registry);
        let mut by_origin: std::collections::BTreeMap<String, usize> = Default::default();
        for e in &view {
            *by_origin
                .entry(format!("{:?}", e.launcher.origin))
                .or_default() += 1;
        }
        let _ = writeln!(
            out,
            "entries: {}, waiting for review: {}",
            view.len(),
            view.iter().filter(|e| e.review_pending).count()
        );
        let _ = writeln!(out, "by origin: {by_origin:?}");
        drop(data);
        let _ = writeln!(out, "watch: {:?}", self.watch_status());
        let _ = writeln!(out, "\n[sessions]");
        for s in self.sessions.list() {
            let _ = writeln!(
                out,
                "{} gen {} remote {} view {:?}: {:?}",
                s.session_id, s.generation, s.remote, s.view_state, s.process_state
            );
        }
        let _ = writeln!(out, "\n[recent log]");
        for line in crate::logging::tail(300) {
            let _ = writeln!(out, "{line}");
        }
        let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).ok();
        out.lines()
            .map(|l| match home.as_deref() {
                Some(h) if h.len() > 1 => l.replace(h, "~"),
                _ => l.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    pub fn export_diagnostics(&self) -> Result<String, String> {
        let report = self.diagnostics_report();
        let dir = crate::export::downloads_dir().ok_or("no Downloads or home folder")?;
        let path = crate::export::export_text(&dir, "diagnostics", &report, now_ms())?;
        Ok(path.to_string_lossy().into_owned())
    }

    /// Check and clean a desktop notification request. None = do not show
    /// (turned off in Settings, or more than 6 in the last 30 seconds).
    pub fn prepare_notification(&self, title: &str, body: &str) -> Option<(String, String)> {
        if !self.data.locked().settings.desktop_notifications {
            return None;
        }
        let clean = |s: &str, max: usize| -> String {
            let s: String = s
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            s.trim().chars().take(max).collect()
        };
        let (title, body) = (clean(title, 120), clean(body, 400));
        if title.is_empty() {
            return None;
        }
        let mut recent = self.notified.locked();
        let window = Duration::from_secs(30);
        while recent.front().is_some_and(|t| t.elapsed() > window) {
            recent.pop_front();
        }
        if recent.len() >= 6 {
            return None;
        }
        recent.push_back(Instant::now());
        Some((title, body))
    }
}
