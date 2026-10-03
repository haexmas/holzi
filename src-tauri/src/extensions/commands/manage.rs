//! `extension_list` and `extension_icon` (US1) and `extension_remove` (US4, R11); enabling and
//! the settings around removing follow in L3.

use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::extensions::registry::list::{icon_data_url, list, ExtensionSummary};
use crate::extensions::registry::remove::remove;
use crate::passwords::clock::unix_millis;
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::wm_session_commands::current_device_uuid;

fn parse_extension_id(text: &str) -> Result<Uuid> {
    Uuid::parse_str(text).map_err(|_| HolziError::ExtensionNotFound)
}

/// Every extension of the vault, ordered by title.
#[tauri::command]
pub async fn extension_list(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ExtensionSummary>> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    db.read(move |q| list(q, device).map_err(Into::into)).await
}

/// The icon of the effective bundle as a `data:` URL.
#[tauri::command]
pub async fn extension_icon(
    state: State<'_, AppState>,
    extension_id: String,
) -> Result<Option<String>> {
    let id = parse_extension_id(&extension_id)?;
    let db = active_database(&state)?;
    db.read(move |q| icon_data_url(q, id).map_err(Into::into))
        .await
}

/// Removes an extension from the vault; with `delete_data` every device also drops its tables.
/// Each device clears up when the removal reaches it (research R11).
#[tauri::command]
pub async fn extension_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    extension_id: String,
    delete_data: bool,
) -> Result<()> {
    let id = parse_extension_id(&extension_id)?;
    let db = active_database(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        remove(
            &db,
            id,
            delete_data,
            unix_millis(std::time::SystemTime::now()),
        )
    })
    .await
    .map_err(|e| HolziError::InvalidInput {
        reason: format!("remove task: {e}"),
    })??;
    super::emit_changed(&app, vec![extension_id]);
    Ok(())
}
