//! The state of an extension on each device (`extension_device_status`, data-model.md): shown in
//! the settings of every device, written only by the device it describes.

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;
use uuid::Uuid;

use crate::error::Result;
use crate::extensions::ids::device_status_id;

/// The states of data-model.md §Zustände.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Transferring,
    Ready,
    SignatureFailed,
    MigrationFailed,
    Disabled,
}

impl DeviceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transferring => "transferring",
            Self::Ready => "ready",
            Self::SignatureFailed => "signature_failed",
            Self::MigrationFailed => "migration_failed",
            Self::Disabled => "disabled",
        }
    }
}

/// The error of a device whose parked sync groups of the extension reached their limit: the sync
/// waits for more of its data until the extension is installed there or removed with "delete data"
/// (research R10). The sync receiver sets it; it stays while the extension is still transferring.
pub const PARKED_LIMIT: &str = "parked_limit";

/// Writes the state of `extension_id` on `device`; a row that already says the same is left alone,
/// so starting an extension does not produce sync traffic. A `transferring` state without an error
/// keeps a [`PARKED_LIMIT`] error the row already has.
pub fn set(
    tx: &mut CrdtTransaction<'_>,
    extension_id: Uuid,
    device: Uuid,
    status: DeviceStatus,
    bundle_id: Option<Uuid>,
    error: Option<&str>,
    now_ms: i64,
) -> Result<()> {
    let id = device_status_id(extension_id, device).to_string();
    let bundle = bundle_id.map(|b| b.to_string());
    let current = tx.query_row(
        "SELECT status, bundle_id, error FROM extension_device_status WHERE id = ?1",
        params![id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        },
    )?;
    let kept = current
        .as_ref()
        .and_then(|(_, _, e)| e.as_deref())
        .filter(|e| *e == PARKED_LIMIT && status == DeviceStatus::Transferring && error.is_none());
    let error = kept.map(str::to_owned).or(error.map(str::to_owned));
    let error = error.as_deref();
    match current {
        Some((s, b, e)) if s == status.as_str() && b == bundle && e.as_deref() == error => {}
        Some(_) => {
            tx.execute(
                "UPDATE extension_device_status \
                 SET status = ?2, bundle_id = ?3, error = ?4, updated_at = ?5 WHERE id = ?1",
                params![id, status.as_str(), bundle, error, now_ms],
            )?;
        }
        None => {
            tx.execute(
                "INSERT INTO extension_device_status \
                 (id, extension_id, vault_device_uuid, status, bundle_id, error, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    extension_id.to_string(),
                    device.to_string(),
                    status.as_str(),
                    bundle,
                    error,
                    now_ms
                ],
            )?;
        }
    }
    Ok(())
}

/// Records on `device` that the parked sync groups of `extension_id` reached their limit, keeping
/// the state the row has (`transferring` for a new row).
pub fn mark_parked_limit(
    tx: &mut CrdtTransaction<'_>,
    extension_id: Uuid,
    device: Uuid,
    now_ms: i64,
) -> Result<()> {
    let id = device_status_id(extension_id, device).to_string();
    let current = tx.query_row(
        "SELECT error FROM extension_device_status WHERE id = ?1",
        params![id],
        |r| r.get::<_, Option<String>>(0),
    )?;
    match current {
        Some(Some(error)) if error == PARKED_LIMIT => {}
        Some(_) => {
            tx.execute(
                "UPDATE extension_device_status SET error = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, PARKED_LIMIT, now_ms],
            )?;
        }
        None => {
            tx.execute(
                "INSERT INTO extension_device_status \
                 (id, extension_id, vault_device_uuid, status, bundle_id, error, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)",
                params![
                    id,
                    extension_id.to_string(),
                    device.to_string(),
                    DeviceStatus::Transferring.as_str(),
                    PARKED_LIMIT,
                    now_ms
                ],
            )?;
        }
    }
    Ok(())
}
