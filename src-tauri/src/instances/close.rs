//! `close_instance` — drop the active instance's Arc, releasing haex-crdt's
//! fs2 lock. Idempotent.

use tauri::{AppHandle, State};

use crate::chat::session::ChatState;
use crate::error::{HolziError, Result};
use crate::state::AppState;

use super::events::emit_instance_list_changed;

/// Closes the currently-active instance if any. Postcondition per
/// contract: no `Arc<Database>` clone survives in `AppState`. Errors with
/// `CloseFailed` when another subsystem still holds a clone.
#[tauri::command]
pub async fn close_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    chat: State<'_, ChatState>,
) -> Result<()> {
    let _operation = chat.acquire_operation()?;
    let handle_opt = {
        let mut guard = state
            .active_instance
            .lock()
            .map_err(|e| HolziError::CloseFailed {
                reason: format!("active_instance mutex poisoned: {e}"),
            })?;
        guard.take()
    };

    let Some(handle) = handle_opt else {
        // Idempotent close on empty state.
        return Ok(());
    };

    let name = handle.name.clone();

    // Verify that our Arc is the only strong reference — if any subsystem
    // still holds a clone the fs2 lock stays held and a subsequent open
    // of the same file would deadlock.
    match std::sync::Arc::try_unwrap(handle.database) {
        Ok(db) => {
            drop(db);
        }
        Err(arc) => {
            let strong = std::sync::Arc::strong_count(&arc);
            // Put the handle back — closing "half" would leave AppState
            // desynced from the actual DB state.
            let mut guard = state
                .active_instance
                .lock()
                .map_err(|e| HolziError::CloseFailed {
                    reason: format!("active_instance mutex poisoned on rollback: {e}"),
                })?;
            *guard = Some(super::super::state::ActiveInstanceHandle {
                name: name.clone(),
                database: arc,
            });
            return Err(HolziError::CloseFailed {
                reason: format!("{strong} database Arc clones still outstanding"),
            });
        }
    }

    *chat.session.lock().unwrap_or_else(|e| e.into_inner()) = None;
    emit_instance_list_changed(&app, "closed", Some(name));
    Ok(())
}
