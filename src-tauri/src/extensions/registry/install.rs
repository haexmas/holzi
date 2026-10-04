//! Installing an extension from a `.xt` file (US1, FR-001..FR-005, contracts/tauri-commands.md).
//!
//! The preview reads and verifies the file without writing anything; the install verifies it
//! again (the file may have changed in between), stores the BLOBs each in a write of its own and
//! then, in one write, the registry rows and the declared permissions
//! (contracts/permissions.md §Installation).

use std::collections::HashMap;
use std::path::Path;

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::effective::{effective_bundle, live_bundles};
use crate::error::{HolziError, Result};
use crate::extensions::bundle::store::{store_blobs, write_registry_rows, BundleIds};
use crate::extensions::bundle::{limits, verify_bundle, BundleRejection, Manifest, VerifiedBundle};
use crate::extensions::ids::{extension_id, TablePrefix};
use crate::extensions::permissions::manifest_map::DeclaredPermission;
use crate::extensions::permissions::store::{self as permission_store, NewPermission};
use crate::extensions::permissions::{PermissionStatus, VAULT_WIDE};
use crate::storage::query::Query;
use crate::vault_gate::VaultDb;

/// One declared permission as the install dialog shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct DeclaredPermissionView {
    pub kind: String,
    pub action: String,
    pub target: String,
    /// Remembered only for this device unless the user chooses all devices.
    pub device_scoped: bool,
}

impl From<&DeclaredPermission> for DeclaredPermissionView {
    fn from(d: &DeclaredPermission) -> Self {
        Self {
            kind: d.kind.as_str().to_owned(),
            action: d.action.as_string(),
            target: d.target_text.clone(),
            device_scoped: d.kind.is_device_scoped(),
        }
    }
}

/// The installed extension a bundle would update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct ExistingInstall {
    /// The effective version installed now.
    pub version: String,
    /// The bundle is older than the effective version; installing needs a confirmation.
    pub is_downgrade: bool,
    /// Declarations without any remembered row yet; only these are put before the user.
    pub new_permissions: Vec<DeclaredPermissionView>,
}

/// What the install dialog shows before anything is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct InstallPreview {
    /// The bundle passed every check of the format.
    pub signature_valid: bool,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BundleRejection>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension_id: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Short form of the publisher key: the first and last four groups of four hex digits.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher_fingerprint: Option<String>,
    pub declared: Vec<DeclaredPermissionView>,
    /// Manifest categories holzi does not offer ("wird von holzi nicht unterstützt").
    pub unsupported_categories: Vec<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing: Option<ExistingInstall>,
    /// Another installed extension has the same name but another publisher (US7-2).
    pub same_name_other_publisher: bool,
}

impl InstallPreview {
    fn refused(error: BundleRejection) -> Self {
        Self {
            signature_valid: false,
            error: Some(error),
            extension_id: None,
            name: None,
            display_name: None,
            version: None,
            description: None,
            author: None,
            publisher_fingerprint: None,
            declared: Vec::new(),
            unsupported_categories: Vec::new(),
            existing: None,
            same_name_other_publisher: false,
        }
    }
}

/// The user's choice for one declared permission.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct PermissionChoice {
    pub kind: String,
    pub action: String,
    pub target: String,
    /// Ticked: `granted`; unticked: `ask`.
    pub granted: bool,
    /// For device-scoped kinds: remember for every own device instead of only this one.
    #[serde(default)]
    pub all_devices: bool,
}

/// `3614 253f 84ba 66a8 … 3ca2 823f 5ca9 7d34`.
pub fn publisher_fingerprint(public_key: &str) -> String {
    let groups: Vec<&str> = (0..public_key.len())
        .step_by(4)
        .filter_map(|i| public_key.get(i..i + 4))
        .collect();
    if groups.len() <= 8 {
        return groups.join(" ");
    }
    format!(
        "{} … {}",
        groups[..4].join(" "),
        groups[groups.len() - 4..].join(" ")
    )
}

/// Reads a `.xt` file: its size is checked on the metadata before a byte is read. Blocking.
pub fn read_bundle_file(path: &Path) -> Result<Vec<u8>> {
    let unreadable = || HolziError::ExtensionInstall {
        reason: "unreadable".into(),
    };
    let metadata = std::fs::metadata(path).map_err(|_| unreadable())?;
    if !metadata.is_file() {
        return Err(unreadable());
    }
    if metadata.len() > limits::ARCHIVE_BYTES {
        return Err(HolziError::ExtensionInstall {
            reason: haex_bundle::ErrorKind::ArchiveTooLarge.as_str().into(),
        });
    }
    std::fs::read(path).map_err(|_| unreadable())
}

fn key_of(kind: &str, action: &str, target: &str) -> (String, String, String) {
    (kind.to_owned(), action.to_owned(), target.to_owned())
}

/// Declarations that have no remembered row at all yet (on any device, declared or not).
fn new_declarations<'a>(
    declared: &'a [DeclaredPermission],
    rows: &[permission_store::PermissionRow],
) -> Vec<&'a DeclaredPermission> {
    declared
        .iter()
        .filter(|d| {
            !rows
                .iter()
                .any(|r| r.is_about(d.kind.as_str(), &d.action.as_string(), &d.target_text))
        })
        .collect()
}

fn is_installed(q: &mut impl Query, id: Uuid) -> Result<bool> {
    Ok(q.query_row(
        "SELECT COUNT(*) FROM extensions WHERE id = ?1 AND state = 'installed'",
        &[&id.to_string()],
        |r| r.get::<_, i64>(0),
    )?
    .unwrap_or(0)
        > 0)
}

/// The preview of a verified bundle against the vault.
pub fn preview_of(
    q: &mut impl Query,
    bundle: &VerifiedBundle,
    manifest: &Manifest,
) -> Result<InstallPreview> {
    let id = extension_id(&manifest.public_key, &manifest.name);
    let declared = &manifest.permissions.declared;
    let existing = if is_installed(q, id)? {
        let rows = permission_store::rows_of(q, id)?;
        effective_bundle(q, id)?.map(|current| ExistingInstall {
            version: current.version.to_string(),
            is_downgrade: manifest.version < current.version,
            new_permissions: new_declarations(declared, &rows)
                .into_iter()
                .map(DeclaredPermissionView::from)
                .collect(),
        })
    } else {
        None
    };
    let same_name_other_publisher = q
        .query_row(
            "SELECT COUNT(*) FROM extensions \
             WHERE name = ?1 AND public_key <> ?2 AND state = 'installed'",
            &[&manifest.name.as_str(), &manifest.public_key.as_str()],
            |r| r.get::<_, i64>(0),
        )?
        .unwrap_or(0)
        > 0;
    Ok(InstallPreview {
        signature_valid: true,
        error: None,
        extension_id: Some(id.to_string()),
        name: Some(manifest.name.as_str().to_owned()),
        display_name: manifest.display_name.clone(),
        version: Some(manifest.version.to_string()),
        description: manifest.description.clone(),
        author: manifest.author.clone(),
        publisher_fingerprint: Some(publisher_fingerprint(&bundle.public_key)),
        declared: declared.iter().map(DeclaredPermissionView::from).collect(),
        unsupported_categories: manifest.permissions.unsupported_categories.clone(),
        existing,
        same_name_other_publisher,
    })
}

/// Reads and verifies `bytes` and builds the preview; a refused bundle is a preview with `error`.
pub fn install_preview(q: &mut impl Query, bytes: &[u8]) -> Result<InstallPreview> {
    let bundle = match verify_bundle(bytes) {
        Ok(bundle) => bundle,
        Err(rejection) => return Ok(InstallPreview::refused(rejection)),
    };
    let manifest = match Manifest::from_verified(&bundle) {
        Ok(manifest) => manifest,
        Err(rejection) => return Ok(InstallPreview::refused(rejection)),
    };
    preview_of(q, &bundle, &manifest)
}

/// Writes the declared permissions (contracts/permissions.md §Installation): a declaration without
/// any row becomes `granted` (ticked) or `ask` (unticked) with `declared = 1`, for device-scoped
/// kinds on this device unless the choice says all devices; a runtime row (`declared = 0`) the
/// manifest now declares becomes `declared = 1` with its state kept; a declared row the manifest
/// no longer declares is deleted; every other row stays.
pub fn apply_declarations(
    tx: &mut CrdtTransaction<'_>,
    extension_id: Uuid,
    declared: &[DeclaredPermission],
    choices: &[PermissionChoice],
    device: Uuid,
    now_ms: i64,
) -> Result<()> {
    let rows = permission_store::rows_of(tx, extension_id)?;
    let choices: HashMap<_, _> = choices
        .iter()
        .map(|c| (key_of(&c.kind, &c.action, &c.target), c))
        .collect();

    for row in &rows {
        let still_declared = declared
            .iter()
            .any(|d| row.is_about(d.kind.as_str(), &d.action.as_string(), &d.target_text));
        if row.declared && !still_declared {
            permission_store::delete(tx, row.id)?;
        } else if !row.declared && still_declared {
            permission_store::mark_declared(tx, row.id, now_ms)?;
        }
    }

    for declaration in new_declarations(declared, &rows) {
        let action = declaration.action.as_string();
        let kind = declaration.kind.as_str();
        let choice = choices.get(&key_of(kind, &action, &declaration.target_text));
        let granted = choice.is_some_and(|c| c.granted);
        let all_devices = choice.is_some_and(|c| c.all_devices);
        let status = if granted {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Ask
        };
        let scope = if declaration.kind.is_device_scoped() && !all_devices {
            device
        } else {
            VAULT_WIDE
        };
        permission_store::put(
            tx,
            extension_id,
            &NewPermission {
                kind,
                action: &action,
                target: &declaration.target_text,
                status: status.as_str(),
                declared: true,
                vault_device_uuid: scope,
            },
            now_ms,
        )?;
    }
    Ok(())
}

/// The display name of the extension follows its effective bundle.
fn refresh_display_name(tx: &mut CrdtTransaction<'_>, extension_id: Uuid) -> Result<()> {
    let Some(effective) = effective_bundle(tx, extension_id)? else {
        return Ok(());
    };
    let manifest_json = tx
        .query_row(
            "SELECT manifest_json FROM extension_bundles WHERE id = ?1",
            &[&effective.bundle_id.to_string()],
            |r| r.get::<_, Vec<u8>>(0),
        )?
        .unwrap_or_default();
    let display_name = std::str::from_utf8(&manifest_json)
        .ok()
        .and_then(|text| haex_bundle::jcs::parse_restricted(text).ok())
        .and_then(|m| {
            m.as_object()?
                .get("displayName")?
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        });
    let current = tx
        .query_row(
            "SELECT display_name FROM extensions WHERE id = ?1",
            &[&extension_id.to_string()],
            |r| r.get::<_, Option<String>>(0),
        )?
        .flatten();
    if current != display_name {
        tx.execute(
            "UPDATE extensions SET display_name = ?2 WHERE id = ?1",
            params![extension_id.to_string(), display_name],
        )?;
    }
    Ok(())
}

/// Retires every live bundle newer than `version` (a confirmed downgrade, R11).
fn retire_newer(
    tx: &mut CrdtTransaction<'_>,
    extension_id: Uuid,
    version: &semver::Version,
) -> Result<()> {
    for newer in live_bundles(tx, extension_id)?
        .into_iter()
        .filter(|b| &b.version > version)
    {
        tx.execute(
            "UPDATE extension_bundles SET retired = 1 WHERE id = ?1",
            params![newer.bundle_id.to_string()],
        )?;
    }
    Ok(())
}

/// What an install did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installed {
    pub ids: BundleIds,
}

/// Installs a bundle from its bytes. Blocking: run it on a blocking thread.
///
/// Verifies again, refuses a downgrade without `confirm_downgrade`, stores the BLOBs (each in its
/// own write) and then, in one write, the registry rows, the permissions, a retirement of newer
/// bundles on a downgrade and the display name of the effective bundle.
pub fn install(
    db: &VaultDb,
    bytes: &[u8],
    choices: Vec<PermissionChoice>,
    confirm_downgrade: bool,
    device: Uuid,
    now_ms: i64,
) -> Result<Installed> {
    let bundle = verify_bundle(bytes)?;
    let manifest = Manifest::from_verified(&bundle)?;
    let id = extension_id(&manifest.public_key, &manifest.name);
    // The tables of that prefix here belong to a development version until it is unloaded (US12).
    let prefix = TablePrefix {
        public_key: manifest.public_key.clone(),
        name: manifest.name.clone(),
    };
    if super::start::dev_prefix_here(db, &prefix)? {
        return Err(HolziError::ExtensionInstall {
            reason: "dev_prefix_conflict".into(),
        });
    }
    let current = db.read_blocking(move |q| {
        if !is_installed(q, id)? {
            return Ok(None);
        }
        effective_bundle(q, id).map_err(Into::into)
    })?;
    let is_downgrade = current.is_some_and(|c| manifest.version < c.version);
    if is_downgrade && !confirm_downgrade {
        return Err(HolziError::ExtensionInstall {
            reason: "downgrade_not_confirmed".into(),
        });
    }

    store_blobs(db, &bundle)?;
    let ids = db.write_blocking(move |tx| {
        let ids = write_registry_rows(tx, &bundle, &manifest, now_ms)?;
        apply_declarations(
            tx,
            ids.extension_id,
            &manifest.permissions.declared,
            &choices,
            device,
            now_ms,
        )?;
        if is_downgrade {
            retire_newer(tx, ids.extension_id, &manifest.version)?;
        }
        refresh_display_name(tx, ids.extension_id)?;
        Ok(ids)
    })?;
    Ok(Installed { ids })
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod install_tests;
