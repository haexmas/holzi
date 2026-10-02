//! The Tauri commands of the password manager (spec 034, `contracts/tauri-commands.md`). Each calls
//! only the [`PasswordsService`], with the caller that its entrance stands for: the commands of the
//! window are `Caller::User`, `passwords_agent_search` is `Caller::BuiltinAgent`. No command takes a
//! caller as an argument.

pub mod items;
pub mod passkeys;
pub mod read;

use serde::Deserialize;
use tauri::State;
use ts_rs::TS;

use super::service::PasswordsService;
use crate::error::Result;
use crate::state::AppState;
use crate::state_utils::active_database;

/// The service over the active vault; `VaultClosed` or `NoActiveInstance` when there is none.
pub(crate) fn service(state: &State<'_, AppState>) -> Result<PasswordsService> {
    Ok(PasswordsService::new(active_database(state)?))
}

/// What a bulk action works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum TargetKind {
    Item,
    Group,
}

/// An entry or a folder, named by its id.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub kind: TargetKind,
    pub id: String,
}
