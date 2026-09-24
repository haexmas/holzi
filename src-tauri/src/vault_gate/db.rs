//! `VaultDb`: the database handle that carries a tracker token (data-model.md).

use std::ops::Deref;
use std::sync::Arc;

use haex_crdt::Database;
use tokio_util::task::task_tracker::TaskTrackerToken;

/// A handle to the vault database that keeps the gate's tracker non-empty while any clone is
/// alive. The tracker is empty exactly when no request still uses the database, so the drain can
/// then take the database out of the app state and drop it, which closes the connection.
#[derive(Clone)]
pub struct VaultDb {
    db: Arc<Database>,
    _token: TaskTrackerToken,
}

impl VaultDb {
    pub(super) fn new(db: Arc<Database>, token: TaskTrackerToken) -> Self {
        Self { db, _token: token }
    }
}

impl Deref for VaultDb {
    type Target = Database;

    fn deref(&self) -> &Database {
        &self.db
    }
}

#[cfg(test)]
#[path = "db_tests.rs"]
mod db_tests;
