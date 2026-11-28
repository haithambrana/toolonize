//! Execution: launch plans, sessions, external launches and SSH auto-reconnect.

use super::*;

impl Core {
    pub(super) fn plan(&self, workspace_id: &str, member_id: &str) -> Result<LaunchPlan, String> {
        check_id(workspace_id)?;
        check_id(member_id)?;
        let data = self.data.locked();
        let w = data
            .state
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .ok_or("workspace not found")?;
        policy::plan_start(w, member_id, &data.registry, &data.state.registry)
    }

    pub(super) fn audit_plan(
        &self,
        action: &str,
        ws: &str,
        member: &str,
        result: &Result<LaunchPlan, String>,
    ) {
        let (outcome, launcher, fp, detail) = match result {
            Ok(p) => (
                Outcome::Allowed,
                Some(p.launcher_id.as_str()),
                Some(p.fingerprint.as_str()),
                p.interpreter
                    .then_some("interpreter execution (authorized)"),
            ),
            Err(e) => (Outcome::Denied, None, None, Some(e.as_str())),
        };
        self.audit.record(
            action,
            outcome,
            Record {
                workspace_id: Some(ws),
                member_id: Some(member),
                launcher_id: launcher,
                fingerprint: fp,
                detail,
            },
        );
    }

    pub fn start_session(
        &self,
        workspace_id: &str,
        member_id: &str,
        rows: u16,
        cols: u16,
    ) -> Result<SessionInfo, String> {
        self.start_session_with(workspace_id, member_id, rows, cols, None)
    }

    /// `resume_cwd`: the folder a resumed terminal was last in (used unless
    /// the launcher names its own working directory).
    pub(super) fn start_session_with(
        &self,
        workspace_id: &str,
        member_id: &str,
        rows: u16,
        cols: u16,
        resume_cwd: Option<&str>,
    ) -> Result<SessionInfo, String> {
        let result = self.plan(workspace_id, member_id).and_then(|p| {
            if p.mode == LaunchMode::Pty {
                Ok(p)
            } else {
                Err("external applications are launched, not embedded".into())
            }
        });
        self.audit_plan("session.start", workspace_id, member_id, &result);
        let mut plan = result?;
        if let Some(dir) = resume_cwd
            .filter(|_| !plan.launcher_cwd && !plan.remote)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute() && p.is_dir())
        {
            plan.spec.cwd = Some(dir);
        }
        let fresh = self
            .sessions
            .find_by_member(workspace_id, member_id)
            .is_none();
        let startup_command = plan.startup_command.clone();
        let (prelude, prelude_cols) = if fresh {
            self.take_prelude(workspace_id, member_id)
        } else {
            (Vec::new(), 0)
        };
        self.sessions
            .start(SessionPlan {
                workspace_id: workspace_id.to_string(),
                member_id: member_id.to_string(),
                title: plan.title,
                spec: plan.spec,
                remote: plan.remote,
                detach_expected: plan.detach_expected,
                dims: PtyDims::clamped(rows, cols),
                prelude,
                prelude_cols,
            })
            .map_err(|e| e.to_string())
            .inspect(|info| {
                if let (true, Some(command)) = (fresh, startup_command) {
                    self.type_startup_command(&info.session_id, command, info.prelude_bytes);
                }
            })
    }

    pub(super) fn replan_session(
        &self,
        session_id: &str,
        action: &str,
    ) -> Result<(SessionInfo, LaunchPlan), String> {
        let info = self.sessions.info(session_id).map_err(|e| e.to_string())?;
        let result = self.plan(&info.workspace_id, &info.member_id);
        self.audit_plan(action, &info.workspace_id, &info.member_id, &result);
        Ok((info, result?))
    }

    pub fn restart_session(&self, session_id: &str) -> Result<SessionInfo, String> {
        let (before, plan) = self.replan_session(session_id, "session.restart")?;
        let info = self
            .sessions
            .restart(session_id, plan.spec)
            .map_err(|e| e.to_string())?;
        if let Some(command) = plan.startup_command {
            self.type_startup_command(session_id, command, before.end_offset);
        }
        Ok(info)
    }

    pub fn reconnect_session(&self, session_id: &str) -> Result<SessionInfo, String> {
        let (before, plan) = self.replan_session(session_id, "session.reconnect")?;
        let info = self
            .sessions
            .reconnect(session_id, plan.spec)
            .map_err(|e| e.to_string())?;
        if let Some(command) = plan.startup_command {
            self.type_startup_command(session_id, command, before.end_offset);
        }
        Ok(info)
    }

    /// Type a member's startup command into its new shell once the shell
    /// printed something (its prompt), or after 3 s — exactly as if the user
    /// typed it (ADR-016). Never passed to a program as arguments.
    fn type_startup_command(&self, session_id: &str, command: String, after: u64) {
        let Some(core) = self.arc() else { return };
        let weak = Arc::downgrade(&core);
        drop(core);
        let session_id = session_id.to_string();
        let _ = std::thread::Builder::new()
            .name("startup-command".into())
            .spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(3);
                loop {
                    let Some(core) = weak.upgrade() else { return };
                    match core.sessions.info(&session_id) {
                        Ok(i) if i.end_offset > after => break,
                        Ok(_) if Instant::now() < deadline => {}
                        Ok(_) => break,
                        Err(_) => return,
                    }
                    drop(core);
                    std::thread::sleep(Duration::from_millis(50));
                }
                // Let the prompt finish drawing.
                std::thread::sleep(Duration::from_millis(150));
                if let Some(core) = weak.upgrade() {
                    let _ = core
                        .sessions
                        .write(&session_id, format!("{command}\r").as_bytes());
                }
            });
    }

    pub fn attach(
        &self,
        session_id: &str,
        from_offset: u64,
        sink: Arc<dyn SessionSink>,
    ) -> Result<crate::session::AttachInfo, String> {
        self.sessions
            .attach(session_id, from_offset, sink)
            .map_err(|e| e.to_string())
    }

    pub fn launch_external(&self, workspace_id: &str, member_id: &str) -> Result<u32, String> {
        let result = self.plan(workspace_id, member_id).and_then(|p| {
            if p.mode == LaunchMode::External {
                Ok(p)
            } else {
                Err("terminal members run embedded".into())
            }
        });
        self.audit_plan("external.launch", workspace_id, member_id, &result);
        let plan = result?;
        match external::launch_detached(&plan) {
            Ok(pid) => {
                self.notice("info", format!("Launched {}", plan.title));
                Ok(pid)
            }
            Err(error) => {
                self.audit.record(
                    "external.launch",
                    Outcome::Failed,
                    Record {
                        workspace_id: Some(workspace_id),
                        member_id: Some(member_id),
                        launcher_id: Some(&plan.launcher_id),
                        detail: Some(&error),
                        ..Default::default()
                    },
                );
                Err(error)
            }
        }
    }

    /// Startup policy executor. Failures isolate per member.
    pub fn open_workspace(
        &self,
        workspace_id: &str,
        choice: Option<OpenChoice>,
    ) -> Result<OpenResult, String> {
        check_id(workspace_id)?;
        let (policy, members, staged) = {
            let data = self.data.locked();
            let w = data
                .state
                .workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .ok_or("workspace not found")?;
            (
                w.startup_policy,
                w.members
                    .iter()
                    .map(|m| (m.id.clone(), m.member_type))
                    .collect::<Vec<_>>(),
                w.staged_import,
            )
        };
        self.set_active(Some(workspace_id))?;
        let first_open = self.opened.locked().insert(workspace_id.to_string());
        if choice.is_none() && !first_open {
            return Ok(OpenResult {
                needs_choice: false,
                outcomes: vec![],
            });
        }
        let choice = match (policy, choice) {
            (_, Some(c)) => c,
            (StartupPolicy::Ask, None) if !members.is_empty() && !staged => {
                return Ok(OpenResult {
                    needs_choice: true,
                    outcomes: vec![],
                })
            }
            (StartupPolicy::Ask, None) | (StartupPolicy::RestoreOnly, None) => {
                OpenChoice::default()
            }
            (StartupPolicy::RestoreTerminals, None) => OpenChoice {
                start_terminals: true,
                launch_external: false,
            },
            (StartupPolicy::RestoreAll, None) => OpenChoice {
                start_terminals: true,
                launch_external: true,
            },
        };
        let mut outcomes = Vec::new();
        for (member_id, member_type) in members {
            let pty = member_type.runs_in_pty();
            let wanted = if pty {
                choice.start_terminals
            } else {
                choice.launch_external
            };
            let (outcome, reason) = if !wanted {
                ("skipped", None)
            } else if pty
                && self
                    .sessions
                    .find_by_member(workspace_id, &member_id)
                    .is_some()
            {
                ("running", None)
            } else {
                let result = if pty {
                    self.start_session(workspace_id, &member_id, 24, 80)
                        .map(|_| ())
                } else {
                    self.launch_external(workspace_id, &member_id).map(|_| ())
                };
                match result {
                    Ok(()) => ("started", None),
                    Err(e) => ("denied", Some(e)),
                }
            };
            outcomes.push(MemberOutcome {
                member_id,
                outcome: outcome.into(),
                reason,
            });
        }
        Ok(OpenResult {
            needs_choice: false,
            outcomes,
        })
    }

    pub(super) fn on_session(&self, info: &SessionInfo) {
        self.memory_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.bus.emit(AppEvent::Session { info: info.clone() });
        crate::logging::info(
            "session",
            &format!(
                "{} gen {}: {:?}",
                info.session_id, info.generation, info.process_state
            ),
        );
        match info.process_state {
            crate::session::ProcessState::Disconnected { .. } => {
                self.maybe_reconnect(info);
                return;
            }
            crate::session::ProcessState::Exited { .. } => {
                self.stop_reconnect(&info.session_id, false);
            }
            crate::session::ProcessState::Closed => {
                self.stop_reconnect(&info.session_id, false);
                return;
            }
            _ => return,
        }
        let auto = {
            let data = self.data.locked();
            data.state
                .workspaces
                .iter()
                .find(|w| w.id == info.workspace_id)
                .and_then(|w| w.member(&info.member_id))
                .map(|m| m.auto_restart)
                .unwrap_or(false)
        };
        if !auto {
            return;
        }
        // Opt-in auto restart, bounded to avoid restart storms.
        let allowed = {
            let mut restarts = self.restarts.locked();
            let history = restarts.entry(info.session_id.clone()).or_default();
            history.retain(|t| t.elapsed() < Duration::from_secs(60));
            if history.len() >= 5 {
                false
            } else {
                history.push(Instant::now());
                true
            }
        };
        if !allowed {
            self.notice(
                "warning",
                format!("{} keeps exiting; automatic restart paused", info.title),
            );
            return;
        }
        if let Some(core) = self.arc() {
            let id = info.session_id.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(1));
                let _ = core.restart_session(&id);
            });
        }
    }

    /// SSH auto-reconnect. The session layer marks a remote session
    /// `Disconnected` when ssh exits with 255 (connection failed or dropped);
    /// a normal `exit` returns the remote shell's status and a user Stop ends
    /// as `Closed`, so neither reconnects. Reconnects the same authorized
    /// member with exponential backoff and bounded attempts.
    /// Returns true when a reconnect was scheduled or abandoned here.
    pub(super) fn maybe_reconnect(&self, info: &SessionInfo) -> bool {
        let dropped = matches!(
            info.process_state,
            crate::session::ProcessState::Disconnected { .. }
        );
        let enabled = self.data.locked().settings.ssh_auto_reconnect;
        if !info.remote || !dropped || !enabled {
            self.stop_reconnect(&info.session_id, false);
            return false;
        }
        let stable = now_ms().saturating_sub(info.started_at_ms) >= RECONNECT_STABLE_MS;
        let token = self
            .reconnect_token
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        let attempt = {
            let mut map = self.reconnects.locked();
            let entry = map
                .entry(info.session_id.clone())
                .or_insert(Reconnect { attempt: 0, token });
            if stable {
                entry.attempt = 0;
            }
            entry.attempt += 1;
            entry.token = token;
            entry.attempt
        };
        if attempt > RECONNECT_MAX_ATTEMPTS {
            self.stop_reconnect(&info.session_id, true);
            self.notice(
                "warning",
                format!(
                    "{}: gave up reconnecting after {RECONNECT_MAX_ATTEMPTS} attempts",
                    info.title
                ),
            );
            return true;
        }
        let delay = reconnect_delay(attempt);
        self.bus.emit(AppEvent::Reconnect {
            session_id: info.session_id.clone(),
            active: true,
            attempt,
            max_attempts: RECONNECT_MAX_ATTEMPTS,
            delay_ms: delay.as_millis() as u64,
        });
        if let Some(core) = self.arc() {
            let id = info.session_id.clone();
            std::thread::spawn(move || {
                std::thread::sleep(delay);
                let current = core.reconnects.locked().get(&id).map(|r| r.token);
                if current != Some(token) {
                    return; // cancelled or superseded
                }
                let still_down = core.sessions.info(&id).is_ok_and(|i| {
                    matches!(
                        i.process_state,
                        crate::session::ProcessState::Disconnected { .. }
                    )
                });
                if still_down {
                    let _ = core.reconnect_session(&id);
                }
            });
        }
        true
    }

    pub(super) fn stop_reconnect(&self, session_id: &str, announce_always: bool) {
        let removed = self.reconnects.locked().remove(session_id).is_some();
        if removed || announce_always {
            self.bus.emit(AppEvent::Reconnect {
                session_id: session_id.to_string(),
                active: false,
                attempt: 0,
                max_attempts: RECONNECT_MAX_ATTEMPTS,
                delay_ms: 0,
            });
        }
    }

    /// The user stopped automatic reconnection for a session.
    pub fn cancel_reconnect(&self, session_id: &str) -> Result<(), String> {
        check_id(session_id)?;
        self.stop_reconnect(session_id, true);
        Ok(())
    }

    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        self.stop_reconnect(session_id, false);
        let info = self.sessions.info(session_id).ok();
        self.sessions.close(session_id).map_err(|e| e.to_string())?;
        // Closed on purpose: it does not come back at the next start.
        if let Some(info) = info {
            self.forget_member_memory(&info.workspace_id, &info.member_id);
        }
        self.bus.emit(AppEvent::SessionClosed {
            session_id: session_id.to_string(),
        });
        Ok(())
    }
}
