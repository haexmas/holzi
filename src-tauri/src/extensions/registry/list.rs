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
    /// `installed` or `removed` (data kept, L3).
    pub state: String,
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
}

struct Row {
    id: String,
    public_key: String,
    name: String,
    enabled: bool,
    state: String,
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
        "SELECT id, public_key, name, enabled, state FROM extensions",
        &[],
        |r| {
            Ok(Row {
                id: r.get(0)?,
                public_key: r.get(1)?,
                name: r.get(2)?,
                enabled: r.get::<_, i64>(3)? != 0,
                state: r.get(4)?,
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
        out.push(ExtensionSummary {
            title: manifest
                .as_ref()
                .map_or_else(|| row.name.clone(), |m| m.title().to_owned()),
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
            status_here,
            status_error_here,
            devices,
        });
    }
    out.sort_by_key(|e| e.title.to_lowercase());
    Ok(out)
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
