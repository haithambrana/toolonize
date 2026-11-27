//! Workspace and member commands. `member_launch` is one of the two
//! execution entry points; it names a member, never a command.

use super::{blocking, CoreState};
use crate::app::{MemberPatch, OpenChoice, OpenResult, TemplateSummary, WorkspaceView};
use crate::backup::RestoreReport;
use crate::transfer::ImportReport;
use crate::workspace::{LayoutMode, StartupPolicy};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct WorkspaceList {
    pub workspaces: Vec<WorkspaceView>,
    pub active: Option<String>,
}

#[tauri::command]
pub async fn workspace_list(core: CoreState<'_>) -> Result<WorkspaceList, String> {
    let (workspaces, active) = core.workspaces();
    Ok(WorkspaceList { workspaces, active })
}

#[tauri::command]
pub async fn workspace_create(core: CoreState<'_>, name: String) -> Result<String, String> {
    blocking(&core, move |c| c.create_workspace(&name)).await
}

#[tauri::command]
pub async fn workspace_rename(
    core: CoreState<'_>,
    workspace_id: String,
    name: String,
) -> Result<(), String> {
    blocking(&core, move |c| c.rename_workspace(&workspace_id, &name)).await
}

#[tauri::command]
pub async fn workspace_set_color(
    core: CoreState<'_>,
    workspace_id: String,
    color: Option<String>,
) -> Result<(), String> {
    blocking(&core, move |c| {
        c.set_workspace_color(&workspace_id, color.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn workspace_delete(core: CoreState<'_>, workspace_id: String) -> Result<(), String> {
    blocking(&core, move |c| c.delete_workspace(&workspace_id)).await
}

#[tauri::command]
pub async fn workspace_set_active(
    core: CoreState<'_>,
    workspace_id: Option<String>,
) -> Result<(), String> {
    blocking(&core, move |c| c.set_active(workspace_id.as_deref())).await
}

#[tauri::command]
pub async fn workspace_set_policy(
    core: CoreState<'_>,
    workspace_id: String,
    policy: StartupPolicy,
) -> Result<(), String> {
    blocking(&core, move |c| c.set_policy(&workspace_id, policy)).await
}

#[tauri::command]
pub async fn workspace_accept_import(
    core: CoreState<'_>,
    workspace_id: String,
) -> Result<(), String> {
    blocking(&core, move |c| c.accept_import(&workspace_id)).await
}

#[tauri::command]
pub async fn workspace_save_layout(
    core: CoreState<'_>,
    workspace_id: String,
    mode: LayoutMode,
    layout: Option<Value>,
    focused_member: Option<String>,
) -> Result<(), String> {
    blocking(&core, move |c| {
        c.save_layout(&workspace_id, mode, layout, focused_member)
    })
    .await
}

#[tauri::command]
pub async fn workspace_open(
    core: CoreState<'_>,
    workspace_id: String,
    choice: Option<OpenChoice>,
) -> Result<OpenResult, String> {
    blocking(&core, move |c| c.open_workspace(&workspace_id, choice)).await
}

#[tauri::command]
pub async fn workspace_export(
    core: CoreState<'_>,
    workspace_id: String,
    machine_bound: bool,
) -> Result<String, String> {
    blocking(&core, move |c| {
        c.export_workspace(&workspace_id, machine_bound)
    })
    .await
}

#[tauri::command]
pub async fn workspace_import(core: CoreState<'_>, bundle: String) -> Result<ImportReport, String> {
    blocking(&core, move |c| c.import_workspace(&bundle)).await
}

/// Copy a workspace with its members, layout and notes; returns the new id.
#[tauri::command]
pub async fn workspace_duplicate(
    core: CoreState<'_>,
    workspace_id: String,
) -> Result<String, String> {
    blocking(&core, move |c| c.duplicate_workspace(&workspace_id)).await
}

/// Write a full backup to Downloads; returns the file path (ADR-014).
#[tauri::command]
pub async fn backup_export(core: CoreState<'_>, include_settings: bool) -> Result<String, String> {
    blocking(&core, move |c| c.export_backup(include_settings)).await
}

/// Restore a backup next to the existing workspaces, staged for review.
#[tauri::command]
pub async fn backup_restore(
    core: CoreState<'_>,
    contents: String,
    restore_settings: bool,
) -> Result<RestoreReport, String> {
    blocking(&core, move |c| {
        c.restore_backup(&contents, restore_settings)
    })
    .await
}

#[tauri::command]
pub async fn template_list(core: CoreState<'_>) -> Result<Vec<TemplateSummary>, String> {
    Ok(core.templates())
}

/// Save a workspace's shape as a template; returns the template id.
#[tauri::command]
pub async fn template_save(
    core: CoreState<'_>,
    workspace_id: String,
    name: String,
) -> Result<String, String> {
    blocking(&core, move |c| c.save_template(&workspace_id, &name)).await
}

/// New workspace from a template; returns the workspace id.
#[tauri::command]
pub async fn template_create_workspace(
    core: CoreState<'_>,
    template_id: String,
    name: String,
) -> Result<String, String> {
    blocking(&core, move |c| c.create_from_template(&template_id, &name)).await
}

#[tauri::command]
pub async fn template_delete(core: CoreState<'_>, template_id: String) -> Result<(), String> {
    blocking(&core, move |c| c.delete_template(&template_id)).await
}

#[tauri::command]
pub async fn member_add(
    core: CoreState<'_>,
    workspace_id: String,
    launcher_id: String,
    fingerprint: String,
    title: Option<String>,
) -> Result<String, String> {
    blocking(&core, move |c| {
        c.add_member(&workspace_id, &launcher_id, &fingerprint, title)
    })
    .await
}

#[tauri::command]
pub async fn member_remove(
    core: CoreState<'_>,
    workspace_id: String,
    member_id: String,
) -> Result<(), String> {
    blocking(&core, move |c| c.remove_member(&workspace_id, &member_id)).await
}

#[tauri::command]
pub async fn member_update(
    core: CoreState<'_>,
    workspace_id: String,
    member_id: String,
    patch: MemberPatch,
) -> Result<(), String> {
    blocking(&core, move |c| {
        c.update_member(&workspace_id, &member_id, patch)
    })
    .await
}

#[tauri::command]
pub async fn member_authorize(
    core: CoreState<'_>,
    workspace_id: String,
    member_id: String,
    fingerprint: String,
) -> Result<(), String> {
    blocking(&core, move |c| {
        c.authorize_member(&workspace_id, &member_id, &fingerprint)
    })
    .await
}

#[tauri::command]
pub async fn member_launch(
    core: CoreState<'_>,
    workspace_id: String,
    member_id: String,
) -> Result<u32, String> {
    blocking(&core, move |c| c.launch_external(&workspace_id, &member_id)).await
}

/// Sticky notes of a workspace: the project note and one per member.
#[tauri::command]
pub async fn notes_get(
    core: CoreState<'_>,
    workspace_id: String,
) -> Result<crate::app::NotesView, String> {
    blocking(&core, move |c| c.notes_view(&workspace_id)).await
}

/// Save one note (`member_id` absent = project note). Durable on return.
#[tauri::command]
pub async fn note_save(
    core: CoreState<'_>,
    workspace_id: String,
    member_id: Option<String>,
    text: String,
) -> Result<u64, String> {
    blocking(&core, move |c| {
        c.save_note(&workspace_id, member_id.as_deref(), &text)
    })
    .await
}

/// Append a deleted workspace's notes to this workspace's project note.
#[tauri::command]
pub async fn notes_restore(
    core: CoreState<'_>,
    trash_id: String,
    workspace_id: String,
) -> Result<(), String> {
    blocking(&core, move |c| {
        c.restore_trashed_notes(&trash_id, &workspace_id)
    })
    .await
}

/// Project folder for new terminals (`dir` empty/absent clears it).
#[tauri::command]
pub async fn workspace_set_project_dir(
    core: CoreState<'_>,
    workspace_id: String,
    dir: Option<String>,
) -> Result<(), String> {
    blocking(&core, move |c| {
        c.set_project_dir(&workspace_id, dir.as_deref())
    })
    .await
}
