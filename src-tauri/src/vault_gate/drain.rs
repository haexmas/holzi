//! The drain ladder and the forced end (research R5, data-model.md `DrainOutcome`).
//!
//! After a close starts, the drain gives tracked work a cooperative window to stop by itself,
//! then aborts the registered async tasks and ends every registered child process, then reports.
//! Children go at the abort rung, not only at the end, because a thread that waits for its child
//! cannot be aborted and would otherwise hold the drain until the limit. Blocking threads cannot
//! be aborted, so the ladder says so ([`DrainOutcome::Stuck`]) instead of pretending; ending the
//! process is the final answer, never a retry for the user.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::VaultGate;

// ponytail: fixed deadlines, 1 s cooperative and 3 s in total. Ceiling: they do not adapt to a
// slow disk or a busy machine. Upgrade path: make them configurable in one place.
/// How long tracked work gets to stop by itself after the close starts.
pub const COOPERATIVE_WINDOW: Duration = Duration::from_secs(1);
/// The most the drain waits in total, from the close request.
pub const TOTAL_LIMIT: Duration = Duration::from_secs(3);

// ponytail: fixed 0.5 s between asking the process to end and ending it forcibly. Ceiling: a stuck
// exit path costs half a second more. Upgrade path: none needed.
/// How long the normal process end gets before a plain thread forces it.
pub const HARD_END_GRACE: Duration = Duration::from_millis(500);

/// How the drain ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainOutcome {
    /// The tracker emptied within the cooperative window.
    Drained,
    /// The tracker emptied after registered tasks were aborted.
    DrainedAfterAbort,
    /// Something un-abortable was still running at the limit. The process ends anyway.
    Stuck,
}

impl VaultGate {
    /// Runs the ladder with the production deadlines.
    pub async fn drain(&self) -> DrainOutcome {
        self.drain_with(COOPERATIVE_WINDOW, TOTAL_LIMIT).await
    }

    /// Runs the ladder: fire the token (idempotent), wait up to `cooperative`, abort registered
    /// tasks and end the registered children, wait until `total` has passed since the start.
    /// Children are ended on every path, so none outlives the session and a late registration is
    /// killed at once.
    pub async fn drain_with(&self, cooperative: Duration, total: Duration) -> DrainOutcome {
        self.request_close();
        if tokio::time::timeout(cooperative, self.inner.tasks.wait())
            .await
            .is_ok()
        {
            self.inner.children.kill_all();
            return DrainOutcome::Drained;
        }
        for abort in self
            .inner
            .aborts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
        {
            abort.abort();
        }
        self.inner.children.kill_all();
        let remaining = total.saturating_sub(cooperative);
        if tokio::time::timeout(remaining, self.inner.tasks.wait())
            .await
            .is_ok()
        {
            DrainOutcome::DrainedAfterAbort
        } else {
            DrainOutcome::Stuck
        }
    }
}

/// Runs `action` on a plain thread after `grace`, for the forced end of the process. See
/// [`on_plain_thread`].
pub fn hard_end_after<F>(grace: Duration, action: F)
where
    F: FnOnce() + Send + 'static,
{
    on_plain_thread("vault-hard-end", grace, action);
}

/// Runs `action` on a plain thread named `name` after `delay`. The thread depends on neither
/// tokio nor the window event loop, so a hung runtime or exit path cannot keep it from running. If
/// no thread can be started the action runs at once: an action that never runs is worse than an
/// early one.
pub fn on_plain_thread<F>(name: &str, delay: Duration, action: F)
where
    F: FnOnce() + Send + 'static,
{
    let slot = Arc::new(Mutex::new(Some(action)));
    let thread_slot = Arc::clone(&slot);
    let spawned = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            std::thread::sleep(delay);
            run_once(&thread_slot);
        });
    if let Err(error) = spawned {
        log::error!("could not start the {name} thread, running its action now: {error}");
        run_once(&slot);
    }
}

fn run_once<F: FnOnce()>(slot: &Mutex<Option<F>>) {
    let action = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(action) = action {
        action();
    }
}

#[cfg(test)]
#[path = "drain_tests.rs"]
mod drain_tests;
