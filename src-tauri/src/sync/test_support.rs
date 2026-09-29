//! Shared fixtures for the sync unit tests.

use std::path::Path;

use haex_crdt::Database;

use crate::identity::installation_id_path;
use crate::instances::vault_config::vault_config;

/// A freshly migrated vault under `dir`, without the post-open sync step.
pub fn open_vault(dir: &Path) -> Database {
    Database::open(vault_config(
        "sync-test-passphrase",
        &dir.join("vault.db"),
        &installation_id_path(dir),
        true,
    ))
    .expect("open vault")
}
