//! Tauri-managed application state: single-slot active instance.
//!
//! Contract: `tauri-commands.md` — only one active instance per app process
//! (FR-022). Backend owns the atomic switch during `open_instance`; the
//! mutex holds the previous runtime until the new candidate is fully open.

use std::sync::{Arc, Mutex};

use haex_crdt::Database;

/// Handle to the currently-active instance's runtime. Dropping the last
/// `Arc<Database>` clone releases haex-crdt's fs2 lock as a consequence
/// of the last-Arc drop.
pub struct ActiveInstanceHandle {
    /// Instance name (matches the file basename without `.db`).
    pub name: String,
    /// haex-crdt handle. All backend subsystems that need DB access
    /// clone the `Arc`; `close_instance` cannot succeed until every clone
    /// has been dropped (per contract postcondition on `CloseFailed`).
    pub database: Arc<Database>,
}

/// The Tauri-managed application state.
pub struct AppState {
    /// `None` on boot; populated by the first `create_instance`/
    /// `open_instance`; cleared by `close_instance`. The mutex is held
    /// across the atomic switch in `open_instance` to keep validate →
    /// stop-old → publish-new indivisible.
    pub active_instance: Mutex<Option<ActiveInstanceHandle>>,
}

impl AppState {
    /// Creates application state with no active instance.
    pub fn new() -> Self {
        Self {
            active_instance: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
