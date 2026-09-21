//! The vault gate: the one owner of "may requests be answered, and what is still running"
//! (spec 013, data-model.md).
//!
//! One app process serves at most one vault session, and closing it ends the process. The gate
//! carries what that needs: the phase of the session, one cancellation token that fires when a
//! close starts, and one tracker that counts tracked tasks, blocking work and every live
//! [`VaultDb`]. The drain ladder ([`VaultGate::drain`]) waits for the tracker to empty, and the
//! invoke-handler wrapper ([`VaultGate::wrap`]) makes every command default-deny once a close
//! has started.

mod db;
mod drain;
mod invoke;

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};

use haex_crdt::Database;
use tokio::runtime::Handle;
use tokio::task::{AbortHandle, JoinHandle};
use tokio_util::sync::CancellationToken;
use tokio_util::task::task_tracker::TaskTrackerToken;
use tokio_util::task::TaskTracker;

use crate::error::{HolziError, Result};

pub use db::VaultDb;
pub use drain::{hard_end_after, DrainOutcome, COOPERATIVE_WINDOW, HARD_END_GRACE, TOTAL_LIMIT};
pub use invoke::APP_SCOPED_COMMANDS;

/// Where the app process is in its one-session life. It only moves forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultPhase {
    /// No vault is open yet; a failed unlock leaves the gate here.
    Idle,
    /// A vault session is running.
    Active,
    /// A close was requested; only the app-scoped allow-list is answered.
    Closing,
}

/// What ends the process after a close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosePolicy {
    /// Start the app again, which shows the unlock screen.
    Relaunch,
    /// Just end the process.
    Exit,
}

/// The policy for this build. Debug builds exit until the relaunch under the dev runner is
/// verified (research R2, task T048); release builds relaunch.
pub fn close_policy() -> ClosePolicy {
    if cfg!(debug_assertions) {
        ClosePolicy::Exit
    } else {
        ClosePolicy::Relaunch
    }
}

/// The gate. Cheap to clone; every clone is the same gate.
#[derive(Clone)]
pub struct VaultGate {
    inner: Arc<Inner>,
}

struct Inner {
    /// Held only for a phase read or transition, never across an await.
    phase: Mutex<VaultPhase>,
    /// Fires once, on the first transition into `Closing`.
    cancel: CancellationToken,
    /// Counts tracked tasks, blocking work and every live `VaultDb`.
    tasks: TaskTracker,
    /// Abort handles of tracked async tasks, used by the second rung of the drain.
    aborts: Mutex<Vec<AbortHandle>>,
    /// Runtime that runs tracked work. `None` means Tauri's own async runtime.
    runtime: Option<Handle>,
}

impl VaultGate {
    /// A gate in `Idle` whose tracked work runs on Tauri's async runtime.
    pub fn new() -> Self {
        Self::build(None)
    }

    /// A gate whose tracked work runs on `runtime`, for tests that drive their own runtime.
    pub fn with_runtime(runtime: Handle) -> Self {
        Self::build(Some(runtime))
    }

    fn build(runtime: Option<Handle>) -> Self {
        Self {
            inner: Arc::new(Inner {
                phase: Mutex::new(VaultPhase::Idle),
                cancel: CancellationToken::new(),
                tasks: TaskTracker::new(),
                aborts: Mutex::new(Vec::new()),
                runtime,
            }),
        }
    }

    fn phase_lock(&self) -> MutexGuard<'_, VaultPhase> {
        // The guarded value is a plain `Copy` enum, so a poisoned lock holds no broken state.
        self.inner.phase.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The current phase.
    pub fn phase(&self) -> VaultPhase {
        *self.phase_lock()
    }

    /// Whether a close was requested.
    pub fn is_closing(&self) -> bool {
        self.phase() == VaultPhase::Closing
    }

    /// Whether an open or create may start now, without changing the phase.
    pub fn ensure_can_open(&self) -> Result<()> {
        match self.phase() {
            VaultPhase::Idle => Ok(()),
            VaultPhase::Active => Err(HolziError::VaultAlreadyActive),
            VaultPhase::Closing => Err(HolziError::VaultClosed),
        }
    }

    /// Moves `Idle` to `Active`. Called only after the database has opened, so a failed unlock
    /// leaves the gate in `Idle`. Exactly one of several racing callers succeeds.
    pub fn begin_session(&self) -> Result<()> {
        let mut phase = self.phase_lock();
        match *phase {
            VaultPhase::Idle => {
                *phase = VaultPhase::Active;
                Ok(())
            }
            VaultPhase::Active => Err(HolziError::VaultAlreadyActive),
            VaultPhase::Closing => Err(HolziError::VaultClosed),
        }
    }

    /// Starts the close: moves to `Closing`, fires the cancellation token and closes the tracker.
    /// Returns whether this call was the first; a repeated request changes nothing (FR-002).
    pub fn request_close(&self) -> bool {
        let mut phase = self.phase_lock();
        if *phase == VaultPhase::Closing {
            return false;
        }
        *phase = VaultPhase::Closing;
        drop(phase);
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        true
    }

    /// The token that fires when the close starts.
    pub fn token(&self) -> CancellationToken {
        self.inner.cancel.clone()
    }

    /// A token that keeps the tracker non-empty until it is dropped.
    pub fn tracker_token(&self) -> TaskTrackerToken {
        self.inner.tasks.token()
    }

    /// Wraps a database handle so the tracker counts it while any clone is alive.
    pub fn vault_db(&self, db: Arc<Database>) -> VaultDb {
        VaultDb::new(db, self.tracker_token())
    }

    /// Whether no tracked work and no `VaultDb` is alive.
    pub fn is_idle(&self) -> bool {
        self.inner.tasks.is_empty()
    }

    fn runtime(&self) -> Handle {
        match &self.inner.runtime {
            Some(handle) => handle.clone(),
            None => tauri::async_runtime::handle().inner().clone(),
        }
    }

    /// Runs `fut` as tracked, abortable session work.
    pub fn spawn<F>(&self, fut: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let handle = self.inner.tasks.spawn_on(fut, &self.runtime());
        let mut aborts = self.inner.aborts.lock().unwrap_or_else(|e| e.into_inner());
        aborts.retain(|abort| !abort.is_finished());
        aborts.push(handle.abort_handle());
        drop(aborts);
        handle
    }

    /// Runs `work` on the blocking pool, tracked. The drain waits for the closure itself,
    /// because aborting the outer task cannot stop a running thread.
    pub fn spawn_blocking<F, R>(&self, work: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.inner.tasks.spawn_blocking_on(work, &self.runtime())
    }

    /// Races `fut` against the close: the future is dropped and `VaultClosed` returned once the
    /// close has started, even if its result was ready at the same moment (FR-001).
    pub async fn run<F: Future>(&self, fut: F) -> Result<F::Output> {
        tokio::select! {
            biased;
            _ = self.inner.cancel.cancelled() => Err(HolziError::VaultClosed),
            out = fut => Ok(out),
        }
    }
}

impl Default for VaultGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod gate_tests;
