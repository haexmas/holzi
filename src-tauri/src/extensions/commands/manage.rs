//! `extension_list` and `extension_icon` (US1); enabling and removing follow in L3.

use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::extensions::registry::list::{icon_data_url, list, ExtensionSummary};
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
