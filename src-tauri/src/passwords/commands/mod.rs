//! The Tauri commands of the password manager (spec 034, `contracts/tauri-commands.md`). Each calls
//! only the [`PasswordsService`], with the caller that its entrance stands for: the commands of the
//! window are `Caller::User`, `passwords_agent_search` is `Caller::BuiltinAgent`. No command takes a
//! caller as an argument.

pub mod agent;
pub mod items;
pub mod organize;
pub mod passkeys;
pub mod presets;
pub mod read;
pub mod usage;

use tauri::State;

use super::service::PasswordsService;
use crate::error::Result;
use crate::state::AppState;
use crate::state_utils::active_database;

/// The service over the active vault; `VaultClosed` or `NoActiveInstance` when there is none.
pub(crate) fn service(state: &State<'_, AppState>) -> Result<PasswordsService> {
    Ok(PasswordsService::with_usage(
        active_database(state)?,
        state.usage(),
    ))
}

pub use crate::passwords::model::{Target, TargetKind};
