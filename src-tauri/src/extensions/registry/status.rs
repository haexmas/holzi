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

/// Writes the state of `extension_id` on `device`; a row that already says the same is left alone,
/// so starting an extension does not produce sync traffic.
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
