//! Typed writes for the `known_devices` table.
//!
//! Bootstrap-inserts live in `identity::bootstrap` and do NOT belong here —
//! HLC is not yet initialised inside the bootstrap transaction, so those rows
//! start sync-invisible on purpose. This module owns the runtime updates
//! that surface the bootstrap row to sync scanners.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection};
use uuid::Uuid;

/// Updates the human-readable alias on this replica's `known_devices` row.
/// Injects `haex_hlc_no_sync = current_hlc()` — the write becomes visible
/// to the sync scanner after this call (Etappe-0 finding #2).
pub fn update_alias(
    conn: &Connection,
    installation_uuid: Uuid,
    alias: &str,
) -> haex_crdt::rusqlite::Result<usize> {
    let sql = format!(
        "UPDATE known_devices \
         SET alias = ?1, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
         WHERE installation_uuid = ?2"
    );
    conn.execute(&sql, params![alias, installation_uuid.to_string()])
}

/// Reads this installation's `vault_device_uuid` from `known_devices`. Never
/// mutates; safe to call in read-only contexts.
pub fn get_vault_device_uuid(
    conn: &Connection,
    installation_uuid: Uuid,
) -> haex_crdt::rusqlite::Result<Option<Uuid>> {
    use haex_crdt::rusqlite::OptionalExtension;
    let raw: Option<String> = conn
        .query_row(
            "SELECT vault_device_uuid FROM known_devices \
             WHERE installation_uuid = ?1",
            params![installation_uuid.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    Ok(raw.and_then(|s| Uuid::parse_str(&s).ok()))
}
