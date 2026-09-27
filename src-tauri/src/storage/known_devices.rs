//! Typed writes for the `known_devices` table.
//!
//! Bootstrap-inserts live in `identity::bootstrap` and do NOT belong here —
//! HLC is not yet initialised inside the bootstrap transaction, so those rows
//! start sync-invisible on purpose. This module owns the runtime updates
//! that surface the bootstrap row to sync scanners, and the device list of
//! the settings (spec 023-settings-app, FR-022).

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection};
use uuid::Uuid;

use crate::identity::VAULT_SCOPE_UUID;

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

/// One device of the vault as `known_devices` records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownDevice {
    pub installation_uuid: Uuid,
    pub vault_device_uuid: Uuid,
    /// `None` until the device finished onboarding.
    pub alias: Option<String>,
}

/// Every device of the vault except the internal vault-scope row that
/// `identity::bootstrap` inserts for vault-wide preferences. Rows whose UUIDs
/// do not parse are skipped; the order is the caller's.
pub fn list_devices(conn: &Connection) -> haex_crdt::rusqlite::Result<Vec<KnownDevice>> {
    let mut stmt = conn.prepare(
        "SELECT installation_uuid, vault_device_uuid, alias FROM known_devices \
         WHERE installation_uuid != ?1",
    )?;
    let rows = stmt.query_map(params![VAULT_SCOPE_UUID.to_string()], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut devices = Vec::new();
    for row in rows {
        let (installation, vault_device, alias) = row?;
        if let (Ok(installation_uuid), Ok(vault_device_uuid)) = (
            Uuid::parse_str(&installation),
            Uuid::parse_str(&vault_device),
        ) {
            devices.push(KnownDevice {
                installation_uuid,
                vault_device_uuid,
                alias,
            });
        }
    }
    Ok(devices)
}
