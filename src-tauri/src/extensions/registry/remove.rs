//! Removing an extension from the vault (spec 017, FR-008, research R11). The removing device
//! keeps the `extensions` row as a tombstone with `state = removed`, `purge_data` and `purge_hlc`
//! (the HLC of this very write), and deletes the other registry rows the normal way, so the
//! deletions sync. The clear-up of tables and device-local state runs on every device, this one
//! included, once it sees the new `purge_hlc` ([`super::purge`], [`super::lifecycle`]).
//!
//! BLOBs no file refers to any more get `orphaned_at`; they are collected after seven days
//! (data-model.md), so a reinstall in between needs no new transfer.

use haex_crdt::rusqlite::params;
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::vault_gate::VaultDb;

/// Removes `extension_id`; with `delete_data` every device also drops its tables. Removing an
/// extension that is not installed is refused.
pub fn remove(db: &VaultDb, extension_id: Uuid, delete_data: bool, now_ms: i64) -> Result<()> {
    db.write_blocking(move |tx| {
        let ext = extension_id.to_string();
        let installed = tx
            .query_row(
                "SELECT COUNT(*) FROM extensions WHERE id = ?1 AND state = 'installed'",
                params![ext],
                |r| r.get::<_, i64>(0),
            )?
            .unwrap_or(0)
            > 0;
        if !installed {
            return Err(HolziError::ExtensionNotFound.into());
        }
        tx.execute(
            "UPDATE extensions SET state = 'removed', purge_data = ?2, updated_at = ?3 \
             WHERE id = ?1",
            params![ext, delete_data, now_ms],
        )?;
        // The transformer stamps the row with this transaction's HLC; that is the removal's.
        tx.execute(
            "UPDATE extensions SET purge_hlc = haex_hlc_no_sync WHERE id = ?1",
            params![ext],
        )?;
        for sql in [
            "DELETE FROM extension_bundle_files WHERE bundle_id IN \
             (SELECT id FROM extension_bundles WHERE extension_id = ?1)",
            "DELETE FROM extension_bundles WHERE extension_id = ?1",
            "DELETE FROM extension_migrations WHERE extension_id = ?1",
            "DELETE FROM extension_permissions WHERE extension_id = ?1",
            "DELETE FROM extension_limits WHERE extension_id = ?1",
            "DELETE FROM extension_device_status WHERE extension_id = ?1",
        ] {
            tx.execute(sql, params![ext])?;
        }
        tx.execute(
            "UPDATE extension_blobs SET orphaned_at = ?1 WHERE orphaned_at IS NULL \
             AND hash NOT IN (SELECT sha256 FROM extension_bundle_files)",
            params![now_ms],
        )?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "remove_tests.rs"]
mod tests;
