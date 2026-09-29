//! The sync service of an open vault (spec 024, FR-031).
//!
//! One service runs per vault session, as tracked work of the vault gate: it
//! ends when the close starts, within the drain limits of spec 013. It wakes
//! on every committed `VaultDb` write (`VaultGate::sync_notify`); wake-ups that
//! arrive close together collapse into one round. Connections, presence and
//! the exchange of progress arrive with user story 1; until then a round only
//! counts.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::vault_gate::VaultGate;

/// Handle to the running service.
#[derive(Clone, Default)]
pub struct SyncService {
    rounds: Arc<AtomicU64>,
}

impl SyncService {
    /// Starts the service as tracked session work of `gate`.
    pub fn start(gate: &VaultGate) -> Result<Self> {
        let service = Self::default();
        let rounds = Arc::clone(&service.rounds);
        let notify = gate.sync_notify();
        let token = gate.token();
        gate.spawn(run(notify, token, rounds))?;
        Ok(service)
    }

    /// How many rounds ran since the start; each follows one or more commits.
    pub fn rounds(&self) -> u64 {
        self.rounds.load(Ordering::Relaxed)
    }
}

async fn run(notify: Arc<Notify>, token: CancellationToken, rounds: Arc<AtomicU64>) {
    loop {
        tokio::select! {
            biased;
            _ = token.cancelled() => break,
            _ = notify.notified() => {
                rounds.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
