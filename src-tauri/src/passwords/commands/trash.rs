//! Trash commands (spec 034, US4, FR-015, FR-016, `contracts/tauri-commands.md` §Papierkorb). Each
//! calls only the service as `Caller::User`; deleting for good is open to nobody else (Z11).

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use super::service;
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::passwords::model::Target;
use crate::state::AppState;

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct TargetsArgs {
    pub targets: Vec<Target>,
}

/// How many entries and folders a trash command touched, folders counted with their content.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct AffectedResult {
    pub affected: u32,
}

/// Moves entries and folders into the trash; what is in the trash already is deleted for good.
#[tauri::command]
pub async fn passwords_trash(
    state: State<'_, AppState>,
    args: TargetsArgs,
) -> Result<AffectedResult> {
    let affected = service(&state)?
        .trash_targets(&Caller::User, args.targets)
        .await?;
    Ok(AffectedResult { affected })
}

/// Takes entries and folders out of the trash, to where they were or to the top level.
#[tauri::command]
pub async fn passwords_restore(
    state: State<'_, AppState>,
    args: TargetsArgs,
) -> Result<AffectedResult> {
    let affected = service(&state)?
        .restore(&Caller::User, args.targets)
        .await?;
    Ok(AffectedResult { affected })
}

/// Deletes what is in the trash for good, children first.
#[tauri::command]
pub async fn passwords_delete_permanently(
    state: State<'_, AppState>,
    args: TargetsArgs,
) -> Result<AffectedResult> {
    let affected = service(&state)?
        .delete_permanently(&Caller::User, args.targets)
        .await?;
    Ok(AffectedResult { affected })
}

/// Empties the trash; the window asks for confirmation first.
#[tauri::command]
pub async fn passwords_empty_trash(state: State<'_, AppState>) -> Result<AffectedResult> {
    let affected = service(&state)?.empty_trash(&Caller::User).await?;
    Ok(AffectedResult { affected })
}
