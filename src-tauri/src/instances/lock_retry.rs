//! Retrying `open_instance` while another process holds the vault's advisory file lock (spec 013
//! FR-018 to FR-020, US4).
//!
//! `haex-crdt`'s own error boundary collapses every `DatabaseError` variant — including its
//! already-distinct `VaultAlreadyOpenElsewhere { path, reason }` — into an opaque
//! `Error::Message(String)` before it reaches this crate (`From<DatabaseError> for Error` in
//! `haex-crdt`'s `src/error.rs`), so `open.rs` classifies the message text the same way
//! `is_wrong_passphrase` already does for a bad passphrase, rather than pattern-matching a variant
//! that never survives the crossing.

use std::future::Future;
use std::time::Duration;

use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::error::{HolziError, Result};

/// The window and cadence `open_instance` retries with (T069). ponytail: fixed 100 ms poll inside
/// a 3 s window; ceiling "adds up to one poll interval of latency" past the true 3 s deadline;
/// upgrade path "none needed" — chosen for local disk-lock contention between two processes on the
/// same host, not network latency, so no adaptive backoff is warranted.
pub const OPEN_RETRY_WINDOW: Duration = Duration::from_secs(3);
pub const OPEN_RETRY_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Retries `attempt` while it fails with `VaultAlreadyOpenElsewhere`, for up to `window`, waiting
/// `poll_interval` between attempts. Returns the first `Ok`, the first error other than
/// `VaultAlreadyOpenElsewhere` (no retry), `VaultClosed` if `token` fires while waiting, or the
/// last `VaultAlreadyOpenElsewhere` once `window` has elapsed.
pub async fn retry_while_locked<T, F, Fut>(
    token: &CancellationToken,
    window: Duration,
    poll_interval: Duration,
    mut attempt: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let deadline = Instant::now() + window;
    loop {
        if token.is_cancelled() {
            return Err(HolziError::VaultClosed);
        }
        match attempt().await {
            Err(HolziError::VaultAlreadyOpenElsewhere) if Instant::now() < deadline => {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => return Err(HolziError::VaultClosed),
                    _ = tokio::time::sleep(poll_interval) => {}
                }
            }
            other => return other,
        }
    }
}

#[cfg(test)]
#[path = "lock_retry_tests.rs"]
mod lock_retry_tests;
