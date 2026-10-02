//! Binary data of attachments and icons (spec 034, US5, FR-019..FR-022, research R4). This file
//! holds the grace period bookkeeping that deleting needs first: when the last link to a binary
//! goes, the binary is marked (`orphaned_at`) and is removed only after seven days, so a link of
//! another device that has not arrived yet does not run into nothing. The attachment functions and
//! the clean-up itself follow with the attachments story.

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;

use super::clock;
use crate::error::Result;

/// Starts the grace period of a binary that no attachment and no history state links any more
/// (`orphaned_at = now`, only if it is not marked yet). A binary that is still linked is left
/// alone.
pub fn mark_if_unreferenced(tx: &mut CrdtTransaction<'_>, hash: &str) -> Result<()> {
    let linked = tx
        .query_row(
            "SELECT (SELECT COUNT(*) FROM haex_passwords_item_binaries WHERE binary_hash = ?1) \
                  + (SELECT COUNT(*) FROM haex_passwords_snapshot_binaries WHERE binary_hash = ?1)",
            params![hash],
            |r| r.get::<_, i64>(0),
        )?
        .unwrap_or(0);
    if linked == 0 {
        tx.execute(
            "UPDATE haex_passwords_binaries SET orphaned_at = ?1 \
             WHERE hash = ?2 AND orphaned_at IS NULL",
            params![clock::now(), hash],
        )?;
    }
    Ok(())
}

/// Ends the grace period of a binary that got a link (again).
pub fn clear_orphan_mark(tx: &mut CrdtTransaction<'_>, hash: &str) -> Result<()> {
    tx.execute(
        "UPDATE haex_passwords_binaries SET orphaned_at = NULL \
         WHERE hash = ?1 AND orphaned_at IS NOT NULL",
        params![hash],
    )?;
    Ok(())
}
