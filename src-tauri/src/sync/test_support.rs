//! Shared fixtures for the sync unit tests.

use std::path::Path;
use std::sync::Arc;

use haex_crdt::Database;

use crate::identity::installation_id_path;
use crate::instances::vault_config::vault_config;
use crate::sync::change::PAGE_BUDGET;
use crate::sync::inbound::{InboundError, Inbox, Received};
use crate::sync::outbound::serve_pull_with_budget;
use crate::sync::replica::Replica;

/// A freshly migrated vault under `dir`, without the post-open sync step.
pub fn open_vault(dir: &Path) -> Database {
    open_vault_with_limit(dir, haex_crdt::MAX_CRDT_TRANSACTION_BYTES)
}

/// [`open_vault`] with a transaction size limit of `limit` bytes.
pub fn open_vault_with_limit(dir: &Path, limit: usize) -> Database {
    let mut config = vault_config(
        "sync-test-passphrase",
        &dir.join("vault.db"),
        &installation_id_path(dir),
        true,
    );
    config.max_transaction_bytes = limit;
    Database::open(config).expect("open vault")
}

/// A device: its own directory, vault file and installation.
pub struct Device {
    _dir: tempfile::TempDir,
    pub replica: Replica,
}

impl Device {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let replica = Replica::new(Arc::new(open_vault(dir.path())));
        Self { _dir: dir, replica }
    }

    pub fn db(&self) -> &Database {
        self.replica.db()
    }

    /// Pulls everything `from` has and this device lacks, as one pull.
    pub fn pull_from(&self, from: &Device) -> Vec<Received> {
        self.try_pull_from(from, PAGE_BUDGET).expect("pull")
    }

    /// [`Device::pull_from`] with pages of `budget` bytes.
    pub fn try_pull_from(
        &self,
        from: &Device,
        budget: usize,
    ) -> Result<Vec<Received>, InboundError> {
        let theirs = self.replica.progress()?;
        let mut outbox = serve_pull_with_budget(&from.replica, &theirs, budget)?;
        let mut inbox = Inbox::new();
        let mut received = Vec::new();
        while let Some(page) = outbox.next_page() {
            received.push(inbox.receive(&self.replica, page)?);
        }
        Ok(received)
    }
}
