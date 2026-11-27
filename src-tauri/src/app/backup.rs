//! Full backup and restore, duplicate, and templates (ADR-014).

use super::*;
use crate::backup::{self, Backup, RestoreReport, Template, BACKUP_FORMAT, BACKUP_SCHEMA};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct TemplateSummary {
    pub id: String,
    pub name: String,
    pub created_at: u64,
    pub members: usize,
}

impl Core {
    /// The backup file contents. Settings only when asked for.
    pub fn backup_text(&self, include_settings: bool) -> Result<String, String> {
        let (state, settings) = {
            let data = self.data.locked();
            (data.state.clone(), data.settings.clone())
        };
        let mut notes = BTreeMap::new();
        for w in &state.workspaces {
            let n = self.workspace_notes(&w.id)?;
            if !n.is_empty() {
                notes.insert(w.id.clone(), n);
            }
        }
        // Authorizations belong to this machine and are never restored; they
        // are left out so the file carries nothing that could look trusted.
        // Environment values can be secrets (tokens): only names travel.
        let strip = |w: &Workspace| {
            let mut w = w.clone();
            for m in &mut w.members {
                m.authorized_fingerprint = None;
                m.authorized_at = None;
                for v in &mut m.launch.env {
                    v.value.clear();
                }
                m.launch.confirmed = None;
            }
            w
        };
        let decisions = &state.registry.decisions;
        let backup = Backup {
            format: BACKUP_FORMAT.into(),
            schema_version: BACKUP_SCHEMA,
            created_at: now_ms(),
            app_version: app_version().into(),
            workspaces: state.workspaces.iter().map(strip).collect(),
            templates: state
                .templates
                .iter()
                .map(|t| Template {
                    workspace: strip(&t.workspace),
                    ..t.clone()
                })
                .collect(),
            aliases: decisions
                .iter()
                .filter_map(|(id, d)| d.alias.clone().map(|a| (id.clone(), a)))
                .collect(),
            hidden: decisions
                .iter()
                .filter(|(_, d)| d.hidden)
                .map(|(id, _)| id.clone())
                .collect(),
            custom_roots: state.registry.custom_roots.clone(),
            synthetic: state.registry.synthetic.clone(),
            settings: include_settings
                .then(|| serde_json::to_value(&settings))
                .transpose()
                .map_err(|e| e.to_string())?,
            notes,
        };
        let text = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
        if text.len() > backup::MAX_BACKUP_BYTES {
            return Err("the backup would be larger than 16 MiB".into());
        }
        Ok(text)
    }

    /// Write a backup file to Downloads; returns its path.
    pub fn export_backup(&self, include_settings: bool) -> Result<String, String> {
        let text = self.backup_text(include_settings)?;
        let dir = crate::export::downloads_dir().ok_or("no Downloads or home folder")?;
        let path = crate::export::export_file(&dir, "backup", "json", &text, now_ms())?;
        self.audit.record(
            "backup.export",
            Outcome::Recorded,
            Record {
                detail: Some(if include_settings {
                    "with settings"
                } else {
                    "without settings"
                }),
                ..Default::default()
            },
        );
        Ok(path.to_string_lossy().into_owned())
    }

    /// Restore a backup next to what exists. Restored workspaces are staged
    /// for review, every member must be authorized again, and nothing starts.
    pub fn restore_backup(
        &self,
        text: &str,
        restore_settings: bool,
    ) -> Result<RestoreReport, String> {
        let existing = self.data.locked().state.workspaces.len();
        let restored = backup::parse_and_validate(text, restore_settings, existing)?;
        // Notes first, under workspace ids that are not used yet: if the
        // workspaces cannot be saved afterwards, the files are removed again.
        {
            let _guard = self.notes_write.locked();
            for (id, notes) in &restored.notes {
                if let Err(error) = self.store_notes(id, notes.clone()) {
                    for (id, _) in &restored.notes {
                        self.drop_notes(id);
                    }
                    return Err(format!("could not save restored notes: {error}"));
                }
            }
        }
        let mut report = restored.report.clone();
        let registry_changed = self.mutate(|s, _| {
            if s.workspaces.len() + restored.workspaces.len() > MAX_WORKSPACES {
                return Err(format!("at most {MAX_WORKSPACES} workspaces"));
            }
            s.workspaces.extend(restored.workspaces.iter().cloned());
            let room = backup::MAX_TEMPLATES.saturating_sub(s.templates.len());
            if restored.templates.len() > room {
                report.warnings.push(format!(
                    "{} templates not restored (at most {})",
                    restored.templates.len() - room,
                    backup::MAX_TEMPLATES
                ));
                report.templates = room;
            }
            s.templates
                .extend(restored.templates.iter().take(room).cloned());
            for (id, alias) in &restored.aliases {
                let d = s.registry.decisions.entry(id.clone()).or_default();
                if d.alias.is_none() {
                    d.alias = Some(alias.clone());
                }
            }
            for id in &restored.hidden {
                s.registry.decisions.entry(id.clone()).or_default().hidden = true;
            }
            let mut changed = false;
            for root in &restored.custom_roots {
                if !s.registry.custom_roots.contains(root)
                    && s.registry.custom_roots.len() < MAX_CUSTOM_ROOTS
                {
                    s.registry.custom_roots.push(root.clone());
                    changed = true;
                }
            }
            for spec in &restored.synthetic {
                if !s.registry.synthetic.contains(spec)
                    && s.registry.synthetic.len() < MAX_SYNTHETIC
                {
                    s.registry.synthetic.push(spec.clone());
                    changed = true;
                }
            }
            Ok(changed)
        });
        let registry_changed = match registry_changed {
            Ok(changed) => changed,
            Err(error) => {
                let _guard = self.notes_write.locked();
                for (id, _) in &restored.notes {
                    self.drop_notes(id);
                }
                return Err(error);
            }
        };
        if let Some(settings) = restored.settings {
            if let Err(error) = self.update_settings(settings) {
                report.settings_restored = false;
                report
                    .warnings
                    .push(format!("settings not restored: {error}"));
            }
        }
        self.audit.record(
            "backup.restore",
            Outcome::Recorded,
            Record {
                detail: Some("restored into review staging; nothing authorized"),
                ..Default::default()
            },
        );
        if registry_changed {
            if let Err(error) = self.rescan() {
                report
                    .warnings
                    .push(format!("launcher scan failed: {error}"));
            }
        }
        self.changed();
        Ok(report)
    }

    /// Copy a workspace (members, options, layout and notes) with new ids.
    /// Member authorizations are kept: same machine, same launchers, and the
    /// user asked for the copy.
    pub fn duplicate_workspace(&self, id: &str) -> Result<String, String> {
        check_id(id)?;
        let source = self
            .data
            .locked()
            .state
            .workspaces
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or("workspace not found")?;
        let name: String = format!("{} (copy)", source.name).chars().take(80).collect();
        let (copy, ids) = backup::copy_with_new_ids(&source, name);
        let new_id = copy.id.clone();
        let notes = self.workspace_notes(id)?;
        if !notes.is_empty() {
            let mapped = crate::notes::WorkspaceNotes {
                project: notes.project.clone(),
                members: notes
                    .members
                    .iter()
                    .filter_map(|(old, n)| ids.get(old).map(|new| (new.clone(), n.clone())))
                    .collect(),
            };
            let _guard = self.notes_write.locked();
            self.store_notes(&new_id, mapped)?;
        }
        let result = self.mutate(|s, _| {
            if s.workspaces.len() >= MAX_WORKSPACES {
                return Err(format!("at most {MAX_WORKSPACES} workspaces"));
            }
            let at = s
                .workspaces
                .iter()
                .position(|w| w.id == id)
                .map_or(s.workspaces.len(), |i| i + 1);
            s.workspaces.insert(at, copy.clone());
            s.active_workspace = Some(new_id.clone());
            Ok(())
        });
        if let Err(error) = result {
            let _guard = self.notes_write.locked();
            self.drop_notes(&new_id);
            return Err(error);
        }
        self.changed();
        Ok(new_id)
    }

    pub fn templates(&self) -> Vec<TemplateSummary> {
        self.data
            .locked()
            .state
            .templates
            .iter()
            .map(|t| TemplateSummary {
                id: t.id.clone(),
                name: t.name.clone(),
                created_at: t.created_at,
                members: t.workspace.members.len(),
            })
            .collect()
    }

    /// Save a workspace's shape (members, options, layout) as a template.
    /// Project-specific parts (folders, notes) are left out.
    pub fn save_template(&self, workspace_id: &str, name: &str) -> Result<String, String> {
        check_id(workspace_id)?;
        let name = validate_name(name)?;
        let id = new_id("tpl");
        self.mutate(|s, _| {
            if s.templates.len() >= backup::MAX_TEMPLATES {
                return Err(format!("at most {} templates", backup::MAX_TEMPLATES));
            }
            if s.templates.iter().any(|t| t.name == name) {
                return Err(format!("a template named “{name}” already exists"));
            }
            let w = s
                .workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .ok_or("workspace not found")?;
            let (mut shape, _) = backup::copy_with_new_ids(w, name.clone());
            shape.project_dir = None;
            shape.staged_import = false;
            for m in &mut shape.members {
                m.start_dir = None;
            }
            s.templates.push(Template {
                id: id.clone(),
                name: name.clone(),
                created_at: now_ms(),
                workspace: shape,
            });
            Ok(())
        })?;
        self.changed();
        Ok(id)
    }

    /// A new workspace from a template, with fresh ids.
    pub fn create_from_template(&self, template_id: &str, name: &str) -> Result<String, String> {
        check_id(template_id)?;
        let name = validate_name(name)?;
        let new_id = self.mutate(|s, _| {
            if s.workspaces.len() >= MAX_WORKSPACES {
                return Err(format!("at most {MAX_WORKSPACES} workspaces"));
            }
            let t = s
                .templates
                .iter()
                .find(|t| t.id == template_id)
                .ok_or("template not found")?;
            let (w, _) = backup::copy_with_new_ids(&t.workspace, name.clone());
            let id = w.id.clone();
            s.workspaces.push(w);
            s.active_workspace = Some(id.clone());
            Ok(id)
        })?;
        self.changed();
        Ok(new_id)
    }

    pub fn delete_template(&self, template_id: &str) -> Result<(), String> {
        check_id(template_id)?;
        self.mutate(|s, _| {
            let before = s.templates.len();
            s.templates.retain(|t| t.id != template_id);
            if s.templates.len() == before {
                return Err("template not found".into());
            }
            Ok(())
        })?;
        self.changed();
        Ok(())
    }
}
