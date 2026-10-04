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

/// What to delete for good; with `inline_references` the placeholders on the deleted entries are
/// first replaced by their values in the entries that hold them (spec 036, FR-048).
#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct DeletePermanentlyArgs {
    pub targets: Vec<Target>,
    #[serde(default)]
    #[ts(optional)]
    pub inline_references: Option<bool>,
}

/// Emptying the trash, with `inline_references` as for deleting for good.
#[derive(Debug, Default, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct EmptyTrashArgs {
    #[serde(default)]
    #[ts(optional)]
    pub inline_references: Option<bool>,
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
    args: DeletePermanentlyArgs,
) -> Result<AffectedResult> {
    let affected = service(&state)?
        .delete_permanently(
            &Caller::User,
            args.targets,
            args.inline_references.unwrap_or(false),
        )
        .await?;
    Ok(AffectedResult { affected })
}

/// Empties the trash; the window asks for confirmation first.
#[tauri::command]
pub async fn passwords_empty_trash(
    state: State<'_, AppState>,
    args: Option<EmptyTrashArgs>,
) -> Result<AffectedResult> {
    let inline = args.unwrap_or_default().inline_references.unwrap_or(false);
    let affected = service(&state)?.empty_trash(&Caller::User, inline).await?;
    Ok(AffectedResult { affected })
}
