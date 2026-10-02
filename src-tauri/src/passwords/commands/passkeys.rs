//! Passkey commands (spec 034, FR-004, `contracts/tauri-commands.md` §Passkeys): rename and
//! delete. The window never creates a passkey and never receives a key.

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
