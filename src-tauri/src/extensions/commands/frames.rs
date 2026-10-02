//! `extension_frame_open` and `extension_frame_close` (contracts/tauri-commands.md §Rahmen).

use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::extensions::protocol::{encode_path, prefix};
use crate::extensions::registry::start::start;
use crate::passwords::clock::unix_millis;
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::wm_session_commands::current_device_uuid;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct FrameOpened {
    /// Id of the frame session for bridge calls; never handed to the extension.
    pub frame: String,
    /// URL of the entry page with the start token.
    pub url: String,
}

/// Starts the extension on this device if needed and opens a frame session for one tab.
#[tauri::command]
pub async fn extension_frame_open(
    app: AppHandle,
    state: State<'_, AppState>,
    extension_id: String,
    tab_id: String,
) -> Result<FrameOpened> {
    let extension_id = Uuid::parse_str(&extension_id).map_err(|_| HolziError::ExtensionNotFound)?;
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    let started = tauri::async_runtime::spawn_blocking(move || {
        start(
            &db,
            extension_id,
            device,
            unix_millis(std::time::SystemTime::now()),
        )
    })
    .await
    .map_err(|e| HolziError::ExtensionNotReady {
        status: format!("start task: {e}"),
    })??;
    let host = state.extensions();
    let started = host.remember_started(started);
    let session = host.frames.open(extension_id, started.bundle_id, &tab_id);
    Ok(FrameOpened {
        frame: session.frame.clone(),
        url: format!(
            "{}{}?hf={}",
            prefix(extension_id),
            encode_path(&started.entry),
            session.token
        ),
    })
}

/// Ends a frame session.
#[tauri::command]
pub async fn extension_frame_close(state: State<'_, AppState>, frame: String) -> Result<()> {
    state.extensions().frames.close(&frame);
    Ok(())
}
