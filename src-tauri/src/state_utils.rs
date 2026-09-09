//! Helpers for pulling the active `Database` out of `AppState` behind
//! the standard error mapping. Every command that reads or writes the
//! active vault uses this.

use std::sync::Arc;

use haex_crdt::Database;
use tauri::State;

use crate::error::{HolziError, Result};
use crate::state::AppState;

/// Returns the currently-active `Database` handle, or
/// [`HolziError::NoActiveInstance`] if no instance is open.
pub fn active_database(state: &State<'_, AppState>) -> Result<Arc<Database>> {
    let guard = state
        .active_instance
        .lock()
        .map_err(|e| HolziError::CrdtInit {
            reason: format!("active_instance mutex poisoned: {e}"),
        })?;
    let handle = guard.as_ref().ok_or(HolziError::NoActiveInstance)?;
    Ok(Arc::clone(&handle.database))
}
