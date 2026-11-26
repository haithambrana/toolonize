//! Discovery and the launcher registry.

use super::*;

impl Core {
    pub(super) fn discovery_config(&self) -> DiscoveryConfig {
        let data = self.data.locked();
        DiscoveryConfig {
            custom_roots: data
                .state
                .registry
                .custom_roots
                .iter()
                .map(PathBuf::from)
                .collect(),
        }
    }

    /// Full rescan of every source plus synthetic launchers. Read-only.
    pub fn rescan(&self) -> Result<Diff, String> {
        let _guard = self.scan_lock.locked();
        let started = Instant::now();
        let config = self.discovery_config();
        let mut output = discovery::scan_all(&config);
        output.launchers.extend(shells::system_shells());
        let synthetic = self.data.locked().state.registry.synthetic.clone();
        output
            .launchers
            .extend(synthetic.iter().map(shells::synthetic_launcher));
        let diff = {
            let mut data = self.data.locked();
            let mut next = data.state.registry.clone();
            let mut registry = data.registry.clone();
            let diff = registry.apply_scan(output.launchers, output.reports, &mut next);
            if next != data.state.registry {
                let mut state = data.state.clone();
                state.registry = next;
                // Registry tracking is best-effort: a failed save must not
                // hide what is on disk now.
                if self
                    .store
                    .save("state", STATE_FORMAT, STATE_SCHEMA, &state)
                    .is_ok()
                {
                    data.state = state;
                }
            }
            data.registry = registry;
            diff
        };
        crate::logging::info(
            "discovery",
            &format!(
                "scan done in {} ms: {} new, {} changed, {} removed",
                started.elapsed().as_millis(),
                diff.added.len(),
                diff.changed.len(),
                diff.removed.len()
            ),
        );
        self.bus.emit(AppEvent::Registry { diff: diff.clone() });
        self.bus.emit(AppEvent::Workspaces);
        Ok(diff)
    }

    /// (Re)start watching launcher roots according to settings.
    pub fn restart_watch(&self) {
        let enabled = self.data.locked().settings.live_updates;
        let mut slot = self.watch.locked();
        *slot = None;
        if !enabled {
            drop(slot);
            self.bus.emit(AppEvent::Watch {
                status: watch::WatchStatus::Off,
            });
            return;
        }
        let roots = discovery::watch_roots(&self.discovery_config());
        let weak = self.me.get().cloned();
        let handle = watch::start(
            roots,
            Arc::new(move || {
                if let Some(core) = weak.as_ref().and_then(Weak::upgrade) {
                    let _ = core.rescan();
                }
            }),
        );
        let status = handle.status();
        *slot = Some(handle);
        drop(slot);
        self.bus.emit(AppEvent::Watch { status });
    }

    pub fn watch_status(&self) -> watch::WatchStatus {
        self.watch
            .lock()
            .unwrap()
            .as_ref()
            .map(|w| w.status())
            .unwrap_or(watch::WatchStatus::Off)
    }

    pub fn registry_view(&self) -> RegistryView {
        let data = self.data.locked();
        RegistryView {
            entries: data.registry.entries(&data.state.registry),
            reports: data.registry.reports.clone(),
            last_scan_ms: data.registry.last_scan_ms,
            custom_roots: data.state.registry.custom_roots.clone(),
            synthetic: data.state.registry.synthetic.clone(),
            removed: data.state.registry.removed.clone(),
            watch: self.watch_status(),
        }
    }

    pub(super) fn registry_changed(&self) {
        self.bus.emit(AppEvent::Registry {
            diff: Diff::default(),
        });
        self.bus.emit(AppEvent::Workspaces);
    }

    pub fn set_hidden(&self, id: &str, hidden: bool) -> Result<(), String> {
        check_id(id)?;
        self.mutate(|s, _| {
            s.registry
                .decisions
                .entry(id.to_string())
                .or_default()
                .hidden = hidden;
            Ok(())
        })?;
        self.registry_changed();
        Ok(())
    }

    pub fn set_alias(&self, id: &str, alias: Option<String>) -> Result<(), String> {
        check_id(id)?;
        self.mutate(|s, _| registry::set_alias(&mut s.registry, id, alias))?;
        self.registry_changed();
        Ok(())
    }

    pub fn approve(&self, id: &str, fingerprint: &str) -> Result<(), String> {
        check_id(id)?;
        let result = self.mutate(|s, reg| registry::approve(reg, &mut s.registry, id, fingerprint));
        self.audit.record(
            "registry.approve",
            if result.is_ok() {
                Outcome::Recorded
            } else {
                Outcome::Denied
            },
            Record {
                launcher_id: Some(id),
                fingerprint: Some(fingerprint),
                detail: result.as_ref().err().map(String::as_str),
                ..Default::default()
            },
        );
        result?;
        self.registry_changed();
        Ok(())
    }

    pub fn revoke(&self, id: &str) -> Result<(), String> {
        check_id(id)?;
        self.mutate(|s, _| {
            if let Some(d) = s.registry.decisions.get_mut(id) {
                d.approved_fingerprint = None;
                d.approved_at = None;
            }
            Ok(())
        })?;
        self.audit.record(
            "registry.revoke",
            Outcome::Recorded,
            Record {
                launcher_id: Some(id),
                ..Default::default()
            },
        );
        self.registry_changed();
        Ok(())
    }

    pub fn acknowledge(&self, ids: &[String]) -> Result<(), String> {
        if ids.len() > 10_000 {
            return Err("too many ids".into());
        }
        self.mutate(|s, reg| {
            for id in ids {
                if let (Some(known), Some(l)) = (s.registry.known.get_mut(id), reg.get(id)) {
                    known.acknowledged = Some(l.fingerprint.clone());
                }
            }
            Ok(())
        })?;
        self.registry_changed();
        Ok(())
    }

    pub fn add_custom_root(&self, path: &str) -> Result<(), String> {
        let canonical = discovery::validate_custom_root(path)?;
        let text = canonical.to_string_lossy().into_owned();
        self.mutate(|s, _| {
            if s.registry.custom_roots.contains(&text) {
                return Ok(());
            }
            if s.registry.custom_roots.len() >= MAX_CUSTOM_ROOTS {
                return Err(format!("at most {MAX_CUSTOM_ROOTS} custom roots"));
            }
            s.registry.custom_roots.push(text.clone());
            Ok(())
        })?;
        self.rescan()?;
        self.restart_watch();
        Ok(())
    }

    pub fn remove_custom_root(&self, path: &str) -> Result<(), String> {
        self.mutate(|s, _| {
            s.registry.custom_roots.retain(|r| r != path);
            Ok(())
        })?;
        self.rescan()?;
        self.restart_watch();
        Ok(())
    }

    pub fn add_synthetic(&self, spec: SyntheticSpec) -> Result<String, String> {
        spec.validate()?;
        let id = spec.launcher_id();
        self.mutate(|s, _| {
            if !s.registry.synthetic.contains(&spec) {
                if s.registry.synthetic.len() >= MAX_SYNTHETIC {
                    return Err("too many custom connections".into());
                }
                s.registry.synthetic.push(spec.clone());
            }
            Ok(())
        })?;
        self.rescan()?;
        Ok(id)
    }

    pub fn remove_synthetic(&self, launcher_id: &str) -> Result<(), String> {
        self.mutate(|s, _| {
            s.registry
                .synthetic
                .retain(|x| x.launcher_id() != launcher_id);
            Ok(())
        })?;
        self.rescan()?;
        Ok(())
    }
}
