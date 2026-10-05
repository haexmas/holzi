//! History commands (spec 034, US4, FR-017, `contracts/tauri-commands.md` §Verlauf). Each calls
//! only the service as `Caller::User`. A state is read masked; a secret of a state comes only
//! through `passwords_history_reveal` (or goes to the clipboard through `passwords_history_copy`).

use serde::Deserialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use super::read::{copy_to_clipboard, CopyResult, ItemIdArgs};
use super::service;
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::passwords::model::{
    HistorySecret, RestoreOutcome, RevealedSecret, SnapshotHeader, SnapshotView,
};
use crate::state::AppState;

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SnapshotIdArgs {
    pub snapshot_id: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct HistoryRevealArgs {
    pub snapshot_id: String,
    pub field: HistorySecret,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct HistoryRestoreArgs {
    pub item_id: String,
    pub snapshot_id: String,
    pub expected_updated_at: String,
}

#[tauri::command]
pub async fn passwords_history_list(
    state: State<'_, AppState>,
    args: ItemIdArgs,
) -> Result<Vec<SnapshotHeader>> {
    service(&state)?
        .history_list(&Caller::User, args.item_id)
        .await
}

#[tauri::command]
pub async fn passwords_history_get(
    state: State<'_, AppState>,
    args: SnapshotIdArgs,
) -> Result<SnapshotView> {
    service(&state)?
        .history_get(&Caller::User, args.snapshot_id)
        .await
}

#[tauri::command]
pub async fn passwords_history_reveal(
    state: State<'_, AppState>,
    args: HistoryRevealArgs,
) -> Result<RevealedSecret> {
    service(&state)?
        .history_reveal(&Caller::User, args.snapshot_id, args.field)
        .await
}

/// Copies a secret of a state to the clipboard; like `passwords_copy_field`, the value is not
/// returned.
#[tauri::command]
pub async fn passwords_history_copy(
    app: AppHandle,
    state: State<'_, AppState>,
    args: HistoryRevealArgs,
) -> Result<CopyResult> {
    let secret = service(&state)?
        .history_reveal(&Caller::User, args.snapshot_id, args.field)
        .await?;
    copy_to_clipboard(app, &state, &secret.value).await
}

#[tauri::command]
pub async fn passwords_history_restore(
    state: State<'_, AppState>,
    args: HistoryRestoreArgs,
) -> Result<RestoreOutcome> {
    service(&state)?
        .history_restore(
            &Caller::User,
            args.item_id,
            args.snapshot_id,
            args.expected_updated_at,
        )
        .await
}
