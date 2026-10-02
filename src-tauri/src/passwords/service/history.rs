//! History methods of the service (spec 034, US4, FR-017): the states of an entry, one state
//! without its secrets, a secret of a state on request, and restoring a state. For the user alone
//! (rule Z11): a caller from outside never reads old passwords.

use super::{require_user, PasswordsService};
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::passwords::model::{
    HistorySecret, RestoreOutcome, RevealedSecret, SnapshotHeader, SnapshotView,
};
use crate::passwords::snapshots;

impl PasswordsService {
    pub async fn history_list(
        &self,
        caller: &Caller,
        item_id: String,
    ) -> Result<Vec<SnapshotHeader>> {
        require_user(caller)?;
        self.db()
            .read(move |q| snapshots::list(q, &item_id).map_err(Into::into))
            .await
    }

    pub async fn history_get(&self, caller: &Caller, snapshot_id: String) -> Result<SnapshotView> {
        require_user(caller)?;
        self.db()
            .read(move |q| snapshots::get(q, &snapshot_id).map_err(Into::into))
            .await
    }

    pub async fn history_reveal(
        &self,
        caller: &Caller,
        snapshot_id: String,
        field: HistorySecret,
    ) -> Result<RevealedSecret> {
        require_user(caller)?;
        self.db()
            .read(move |q| snapshots::reveal(q, &snapshot_id, &field).map_err(Into::into))
            .await
    }

    /// Makes a state the current one (the token `expected_updated_at` must still be current).
    pub async fn history_restore(
        &self,
        caller: &Caller,
        item_id: String,
        snapshot_id: String,
        expected_updated_at: String,
    ) -> Result<RestoreOutcome> {
        require_user(caller)?;
        self.db()
            .write(move |tx| {
                snapshots::restore(tx, &item_id, &snapshot_id, &expected_updated_at)
                    .map_err(Into::into)
            })
            .await
    }
}
