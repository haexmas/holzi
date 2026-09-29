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
//! Writes take a [`CrdtTransaction`]; haex-crdt stamps the HLC. Reads take
//! any [`Query`], so they also run inside a write.

use haex_crdt::rusqlite::params;
use haex_crdt::{CrdtTransaction, Result};
use uuid::Uuid;

use crate::identity::VAULT_SCOPE_UUID;
use crate::storage::query::Query;

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
pub fn get(q: &mut impl Query, scope: PrefScope, key: &str) -> Result<Option<String>> {
    let raw: Option<Option<String>> = q.query_row(
        "SELECT value FROM preferences \
         WHERE vault_device_uuid = ?1 AND key = ?2",
        params![scope.to_uuid().to_string(), key],
        |r| r.get::<_, Option<String>>(0),
    )?;
    Ok(raw.flatten())
}

/// Inserts or updates a preference row.
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
    tx: &mut CrdtTransaction<'_>,
    scope: PrefScope,
    key: &str,
    value: &str,
) -> Result<usize> {
    let uuid_str = scope.to_uuid().to_string();
    let exists = tx
        .query_row(
            "SELECT COUNT(*) FROM preferences \
             WHERE vault_device_uuid = ?1 AND key = ?2",
            params![uuid_str, key],
            |r| r.get::<_, i64>(0),
        )?
        .unwrap_or(0);
    if exists > 0 {
        tx.execute(
            "UPDATE preferences SET value = ?1 \
             WHERE vault_device_uuid = ?2 AND key = ?3",
            params![value, uuid_str, key],
        )
    } else {
        tx.execute(
            "INSERT INTO preferences (vault_device_uuid, key, value) VALUES (?1, ?2, ?3)",
            params![uuid_str, key, value],
        )
    }
}

/// Deletes a preference row. Idempotent — deleting an absent row is
/// not an error.
pub fn delete(tx: &mut CrdtTransaction<'_>, scope: PrefScope, key: &str) -> Result<usize> {
    tx.execute(
        "DELETE FROM preferences WHERE vault_device_uuid = ?1 AND key = ?2",
        params![scope.to_uuid().to_string(), key],
    )
}

/// Parses a stored boolean preference. Only `'true'` and `'false'` count;
/// anything else reads as unset, like an absent row (the `voice.auto_send`
/// convention).
pub fn parse_bool(value: Option<&str>) -> Option<bool> {
    match value {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    }
}

/// Lists every preference row for the given scope, ordered by key.
pub fn list_by_scope(q: &mut impl Query, scope: PrefScope) -> Result<Vec<PrefRow>> {
    q.query_map(
        "SELECT key, value FROM preferences \
         WHERE vault_device_uuid = ?1 \
         ORDER BY key ASC",
        params![scope.to_uuid().to_string()],
        |r| {
            Ok(PrefRow {
                scope,
                key: r.get(0)?,
                value: r.get(1)?,
            })
        },
    )
}

/// Lists every preference row for the given key across all scopes,
/// ordered by `vault_device_uuid`.
pub fn list_by_key(q: &mut impl Query, key: &str) -> Result<Vec<PrefRow>> {
    let key_owned = key.to_string();
    q.query_map(
        "SELECT vault_device_uuid, value FROM preferences \
         WHERE key = ?1 \
         ORDER BY vault_device_uuid ASC",
        params![key],
        move |r| {
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
        },
    )
}
