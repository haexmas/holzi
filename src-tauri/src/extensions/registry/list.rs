//! The installed extensions as the launcher, the settings and the app list of the wm see them.

use base64::Engine;
use serde::Serialize;
use ts_rs::TS;
use uuid::Uuid;

use super::effective::effective_bundle;
use super::install::publisher_fingerprint;
use crate::error::Result;
use crate::extensions::bundle::store::read_verified_file;
use crate::extensions::bundle::Manifest;
use crate::extensions::ids::{ExtensionName, PublicKey, TablePrefix};
use crate::extensions::mime;
use crate::storage::known_devices;
use crate::storage::query::Query;

/// The state of an extension on one own device (US4, T080).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct DeviceState {
    /// The device's `vault_device_uuid`.
    pub device_id: String,
    /// The device's name; empty when it has none.
    pub device_name: String,
    pub this_device: bool,
    /// `transferring`, `ready`, `signature_failed`, `migration_failed` or `disabled`.
    pub status: String,
    /// The error kind (`migration_changed`, …), never data of the extension.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One extension of the vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct ExtensionSummary {
    pub id: String,
    pub name: String,
    /// `displayName` of the effective bundle, else `name`.
    pub title: String,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Version of the effective bundle; `None` while no bundle of it is live.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub publisher_fingerprint: String,
    pub enabled: bool,
    /// `installed`, or `removed` with its data kept (FR-008); one removed with its data is not
    /// listed.
    pub state: String,
    /// For `removed`: the bytes its kept tables take on this device, indexes included; `None` when
    /// SQLite cannot tell.
    #[ts(optional, type = "number")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_data_bytes: Option<u64>,
    /// The app opens at most once (manifest `singleInstance`).
    pub single_instance: bool,
    pub has_icon: bool,
    /// State on this device (`transferring`, `ready`, …); `None` before its first start here.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_here: Option<String>,
    /// The error kind of that state.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_error_here: Option<String>,
    /// The state on every own device that started it, this device first. Inline, so the binding
    /// imports nothing (the check scripts load it with Node's own module rules).
    #[ts(inline)]
    pub devices: Vec<DeviceState>,
    /// A development version on this device (US12): listed while developer mode is on.
    pub dev: bool,
}

struct Row {
    id: String,
    public_key: String,
    name: String,
    display_name: Option<String>,
    enabled: bool,
    state: String,
}

/// The bytes the tables of `prefix` and their indexes take in this vault file, from SQLite's
/// `dbstat`; `None` when it cannot be read.
fn data_bytes(q: &mut impl Query, public_key: &str, name: &str) -> Option<u64> {
    let prefix = TablePrefix {
        public_key: PublicKey::parse_case_insensitive(public_key).ok()?,
        name: ExtensionName::parse(name).ok()?,
    }
    .to_string();
    // Not LIKE: `_` is a wildcard there, and every prefix holds two of them.
    q.query_row(
        "SELECT COALESCE(SUM(pgsize), 0) FROM dbstat WHERE name IN \
         (SELECT name FROM sqlite_master \
          WHERE lower(substr(tbl_name, 1, length(?1))) = lower(?1))",
        &[&prefix],
        |r| r.get::<_, i64>(0),
    )
    .ok()
    .flatten()
    .and_then(|bytes| u64::try_from(bytes).ok())
}

/// The manifest of the effective bundle.
pub fn effective_manifest(
    q: &mut impl Query,
    extension_id: Uuid,
) -> Result<Option<(Uuid, Manifest)>> {
    let Some(effective) = effective_bundle(q, extension_id)? else {
        return Ok(None);
    };
    let bytes = q
        .query_row(
            "SELECT manifest_json FROM extension_bundles WHERE id = ?1",
            &[&effective.bundle_id.to_string()],
            |r| r.get::<_, Vec<u8>>(0),
        )?
        .unwrap_or_default();
    Ok(Manifest::from_stored(&bytes)
        .ok()
        .map(|manifest| (effective.bundle_id, manifest)))
}

/// Every extension of the vault, ordered by title.
pub fn list(q: &mut impl Query, device: Uuid) -> Result<Vec<ExtensionSummary>> {
    let rows = q.query_map(
        "SELECT id, public_key, name, display_name, enabled, state FROM extensions \
         WHERE state = 'installed' OR purge_data = 0",
        &[],
        |r| {
            Ok(Row {
                id: r.get(0)?,
                public_key: r.get(1)?,
                name: r.get(2)?,
                display_name: r.get(3)?,
                enabled: r.get::<_, i64>(4)? != 0,
                state: r.get(5)?,
            })
        },
    )?;
    let names: std::collections::HashMap<String, String> = known_devices::list_devices(q)?
        .into_iter()
        .map(|d| (d.vault_device_uuid.to_string(), d.alias.unwrap_or_default()))
        .collect();
    let here = device.to_string();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let Ok(id) = Uuid::parse_str(&row.id) else {
            continue;
        };
        let manifest = effective_manifest(q, id)?.map(|(_, m)| m);
        let mut devices: Vec<DeviceState> = q.query_map(
            "SELECT vault_device_uuid, status, error FROM extension_device_status \
                 WHERE extension_id = ?1",
            &[&row.id],
            |r| {
                let uuid: String = r.get(0)?;
                Ok(DeviceState {
                    device_name: names.get(&uuid).cloned().unwrap_or_default(),
                    this_device: uuid == here,
                    device_id: uuid,
                    status: r.get(1)?,
                    error: r.get(2)?,
                })
            },
        )?;
        devices.sort_by(|a, b| {
            b.this_device.cmp(&a.this_device).then_with(|| {
                a.device_name
                    .to_lowercase()
                    .cmp(&b.device_name.to_lowercase())
            })
        });
        let mine = devices.iter().find(|d| d.this_device);
        let status_here = mine.map(|d| d.status.clone());
        let status_error_here = mine.and_then(|d| d.error.clone());
        let kept_data_bytes = if row.state == "installed" {
            None
        } else {
            data_bytes(q, &row.public_key, &row.name)
        };
        out.push(ExtensionSummary {
            title: manifest.as_ref().map_or_else(
                || row.display_name.clone().unwrap_or_else(|| row.name.clone()),
                |m| m.title().to_owned(),
            ),
            description: manifest.as_ref().and_then(|m| m.description.clone()),
            version: manifest.as_ref().map(|m| m.version.to_string()),
            single_instance: manifest.as_ref().is_some_and(|m| m.single_instance),
            has_icon: manifest
                .as_ref()
                .and_then(|m| m.icon.as_deref())
                .is_some_and(mime::is_image),
            publisher_fingerprint: publisher_fingerprint(&row.public_key),
            id: row.id,
            name: row.name,
            enabled: row.enabled,
            state: row.state,
            kept_data_bytes,
            status_here,
            status_error_here,
            devices,
            dev: false,
        });
    }
    if crate::extensions::dev::mode(q, device)? {
        out.extend(
            crate::extensions::dev::registrations(q, device)?
                .into_iter()
                .map(dev_summary),
        );
    }
    out.sort_by_key(|e| e.title.to_lowercase());
    Ok(out)
}

/// A development version as the launcher and the settings show it; what its manifest says now.
fn dev_summary(registration: crate::extensions::dev::DevRegistration) -> ExtensionSummary {
    let manifest =
        crate::extensions::dev::read_project(std::path::Path::new(&registration.project_path))
            .ok()
            .map(|project| project.manifest);
    ExtensionSummary {
        id: registration.id.to_string(),
        name: registration.prefix.name.as_str().to_owned(),
        title: manifest
            .as_ref()
            .map_or_else(|| registration.title.clone(), |m| m.title().to_owned()),
        description: manifest.as_ref().and_then(|m| m.description.clone()),
        version: manifest.as_ref().map(|m| m.version.to_string()),
        publisher_fingerprint: publisher_fingerprint(registration.prefix.public_key.as_str()),
        enabled: true,
        state: "installed".to_owned(),
        kept_data_bytes: None,
        single_instance: manifest.as_ref().is_some_and(|m| m.single_instance),
        has_icon: false,
        status_here: None,
        status_error_here: None,
        devices: Vec::new(),
        dev: true,
    }
}

/// The icon of the effective bundle as a `data:` URL, or `None` without an image icon or while
/// its file is still transferring.
pub fn icon_data_url(q: &mut impl Query, extension_id: Uuid) -> Result<Option<String>> {
    let Some((bundle_id, manifest)) = effective_manifest(q, extension_id)? else {
        return Ok(None);
    };
    let Some(icon) = manifest.icon.filter(|path| mime::is_image(path)) else {
        return Ok(None);
    };
    let Ok(Some(data)) = read_verified_file(q, bundle_id, &icon) else {
        return Ok(None);
    };
    Ok(Some(format!(
        "data:{};base64,{}",
        mime::for_path(&icon),
        base64::engine::general_purpose::STANDARD.encode(data)
    )))
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod list_tests;
