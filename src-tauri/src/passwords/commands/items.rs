//! Write commands for entries (spec 034, `contracts/tauri-commands.md` §Schreiben: Einträge).

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use super::service;
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::passwords::model::{ItemInput, ItemPatch};
use crate::state::AppState;

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct CreateItemArgs {
    pub input: ItemInput,
    #[ts(optional)]
    pub group_id: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct CreateItemResult {
    pub item_id: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct UpdateItemArgs {
    pub item_id: String,
    /// The `updatedAt` the window read; a newer one on disk is a conflict.
    pub expected_updated_at: String,
    pub patch: ItemPatch,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct UpdateItemResult {
    pub updated_at: String,
}

/// Creates an entry; an invalid TOTP secret is refused, a missing title is fine.
#[tauri::command]
pub async fn passwords_create_item(
    state: State<'_, AppState>,
    args: CreateItemArgs,
) -> Result<CreateItemResult> {
    let item_id = service(&state)?
        .create_item(&Caller::User, &[], args.input, args.group_id)
        .await?;
    Ok(CreateItemResult { item_id })
}

/// A partial update with the conflict check.
#[tauri::command]
pub async fn passwords_update_item(
    state: State<'_, AppState>,
    args: UpdateItemArgs,
) -> Result<UpdateItemResult> {
    let updated_at = service(&state)?
        .update_item(
            &Caller::User,
            &[],
            args.item_id,
            args.expected_updated_at,
            args.patch,
        )
        .await?;
    Ok(UpdateItemResult { updated_at })
}
