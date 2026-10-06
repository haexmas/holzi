//! `extension_install_preview` and `extension_install` (US1).

use std::path::PathBuf;

use serde::Deserialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use super::emit_changed;
use crate::error::{HolziError, Result};
use crate::extensions::registry::install::{
    install, install_preview, read_bundle_file, InstallPreview, PermissionChoice,
};
use crate::extensions::registry::list::{list, ExtensionSummary};
use crate::passwords::clock::unix_millis;
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::wm_session_commands::current_device_uuid;

fn join_error(error: tauri::Error) -> HolziError {
    HolziError::ExtensionInstall {
        reason: format!("install task: {error}"),
    }
}

/// Reads the file chosen in the file dialog and checks it; writes nothing.
#[tauri::command]
pub async fn extension_install_preview(
    state: State<'_, AppState>,
    path: String,
) -> Result<InstallPreview> {
    let db = active_database(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = read_bundle_file(&PathBuf::from(path))?;
        db.read_blocking(move |q| install_preview(q, &bytes).map_err(Into::into))
    })
    .await
    .map_err(join_error)?
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct ExtensionInstallArgs {
    pub path: String,
    /// The choice per declared permission; a declaration without a choice becomes `ask`.
    pub accepted: Vec<PermissionChoice>,
    /// The user confirmed a downgrade or the replacement of another bundle of the same version.
    #[serde(default)]
    pub confirmed: bool,
}

/// Checks the file again and installs it for the vault (FR-005).
#[tauri::command]
pub async fn extension_install(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ExtensionInstallArgs,
) -> Result<ExtensionSummary> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    let id = tauri::async_runtime::spawn_blocking(move || {
        let bytes = read_bundle_file(&PathBuf::from(&args.path))?;
        let now = unix_millis(std::time::SystemTime::now());
        let installed = install(&db, &bytes, args.accepted, args.confirmed, device, now)?;
        let id = installed.ids.extension_id.to_string();
        let summary = db
            .read_blocking(move |q| list(q, device).map_err(Into::into))?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or(HolziError::ExtensionNotFound)?;
        Ok::<_, HolziError>(summary)
    })
    .await
    .map_err(join_error)??;
    emit_changed(&app, vec![id.id.clone()]);
    Ok(id)
}
