//! Freeing the files of removed extensions (spec 017, T091, data-model.md `extension_blobs`), once
//! per vault open as for attachments in spec 034.
//!
//! A BLOB no bundle file refers to is marked `orphaned_at` when that is first seen, a BLOB a file
//! refers to again loses the mark, and one unreferenced for [`BLOB_ORPHAN_GRACE_DAYS`] is deleted.
//! The grace period covers a BLOB that arrived before the bundle rows that use it (each BLOB is a
//! write of its own) and a reinstall soon after a removal.

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;

use crate::extensions::BLOB_ORPHAN_GRACE_DAYS;
use crate::state::AppState;

const DAY_MS: i64 = 86_400_000;

/// Marks, unmarks and deletes as described above; returns how many BLOBs were deleted.
pub fn prune_orphans(tx: &mut CrdtTransaction<'_>, now_ms: i64) -> haex_crdt::Result<usize> {
    tx.execute(
        "UPDATE extension_blobs SET orphaned_at = NULL WHERE orphaned_at IS NOT NULL \
         AND hash IN (SELECT sha256 FROM extension_bundle_files)",
        &[],
    )?;
    tx.execute(
        "UPDATE extension_blobs SET orphaned_at = ?1 WHERE orphaned_at IS NULL \
         AND hash NOT IN (SELECT sha256 FROM extension_bundle_files)",
        params![now_ms],
    )?;
    let grace = i64::try_from(BLOB_ORPHAN_GRACE_DAYS).unwrap_or(i64::MAX / DAY_MS) * DAY_MS;
    tx.execute(
        "DELETE FROM extension_blobs WHERE orphaned_at IS NOT NULL AND orphaned_at <= ?1 \
         AND hash NOT IN (SELECT sha256 FROM extension_bundle_files)",
        params![now_ms.saturating_sub(grace)],
    )
}

/// Runs [`prune_orphans`] for the vault just opened, as tracked session work.
pub fn start_after_open(state: &AppState) {
    let Ok(db) = state.database() else {
        return;
    };
    let started = state.gate().spawn(async move {
        let now = crate::passwords::clock::unix_millis(std::time::SystemTime::now());
        match db.write(move |tx| prune_orphans(tx, now)).await {
            Ok(0) => {}
            Ok(deleted) => log::info!("extensions: freed {deleted} unused files"),
            Err(error) => log::warn!("extensions: freeing unused files failed: {error}"),
        }
    });
    if let Err(error) = started {
        log::warn!("extensions: freeing unused files could not start: {error}");
    }
}

#[cfg(test)]
#[path = "blobs_tests.rs"]
mod tests;
