//! Tauri-managed application state: single-slot active instance.
//!
//! Contract: `tauri-commands.md` — only one active instance per app process (FR-022, now enforced
//! by refusal rather than a switch: spec 013 FR-010, ADR 0003). `install` is the one place that
//! publishes a handle, under the same lock as the gate's own `Idle` → `Active` transition, so the
//! two never disagree.
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
    /// haex-crdt handle. Requests get it through [`AppState::database`] as a tracked
    /// [`VaultDb`]; the close drops this reference once every tracked clone is gone, or at the
    /// drain limit, and the process ends either way.
    pub database: Arc<Database>,
}

/// The Tauri-managed application state.
pub struct AppState {
    /// `None` on boot; published once by `create_instance`/`open_instance` (a second attempt is
    /// refused by the gate before either reaches here); cleared by `close_instance`.
    active_instance: Mutex<Option<ActiveInstanceHandle>>,
    /// The gate whose tracker counts every `VaultDb` this state hands out, and whose phase
    /// `install` advances in the same breath as publishing.
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

    /// The gate whose tracker counts this state's database handles, for commands that race their
    /// long work against the close, and whose `ensure_can_open`/`begin_session` this state's own
    /// `install` defers to instead of keeping a second, separate notion of "is one active".
    pub fn gate(&self) -> &VaultGate {
        &self.gate
    }

    /// The name of the active instance, if any.
    pub fn active_name(&self) -> Result<Option<String>> {
        Ok(self.slot("")?.as_ref().map(|active| active.name.clone()))
    }

    /// Publishes `handle`, running `commit` under the same lock right before the publish (an
    /// error from `commit` leaves the slot empty), then moving the gate `Idle` → `Active` in the
    /// same breath. [`HolziError::VaultAlreadyActive`] or [`HolziError::VaultClosed`] if the gate
    /// is not `Idle` — from a second `install` racing this one (exactly one wins) or a close
    /// already under way.
    pub fn install(
        &self,
        handle: ActiveInstanceHandle,
        commit: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let mut guard = self.slot(" during publish")?;
        commit()?;
        self.gate.begin_session()?;
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
