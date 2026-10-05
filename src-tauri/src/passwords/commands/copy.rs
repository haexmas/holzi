//! The copy command (spec 036, `contracts/tauri-commands.md` §`passwords_copy`): calls only the
//! service as `Caller::User`.

use serde::Deserialize;
use tauri::State;
use ts_rs::TS;

use super::service;
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::passwords::copy::{CopyOptions, CopyReport};
use crate::passwords::model::Target;
use crate::state::AppState;

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct CopyArgs {
    pub targets: Vec<Target>,
    /// `null` is the top level.
    pub into_group_id: Option<String>,
    pub options: CopyOptions,
}

/// Copies entries and folders into a folder, all or nothing.
#[tauri::command]
pub async fn passwords_copy(state: State<'_, AppState>, args: CopyArgs) -> Result<CopyReport> {
    service(&state)?
        .copy(
            &Caller::User,
            args.targets,
            args.into_group_id,
            args.options,
        )
        .await
}
