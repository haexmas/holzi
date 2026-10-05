//! Passkey commands (spec 034, FR-004, `contracts/tauri-commands.md` §Passkeys): rename and
//! delete, and since spec 036 unlink ("Verweis lösen", research R6). The window never creates a
//! passkey and never receives a key.

use serde::Deserialize;
use tauri::State;
use ts_rs::TS;

use super::service;
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::state::AppState;

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct PasskeyRenameArgs {
    pub passkey_id: String,
    /// `None` clears the nickname.
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct PasskeyDeleteArgs {
    pub passkey_id: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct PasskeyUnlinkArgs {
    /// The entry that shows the passkey of another entry.
    pub item_id: String,
    pub passkey_id: String,
}

#[tauri::command]
pub async fn passwords_passkey_rename(
    state: State<'_, AppState>,
    args: PasskeyRenameArgs,
) -> Result<()> {
    service(&state)?
        .passkey_rename(&Caller::User, args.passkey_id, args.nickname)
        .await
}

#[tauri::command]
pub async fn passwords_passkey_delete(
    state: State<'_, AppState>,
    args: PasskeyDeleteArgs,
) -> Result<()> {
    service(&state)?
        .passkey_delete(&Caller::User, args.passkey_id)
        .await
}

/// Drops only the link; the passkey stays at its own entry.
#[tauri::command]
pub async fn passwords_passkey_unlink(
    state: State<'_, AppState>,
    args: PasskeyUnlinkArgs,
) -> Result<()> {
    service(&state)?
        .passkey_unlink(&Caller::User, args.item_id, args.passkey_id)
        .await
}
