//! Tauri-managed application state: single-slot active instance.
//!
//! Contract: `tauri-commands.md` — only one active instance per app process
//! (FR-022). Backend owns the atomic switch during `open_instance`; the
//! mutex holds the previous runtime until the new candidate is fully open.
//!
//! The slot is private to this module (spec 013): requests reach the vault through
//! [`AppState::database`], which hands out a [`VaultDb`] that the gate's tracker counts, and the
//! lifecycle commands publish and remove the handle through the methods below.

use std::sync::{Arc, Mutex, MutexGuard};

use haex_crdt::Database;

use crate::error::{HolziError, Result};
use crate::vault_gate::{VaultDb, VaultGate};

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
    active_instance: Mutex<Option<ActiveInstanceHandle>>,
    /// The gate whose tracker counts every `VaultDb` this state hands out.
    gate: VaultGate,
}

impl AppState {
    /// Creates application state with no active instance.
    pub fn new(gate: VaultGate) -> Self {
        Self {
            active_instance: Mutex::new(None),
            gate,
        }
    }

    fn slot(&self, context: &str) -> Result<MutexGuard<'_, Option<ActiveInstanceHandle>>> {
        self.active_instance
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("active_instance mutex poisoned{context}: {e}"),
            })
    }

    /// The active vault's database as a tracked handle, [`HolziError::VaultClosed`] once a close
    /// has started, or [`HolziError::NoActiveInstance`] if no instance is open.
    pub fn database(&self) -> Result<VaultDb> {
        self.gate.with_admission(|admission| {
            let guard = self.slot("")?;
            let handle = guard.as_ref().ok_or(HolziError::NoActiveInstance)?;
            Ok(admission.vault_db(Arc::clone(&handle.database)))
        })
    }

    /// The database of the active instance if it is named `name`.
    pub fn active_database_named(&self, name: &str) -> Result<Option<Arc<Database>>> {
        let guard = self.slot("")?;
        Ok(guard
            .as_ref()
            .filter(|active| active.name == name)
            .map(|active| Arc::clone(&active.database)))
    }

    /// Whether the active instance is named `name`.
    pub fn is_active_named(&self, name: &str) -> Result<bool> {
        Ok(self
            .slot("")?
            .as_ref()
            .is_some_and(|active| active.name == name))
    }

    /// Whether any instance is active.
    pub fn has_active(&self) -> Result<bool> {
        Ok(self.slot("")?.is_some())
    }

    /// Publishes `handle` if no instance is active, running `commit` under the same lock right
    /// before the publish; an error from `commit` leaves the slot empty. Refuses with
    /// [`HolziError::InstanceAlreadyActive`] when an instance is active.
    pub fn install(
        &self,
        handle: ActiveInstanceHandle,
        commit: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let mut guard = self.slot(" during publish")?;
        if guard.is_some() {
            return Err(HolziError::InstanceAlreadyActive);
        }
        commit()?;
        *guard = Some(handle);
        Ok(())
    }

    /// The atomic switch of `open_instance`: under one lock, drops the previous handle, runs
    /// `while_locked`, then publishes `handle`. Stage 4 of spec 013 removes it together with the
    /// switch itself.
    pub fn switch_to(
        &self,
        handle: ActiveInstanceHandle,
        while_locked: impl FnOnce(),
    ) -> Result<()> {
        let mut guard = self.slot("")?;
        drop(guard.take());
        while_locked();
        *guard = Some(handle);
        Ok(())
    }

    /// Removes and returns the active handle so the close can drop it once no clone is left.
    pub fn take(&self) -> Result<Option<ActiveInstanceHandle>> {
        Ok(self.slot("")?.take())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(VaultGate::new())
    }
}
