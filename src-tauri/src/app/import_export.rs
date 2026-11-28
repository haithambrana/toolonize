//! Non-secret workspace import and export.

use super::*;

impl Core {
    pub fn export_workspace(
        &self,
        workspace_id: &str,
        machine_bound: bool,
    ) -> Result<String, String> {
        check_id(workspace_id)?;
        let data = self.data.locked();
        let w = data
            .state
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .ok_or("workspace not found")?;
        let names = |id: &str| data.registry.get(id).map(|l| l.display_name.clone());
        transfer::export(w, &names, machine_bound)
            .map_err(|findings| format!("export refused: {}", findings.join("; ")))
    }

    pub fn import_workspace(&self, text: &str) -> Result<ImportReport, String> {
        let imported = {
            let data = self.data.locked();
            let exists = |id: &str| data.registry.get(id).is_some();
            transfer::import(text, &transfer::Resolver { exists: &exists })?
        };
        let report = imported.report.clone();
        self.mutate(|s, _| {
            if s.workspaces.len() >= MAX_WORKSPACES {
                return Err(format!("at most {MAX_WORKSPACES} workspaces"));
            }
            for spec in &imported.synthetic {
                if !s.registry.synthetic.contains(spec)
                    && s.registry.synthetic.len() < MAX_SYNTHETIC
                {
                    s.registry.synthetic.push(spec.clone());
                }
            }
            s.workspaces.push(imported.workspace.clone());
            Ok(())
        })?;
        self.audit.record(
            "workspace.import",
            Outcome::Recorded,
            Record {
                workspace_id: Some(&report.workspace_id),
                detail: Some("imported into review staging; nothing authorized"),
                ..Default::default()
            },
        );
        if !imported.synthetic.is_empty() {
            self.rescan()?;
        }
        self.changed();
        Ok(report)
    }
}
