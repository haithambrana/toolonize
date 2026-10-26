//! Launcher registry commands: inspection, review decisions, sources.
//! None of these execute anything (FR-004).

use super::{blocking, CoreState};
use crate::app::RegistryView;
use crate::registry::Diff;
use crate::shells::SyntheticSpec;

#[tauri::command]
pub async fn registry_view(core: CoreState<'_>) -> Result<RegistryView, String> {
    Ok(core.registry_view())
}

#[tauri::command]
pub async fn registry_rescan(core: CoreState<'_>) -> Result<Diff, String> {
    blocking(&core, |c| c.rescan()).await
}

#[tauri::command]
pub async fn registry_set_hidden(
    core: CoreState<'_>,
    launcher_id: String,
    hidden: bool,
) -> Result<(), String> {
    blocking(&core, move |c| c.set_hidden(&launcher_id, hidden)).await
}

#[tauri::command]
pub async fn registry_set_alias(
    core: CoreState<'_>,
    launcher_id: String,
    alias: Option<String>,
) -> Result<(), String> {
    blocking(&core, move |c| c.set_alias(&launcher_id, alias)).await
}

#[tauri::command]
pub async fn registry_approve(
    core: CoreState<'_>,
    launcher_id: String,
    fingerprint: String,
) -> Result<(), String> {
    blocking(&core, move |c| c.approve(&launcher_id, &fingerprint)).await
}

#[tauri::command]
pub async fn registry_revoke(core: CoreState<'_>, launcher_id: String) -> Result<(), String> {
    blocking(&core, move |c| c.revoke(&launcher_id)).await
}

#[tauri::command]
pub async fn registry_acknowledge(
    core: CoreState<'_>,
    launcher_ids: Vec<String>,
) -> Result<(), String> {
    blocking(&core, move |c| c.acknowledge(&launcher_ids)).await
}

#[tauri::command]
pub async fn custom_root_add(core: CoreState<'_>, root: String) -> Result<(), String> {
    blocking(&core, move |c| c.add_custom_root(&root)).await
}

#[tauri::command]
pub async fn custom_root_remove(core: CoreState<'_>, root: String) -> Result<(), String> {
    blocking(&core, move |c| c.remove_custom_root(&root)).await
}

#[tauri::command]
pub async fn synthetic_add(core: CoreState<'_>, spec: SyntheticSpec) -> Result<String, String> {
    blocking(&core, move |c| c.add_synthetic(spec)).await
}

#[tauri::command]
pub async fn synthetic_remove(core: CoreState<'_>, launcher_id: String) -> Result<(), String> {
    blocking(&core, move |c| c.remove_synthetic(&launcher_id)).await
}

/// Host aliases from the user's ssh config (ADR-011). Takes no path.
#[tauri::command]
pub async fn ssh_hosts(core: CoreState<'_>) -> Result<crate::sshconfig::HostList, String> {
    blocking(&core, move |c| Ok(c.ssh_hosts())).await
}
