//! Workspaces, members and layouts.

use super::*;

impl Core {
    pub(super) fn view(&self, w: &Workspace, data: &Data) -> WorkspaceView {
        let members = w
            .members
            .iter()
            .map(|m| {
                let entry = data
                    .registry
                    .effective(&m.launcher_id, &data.state.registry);
                MemberView {
                    member: m.clone(),
                    status: policy::member_status(m, &data.registry, &data.state.registry),
                    effective_type: entry.as_ref().map(|e| e.effective_type),
                    origin_label: entry.as_ref().map(|e| e.launcher.origin_label.clone()),
                    interpreter: entry
                        .as_ref()
                        .map(|e| e.launcher.classification.interpreter)
                        .unwrap_or(false),
                    review_pending: entry.as_ref().map(|e| e.review_pending).unwrap_or(false),
                }
            })
            .collect();
        WorkspaceView {
            id: w.id.clone(),
            name: w.name.clone(),
            icon: w.icon.clone(),
            created_at: w.created_at,
            updated_at: w.updated_at,
            startup_policy: w.startup_policy,
            layout: w.layout.clone(),
            staged_import: w.staged_import,
            color: w.color.clone(),
            project_dir: w.project_dir.clone(),
            members,
        }
    }

    pub fn workspaces(&self) -> (Vec<WorkspaceView>, Option<String>) {
        let data = self.data.locked();
        (
            data.state
                .workspaces
                .iter()
                .map(|w| self.view(w, &data))
                .collect(),
            data.state.active_workspace.clone(),
        )
    }

    pub(super) fn changed(&self) {
        self.bus.emit(AppEvent::Workspaces);
    }

    pub fn create_workspace(&self, name: &str) -> Result<String, String> {
        let name = validate_name(name)?;
        let id = new_id("ws");
        self.mutate(|s, _| {
            if s.workspaces.len() >= MAX_WORKSPACES {
                return Err(format!("at most {MAX_WORKSPACES} workspaces"));
            }
            s.workspaces.push(Workspace::new(id.clone(), name));
            s.active_workspace = Some(id.clone());
            Ok(())
        })?;
        self.changed();
        Ok(id)
    }

    pub(super) fn with_workspace<R>(
        &self,
        id: &str,
        f: impl FnOnce(&mut Workspace, &Registry, &RegistryState) -> Result<R, String>,
    ) -> Result<R, String> {
        check_id(id)?;
        let result = self.mutate(|s, reg| {
            let registry_state = s.registry.clone();
            let w = s
                .workspaces
                .iter_mut()
                .find(|w| w.id == id)
                .ok_or("workspace not found")?;
            let r = f(w, reg, &registry_state)?;
            w.touch();
            Ok(r)
        })?;
        self.changed();
        Ok(result)
    }

    pub fn rename_workspace(&self, id: &str, name: &str) -> Result<(), String> {
        let name = validate_name(name)?;
        self.with_workspace(id, |w, _, _| {
            w.name = name;
            Ok(())
        })
    }

    pub fn set_workspace_color(&self, id: &str, color: Option<&str>) -> Result<(), String> {
        let color = validate_color(color)?;
        self.with_workspace(id, |w, _, _| {
            w.color = color;
            Ok(())
        })
    }

    pub fn set_project_dir(&self, id: &str, dir: Option<&str>) -> Result<(), String> {
        let dir = validate_dir(dir)?;
        self.with_workspace(id, |w, _, _| {
            w.project_dir = dir;
            Ok(())
        })
    }

    pub fn delete_workspace(&self, id: &str) -> Result<(), String> {
        check_id(id)?;
        // Notes first: if they cannot be kept, nothing is deleted.
        let (name, titles) = {
            let data = self.data.locked();
            let w = data
                .state
                .workspaces
                .iter()
                .find(|w| w.id == id)
                .ok_or("workspace not found")?;
            let titles: HashMap<String, String> = w
                .members
                .iter()
                .map(|m| (m.id.clone(), self.member_title(m, &data)))
                .collect();
            (w.name.clone(), titles)
        };
        self.trash_workspace_notes(id, &name, &titles)?;
        self.mutate(|s, _| {
            let before = s.workspaces.len();
            s.workspaces.retain(|w| w.id != id);
            if s.workspaces.len() == before {
                return Err("workspace not found".into());
            }
            if s.active_workspace.as_deref() == Some(id) {
                s.active_workspace = s.workspaces.first().map(|w| w.id.clone());
            }
            Ok(())
        })?;
        self.sessions.close_workspace(id);
        self.changed();
        Ok(())
    }

    pub fn set_active(&self, id: Option<&str>) -> Result<(), String> {
        if let Some(id) = id {
            check_id(id)?;
        }
        self.mutate(|s, _| {
            if let Some(id) = id {
                if !s.workspaces.iter().any(|w| w.id == id) {
                    return Err("workspace not found".into());
                }
            }
            s.active_workspace = id.map(str::to_string);
            Ok(())
        })?;
        self.changed();
        Ok(())
    }

    pub fn set_policy(&self, id: &str, policy: StartupPolicy) -> Result<(), String> {
        self.with_workspace(id, |w, _, _| {
            w.startup_policy = policy;
            Ok(())
        })
    }

    pub fn accept_import(&self, id: &str) -> Result<(), String> {
        self.with_workspace(id, |w, _, _| {
            w.staged_import = false;
            Ok(())
        })
    }

    /// Pin a launcher into a workspace: the explicit authorization of its
    /// current fingerprint for this member (FR-013, FR-015).
    pub fn add_member(
        &self,
        workspace_id: &str,
        launcher_id: &str,
        fingerprint: &str,
        title: Option<String>,
    ) -> Result<String, String> {
        check_id(launcher_id)?;
        let title = validate_title(title)?;
        let member_id = new_id("m");
        let result = self.with_workspace(workspace_id, |w, reg, rs| {
            if w.members.len() >= MAX_MEMBERS {
                return Err(format!("at most {MAX_MEMBERS} members per workspace"));
            }
            let entry = reg.effective(launcher_id, rs).ok_or("launcher not found")?;
            if entry.launcher.fingerprint != fingerprint {
                return Err("the launcher changed since it was displayed; review it again".into());
            }
            let member_type =
                MemberType::from_launcher(entry.effective_type).ok_or(if entry.review_pending {
                    "review this launcher before adding it"
                } else {
                    "ToolOnize cannot run this launcher"
                })?;
            let program = entry
                .launcher
                .descriptor
                .program
                .clone()
                .unwrap_or_default();
            let detach_expected = matches!(
                crate::classify::program_name(&program).as_str(),
                "tmux" | "screen" | "zellij" | "byobu"
            ) || entry.launcher.descriptor.args.iter().any(|a| a == "tmux");
            w.members.push(Member {
                id: member_id.clone(),
                launcher_id: launcher_id.to_string(),
                member_type,
                title,
                authorized_fingerprint: Some(fingerprint.to_string()),
                authorized_at: Some(now_ms()),
                auto_restart: false,
                detach_expected,
                added_at: now_ms(),
                portable: None,
                color: None,
                start_dir: None,
                zoom: 0,
            });
            Ok(())
        });
        self.audit.record(
            "member.authorize",
            if result.is_ok() {
                Outcome::Recorded
            } else {
                Outcome::Denied
            },
            Record {
                workspace_id: Some(workspace_id),
                member_id: Some(&member_id),
                launcher_id: Some(launcher_id),
                fingerprint: Some(fingerprint),
                detail: result.as_ref().err().map(String::as_str),
            },
        );
        result.map(|_| member_id)
    }

    /// Re-authorize a member after review (changed or imported members).
    pub fn authorize_member(
        &self,
        workspace_id: &str,
        member_id: &str,
        fingerprint: &str,
    ) -> Result<(), String> {
        let result = self.with_workspace(workspace_id, |w, reg, rs| {
            let member = w.member_mut(member_id).ok_or("member not found")?;
            let entry = reg
                .effective(&member.launcher_id, rs)
                .ok_or("the launcher is not present on this machine")?;
            if entry.launcher.fingerprint != fingerprint {
                return Err("the launcher changed since it was displayed; review it again".into());
            }
            let member_type =
                MemberType::from_launcher(entry.effective_type).ok_or(if entry.review_pending {
                    "review this launcher in the Launchers view first"
                } else {
                    "ToolOnize cannot run this launcher"
                })?;
            member.member_type = member_type;
            member.authorized_fingerprint = Some(fingerprint.to_string());
            member.authorized_at = Some(now_ms());
            Ok(member.launcher_id.clone())
        });
        self.audit.record(
            "member.reauthorize",
            if result.is_ok() {
                Outcome::Recorded
            } else {
                Outcome::Denied
            },
            Record {
                workspace_id: Some(workspace_id),
                member_id: Some(member_id),
                launcher_id: result.as_ref().ok().map(String::as_str),
                fingerprint: Some(fingerprint),
                detail: result.as_ref().err().map(String::as_str),
            },
        );
        result.map(|_| ())
    }

    pub fn remove_member(&self, workspace_id: &str, member_id: &str) -> Result<(), String> {
        let title = self.with_workspace(workspace_id, |w, reg, rs| {
            let m = w.member(member_id).ok_or("member not found")?;
            let title = m.title.clone().unwrap_or_else(|| {
                reg.effective(&m.launcher_id, rs)
                    .map(|e| e.decision.alias.unwrap_or(e.launcher.display_name))
                    .unwrap_or_else(|| m.launcher_id.clone())
            });
            w.members.retain(|m| m.id != member_id);
            if w.layout.focused_member.as_deref() == Some(member_id) {
                w.layout.focused_member = None;
            }
            Ok(title)
        })?;
        self.keep_removed_member_note(workspace_id, member_id, &title);
        if let Some(info) = self.sessions.find_by_member(workspace_id, member_id) {
            let _ = self.sessions.close(&info.session_id);
        }
        Ok(())
    }

    pub fn update_member(
        &self,
        workspace_id: &str,
        member_id: &str,
        patch: MemberPatch,
    ) -> Result<(), String> {
        let title = match patch.title {
            Some(t) => Some(validate_title(Some(t))?),
            None => None,
        };
        let color = match patch.color {
            Some(c) => Some(validate_color(Some(&c))?),
            None => None,
        };
        if let Some(z) = patch.zoom {
            if !(-6..=12).contains(&z) {
                return Err("zoom must be between -6 and +12".into());
            }
        }
        let start_dir = match patch.start_dir {
            Some(d) => Some(validate_dir(Some(&d))?),
            None => None,
        };
        let renamed = title.is_some();
        let shown = self.with_workspace(workspace_id, |w, reg, rs| {
            let m = w.member_mut(member_id).ok_or("member not found")?;
            if let Some(t) = title {
                m.title = t;
            }
            if let Some(v) = patch.auto_restart {
                m.auto_restart = v;
            }
            if let Some(v) = patch.detach_expected {
                m.detach_expected = v;
            }
            if let Some(c) = color {
                m.color = c;
            }
            if let Some(d) = start_dir {
                m.start_dir = d;
            }
            if let Some(z) = patch.zoom {
                m.zoom = z;
            }
            // Title a running session should show: the member title, or the
            // launcher's alias/name when the title was reset.
            let launcher = reg.effective(&m.launcher_id, rs);
            Ok(m.title.clone().unwrap_or_else(|| {
                launcher
                    .map(|e| e.decision.alias.unwrap_or(e.launcher.display_name))
                    .unwrap_or_else(|| m.launcher_id.clone())
            }))
        })?;
        if renamed {
            if let Some(info) = self.sessions.find_by_member(workspace_id, member_id) {
                let _ = self.sessions.rename(&info.session_id, &shown);
            }
        }
        Ok(())
    }

    pub fn save_layout(
        &self,
        workspace_id: &str,
        mode: LayoutMode,
        json: Option<Value>,
        focused_member: Option<String>,
    ) -> Result<(), String> {
        self.with_workspace(workspace_id, |w, _, _| {
            let ids: Vec<String> = w.members.iter().map(|m| m.id.clone()).collect();
            if let Some(json) = &json {
                validate_layout(json, &ids)?;
            }
            if let Some(f) = &focused_member {
                if !ids.contains(f) {
                    return Err("focused member is not part of the workspace".into());
                }
            }
            w.layout = LayoutState {
                mode,
                json,
                focused_member,
            };
            Ok(())
        })
    }
}
