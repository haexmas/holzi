//! What the password manager tidies once when a vault is opened (spec 034): the clean-up of binary
//! data that nothing uses any more (FR-022) and, with the sync story, the merge of tags that two
//! devices created under the same name at once. It runs as tracked session work, a failure is
//! logged without any value and never blocks opening.

use super::binaries;
use super::clock::unix_millis;
use crate::state::AppState;

/// Starts the tidy-up of the open vault in the background.
pub fn start_after_open(state: &AppState) {
    let Ok(db) = state.database() else {
        return;
    };
    let started = state.gate().spawn(async move {
        let now = unix_millis(std::time::SystemTime::now());
        match db
            .write(move |tx| binaries::prune_binaries(tx, now).map_err(Into::into))
            .await
        {
            Ok(0) => {}
            Ok(deleted) => log::info!("passwords: removed {deleted} unused binary rows"),
            Err(error) => log::warn!("passwords: the clean-up of binary data failed: {error}"),
        }
    });
    if let Err(error) = started {
        log::warn!("passwords: the clean-up could not start: {error}");
    }
}
