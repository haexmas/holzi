//! Typed writes and reads for the `preferences` table.
//!
//! Namespaced key/value store shared across devices via CRDT sync. See
//! [data-model.md](../../../specs/002-onboarding-model-prefs/data-model.md).
//!
//! Rows are keyed by `(vault_device_uuid, key)`; `vault_device_uuid`
//! is either a real device UUID or the vault-scope sentinel
//! [`crate::identity::VAULT_SCOPE_UUID`]. The hard FK on
//! `known_devices(vault_device_uuid) ON DELETE CASCADE` guarantees that
//! retiring a device automatically drops its per-device preferences.
//!
//! Every write goes through the helpers here so
//! `haex_hlc_no_sync = current_hlc()` is set — the Etappe-0 convention
//! from `storage::` module docs.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result};
use uuid::Uuid;

use crate::identity::VAULT_SCOPE_UUID;

/// Scope of a preference row: vault-wide (sentinel-backed) or a
/// specific device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefScope {
    /// Row keyed under [`VAULT_SCOPE_UUID`] and visible on every device.
    Vault,
    /// Row keyed under a real `known_devices.vault_device_uuid`.
    Device(Uuid),
}

impl PrefScope {
    /// Resolves this scope to its stored `vault_device_uuid` value.
    pub fn to_uuid(self) -> Uuid {
        match self {
            PrefScope::Vault => VAULT_SCOPE_UUID,
            PrefScope::Device(uuid) => uuid,
        }
    }
}

/// One preference entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefRow {
    pub scope: PrefScope,
    pub key: String,
    pub value: Option<String>,
}

/// Error surfaced for structural violations the SQL layer would report
/// less usefully. Namespace enforcement lives here because the CHECK
/// constraint approach would collide with CRDT sync-apply.
#[derive(Debug, thiserror::Error)]
pub enum PrefError {
    #[error("preference key must not be empty")]
    EmptyKey,
    #[error("preference key must be dotted-namespaced (contain '.'), got '{0}'")]
    UnnamespacedKey(String),
    #[error(
        "Device-scoped preferences must use a real device UUID; the nil UUID is reserved for vault scope"
    )]
    NilDeviceUuid,
}

/// Validates the key format enforced across the wrapper API.
pub fn validate_key(key: &str) -> std::result::Result<(), PrefError> {
    if key.is_empty() {
        return Err(PrefError::EmptyKey);
    }
    if !key.contains('.') {
        return Err(PrefError::UnnamespacedKey(key.to_string()));
    }
    Ok(())
}

/// Validates that a scope's underlying UUID is well-formed.
pub fn validate_scope(scope: PrefScope) -> std::result::Result<(), PrefError> {
    if let PrefScope::Device(uuid) = scope {
        if uuid == VAULT_SCOPE_UUID {
            return Err(PrefError::NilDeviceUuid);
        }
    }
    Ok(())
}

/// Reads a single preference value. Returns `None` when the row is
/// absent OR when the stored value is SQL NULL — the two are folded
/// together by design (data-model.md).
pub fn get(conn: &Connection, scope: PrefScope, key: &str) -> Result<Option<String>> {
    let raw: Option<Option<String>> = conn
        .query_row(
            "SELECT value FROM preferences \
             WHERE vault_device_uuid = ?1 AND key = ?2",
            params![scope.to_uuid().to_string(), key],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?;
    Ok(raw.flatten())
}

/// Inserts or updates a preference row. Always sets
/// `haex_hlc_no_sync = current_hlc()`.
///
/// Uses a check-then-write path rather than `INSERT ... ON CONFLICT DO
/// UPDATE`: the ON-CONFLICT form makes SQLite fire both the row's
/// AFTER-INSERT and AFTER-UPDATE triggers in the same statement, and
/// their `INSERT OR REPLACE INTO haex_crdt_dirty_tables_no_sync`
/// bodies collide on the primary key when both fire back-to-back
/// against a table whose only tracked column is `value`. Splitting
/// avoids that conflict and keeps the sync-visibility contract intact
/// (the winning path — INSERT or UPDATE — fires exactly one CRDT
/// trigger, so the row is marked dirty exactly once).
pub fn insert_or_update(
    conn: &Connection,
    scope: PrefScope,
    key: &str,
    value: &str,
) -> Result<usize> {
    let uuid_str = scope.to_uuid().to_string();
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM preferences \
         WHERE vault_device_uuid = ?1 AND key = ?2",
        params![uuid_str, key],
        |r| r.get(0),
    )?;
    if exists > 0 {
        let update_sql = format!(
            "UPDATE preferences \
             SET value = ?1, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
             WHERE vault_device_uuid = ?2 AND key = ?3"
        );
        conn.execute(&update_sql, params![value, uuid_str, key])
    } else {
        let insert_sql = format!(
            "INSERT INTO preferences \
               (vault_device_uuid, key, value, {HLC_TIMESTAMP_COLUMN}) \
             VALUES (?1, ?2, ?3, current_hlc())"
        );
        conn.execute(&insert_sql, params![uuid_str, key, value])
    }
}

/// Deletes a preference row. Idempotent — deleting an absent row is
/// not an error.
pub fn delete(conn: &Connection, scope: PrefScope, key: &str) -> Result<usize> {
    conn.execute(
        "DELETE FROM preferences WHERE vault_device_uuid = ?1 AND key = ?2",
        params![scope.to_uuid().to_string(), key],
    )
}

/// Lists every preference row for the given scope, ordered by key.
pub fn list_by_scope(conn: &Connection, scope: PrefScope) -> Result<Vec<PrefRow>> {
    let mut stmt = conn.prepare(
        "SELECT key, value FROM preferences \
         WHERE vault_device_uuid = ?1 \
         ORDER BY key ASC",
    )?;
    let rows = stmt.query_map(params![scope.to_uuid().to_string()], |r| {
        Ok(PrefRow {
            scope,
            key: r.get(0)?,
            value: r.get(1)?,
        })
    })?;
    rows.collect()
}

/// Lists every preference row for the given key across all scopes,
/// ordered by `vault_device_uuid`.
pub fn list_by_key(conn: &Connection, key: &str) -> Result<Vec<PrefRow>> {
    let mut stmt = conn.prepare(
        "SELECT vault_device_uuid, value FROM preferences \
         WHERE key = ?1 \
         ORDER BY vault_device_uuid ASC",
    )?;
    let key_owned = key.to_string();
    let rows = stmt.query_map(params![key], move |r| {
        let uuid_str: String = r.get(0)?;
        let uuid = Uuid::parse_str(&uuid_str).map_err(|e| {
            haex_crdt::rusqlite::Error::FromSqlConversionFailure(
                0,
                haex_crdt::rusqlite::types::Type::Text,
                Box::new(e),
            )
        })?;
        let scope = if uuid == VAULT_SCOPE_UUID {
            PrefScope::Vault
        } else {
            PrefScope::Device(uuid)
        };
        Ok(PrefRow {
            scope,
            key: key_owned.clone(),
            value: r.get(1)?,
        })
    })?;
    rows.collect()
}
