//! Starting an extension on this device before its first frame (FR-003, data-model.md
//! §Zustände): it must be installed and enabled, its effective bundle complete and verifying, and
//! its migrations applied; the outcome becomes its state on this device.

use uuid::Uuid;

use super::effective::effective_bundle;
use super::status::{self, DeviceStatus};
use crate::error::{HolziError, Result};
use crate::extensions::bundle::store::{verify_stored_bundle, StoredBundleState};
use crate::extensions::bundle::Manifest;
use crate::extensions::ids::TablePrefix;
use crate::extensions::protocol::{csp, prefix};
use crate::extensions::sql::migrate::{apply_pending, check_applied_kept, MigrationError};
use crate::storage::query::Query;
use crate::vault_gate::VaultDb;

/// What a frame of a started extension needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub bundle_id: Uuid,
    /// Path of the entry page inside the bundle.
    pub entry: String,
    /// The Content-Security-Policy header of its files.
    pub csp: String,
}

enum Registration {
    Missing,
    Disabled,
    Enabled,
}

fn registration(q: &mut impl Query, extension_id: Uuid) -> Result<Registration> {
    let row = q.query_row(
        "SELECT enabled, state FROM extensions WHERE id = ?1",
        &[&extension_id.to_string()],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    )?;
    Ok(match row {
        Some((_, state)) if state != "installed" => Registration::Missing,
        None => Registration::Missing,
        Some((0, _)) => Registration::Disabled,
        Some(_) => Registration::Enabled,
    })
}

fn not_ready(status: DeviceStatus) -> HolziError {
    HolziError::ExtensionNotReady {
        status: status.as_str().to_owned(),
    }
}

/// The error kind stored in the device status; no data of the extension.
fn migration_error_kind(error: &MigrationError) -> &'static str {
    match error {
        MigrationError::Changed { .. } => "migration_changed",
        MigrationError::Refused { .. } => "migration_refused",
        MigrationError::Failed { .. } => "migration_failed",
        MigrationError::Missing { .. } => "migration_missing",
        MigrationError::Unavailable => "database_unavailable",
    }
}

/// Checks and prepares `extension_id` on `device`. Blocking: run it on a blocking thread.
pub fn start(db: &VaultDb, extension_id: Uuid, device: Uuid, now_ms: i64) -> Result<Started> {
    let checked = db.read_blocking(move |q| {
        let state = match registration(q, extension_id)? {
            Registration::Missing => return Err(HolziError::ExtensionNotFound.into()),
            Registration::Disabled => return Err(HolziError::ExtensionDisabled.into()),
            Registration::Enabled => match effective_bundle(q, extension_id)? {
                None => None,
                Some(effective) => Some((
                    effective.bundle_id,
                    verify_stored_bundle(q, effective.bundle_id)?,
                )),
            },
        };
        Ok(state)
    })?;

    let (status, bundle_id, error, started) = match checked {
        None => (DeviceStatus::Transferring, None, None, None),
        Some((bundle_id, StoredBundleState::Transferring)) => {
            (DeviceStatus::Transferring, Some(bundle_id), None, None)
        }
        Some((bundle_id, StoredBundleState::SignatureFailed(rejection))) => (
            DeviceStatus::SignatureFailed,
            Some(bundle_id),
            Some(rejection.kind),
            None,
        ),
        Some((bundle_id, StoredBundleState::Ready(bundle))) => {
            let manifest = Manifest::from_verified(&bundle)?;
            let own = TablePrefix {
                public_key: manifest.public_key.clone(),
                name: manifest.name.clone(),
            };
            let offered: Vec<(String, String)> = bundle
                .migrations
                .iter()
                .map(|m| (m.name.clone(), m.sql.clone()))
                .collect();
            let kept = db
                .read_blocking(move |q| Ok(check_applied_kept(q, extension_id, &offered)))
                .unwrap_or(Err(MigrationError::Unavailable));
            match kept.and_then(|()| apply_pending(db, extension_id, &own, now_ms)) {
                Ok(_) => {
                    let started = Started {
                        bundle_id,
                        entry: manifest.entry,
                        csp: csp::for_bundle(&bundle, &prefix(extension_id)),
                    };
                    (DeviceStatus::Ready, Some(bundle_id), None, Some(started))
                }
                Err(error) => {
                    log::warn!("extension {extension_id}: migrations failed: {error:?}");
                    (
                        DeviceStatus::MigrationFailed,
                        Some(bundle_id),
                        Some(migration_error_kind(&error).to_owned()),
                        None,
                    )
                }
            }
        }
    };
    db.write_blocking(move |tx| {
        status::set(
            tx,
            extension_id,
            device,
            status,
            bundle_id,
            error.as_deref(),
            now_ms,
        )
        .map_err(Into::into)
    })?;
    started.ok_or_else(|| not_ready(status))
}

#[cfg(test)]
#[path = "start_tests.rs"]
mod tests;
