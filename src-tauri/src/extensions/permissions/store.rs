//! Remembered permissions in `extension_permissions` (data-model.md). Row ids derive from the
//! natural key (`ids::permission_id`), so two devices that remember the same permission write the
//! same row. Writes are check-then-write like the other CRDT tables (`storage/preferences.rs`).

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;
use uuid::Uuid;

use super::{Permission, PermissionKind, VAULT_WIDE};
use crate::error::Result;
use crate::extensions::ids::permission_id;
use crate::storage::query::Query;

/// One stored row as text; [`super::Permission::from_row`] reads it for decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRow {
    pub id: Uuid,
    pub kind: String,
    pub action: String,
    pub target: String,
    pub status: String,
    pub declared: bool,
    pub vault_device_uuid: Uuid,
}

impl PermissionRow {
    /// Whether this row is about the same (kind, action, target).
    pub fn is_about(&self, kind: &str, action: &str, target: &str) -> bool {
        self.kind == kind && self.action == action && self.target == target
    }
}

/// Every row of an extension, on every device.
pub fn rows_of(q: &mut impl Query, extension_id: Uuid) -> Result<Vec<PermissionRow>> {
    let rows = q.query_map(
        "SELECT id, kind, action, target, status, declared, vault_device_uuid \
         FROM extension_permissions WHERE extension_id = ?1",
        &[&extension_id.to_string()],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
            ))
        },
    )?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, kind, action, target, status, declared, device)| {
            Some(PermissionRow {
                id: Uuid::parse_str(&id).ok()?,
                kind,
                action,
                target,
                status,
                declared: declared != 0,
                vault_device_uuid: Uuid::parse_str(&device).ok()?,
            })
        })
        .collect())
}

/// The remembered permissions of one kind that can hold on `device`: vault-wide or this device's.
/// Rows holzi cannot read are absent (FR-022).
pub fn candidates(
    q: &mut impl Query,
    extension_id: Uuid,
    kind: PermissionKind,
    device: Uuid,
) -> Result<Vec<Permission>> {
    Ok(rows_of(q, extension_id)?
        .into_iter()
        .filter(|r| r.kind == kind.as_str())
        .filter(|r| r.vault_device_uuid == VAULT_WIDE || r.vault_device_uuid == device)
        .filter_map(|r| {
            Permission::from_row(
                &r.kind,
                &r.action,
                &r.target,
                &r.status,
                r.vault_device_uuid,
            )
        })
        .collect())
}

/// What to write for one permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPermission<'a> {
    pub kind: &'a str,
    pub action: &'a str,
    pub target: &'a str,
    pub status: &'a str,
    pub declared: bool,
    pub vault_device_uuid: Uuid,
}

/// Writes a permission: a new row, or the state and `declared` of the row with the same key.
pub fn put(
    tx: &mut CrdtTransaction<'_>,
    extension_id: Uuid,
    permission: &NewPermission<'_>,
    now_ms: i64,
) -> Result<Uuid> {
    let id = permission_id(
        extension_id,
        permission.kind,
        permission.action,
        permission.target,
        permission.vault_device_uuid,
    );
    let key = id.to_string();
    let known = tx
        .query_row(
            "SELECT COUNT(*) FROM extension_permissions WHERE id = ?1",
            params![key],
            |r| r.get::<_, i64>(0),
        )?
        .unwrap_or(0)
        > 0;
    if known {
        tx.execute(
            "UPDATE extension_permissions SET status = ?2, declared = ?3, updated_at = ?4 \
             WHERE id = ?1",
            params![key, permission.status, permission.declared, now_ms],
        )?;
    } else {
        tx.execute(
            "INSERT INTO extension_permissions (id, extension_id, kind, action, target, status, \
             declared, vault_device_uuid, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                key,
                extension_id.to_string(),
                permission.kind,
                permission.action,
                permission.target,
                permission.status,
                permission.declared,
                permission.vault_device_uuid.to_string(),
                now_ms
            ],
        )?;
    }
    Ok(id)
}

/// Marks a row as declared by the manifest, keeping its state.
pub fn mark_declared(tx: &mut CrdtTransaction<'_>, id: Uuid, now_ms: i64) -> Result<()> {
    tx.execute(
        "UPDATE extension_permissions SET declared = 1, updated_at = ?2 WHERE id = ?1",
        params![id.to_string(), now_ms],
    )?;
    Ok(())
}

pub fn delete(tx: &mut CrdtTransaction<'_>, id: Uuid) -> Result<()> {
    tx.execute(
        "DELETE FROM extension_permissions WHERE id = ?1",
        params![id.to_string()],
    )?;
    Ok(())
}
