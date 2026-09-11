//! `current_device_info` and `update_device_alias` Tauri commands.
//!
//! See spec 002 [`contracts/tauri-commands.md`](../../../specs/002-onboarding-model-prefs/contracts/tauri-commands.md).
//! The device info command powers the onboarding-wizard's alias prefill,
//! the workspace-landing header, and the onboarded-middleware's
//! decision ("alias == null → route to /onboarding"). Alias-rename is
//! the settings-screen entry point and the wizard's final commit step.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::hardware::hostname;
use crate::identity::{installation_id_path, read_or_mint_installation_uuid, VAULT_SCOPE_UUID};
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::known_devices;

/// Frontend view of the active device's identity + OS hostname.
///
/// `alias` is `None` before the onboarding wizard's final commit (that
/// is what triggers the onboarded-middleware redirect). `hostname` is
/// the raw `Option<String>` from `sysinfo`; the frontend substitutes
/// its localised fallback when it comes back `None`
/// (`onboarding.alias.defaultPlaceholder`). Both are camelCased on the
/// wire per the module-wide serde convention.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfoPayload {
    pub installation_uuid: Uuid,
    pub vault_device_uuid: Uuid,
    pub alias: Option<String>,
    pub hostname: Option<String>,
}

/// Returns the current device's identity for the active vault.
///
/// Requires an open vault — the payload joins the app-local
/// installation-id file with the vault-tracked `known_devices` row.
/// Errors as `NoActiveInstance` when no vault is open.
#[tauri::command]
pub async fn current_device_info(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeviceInfoPayload> {
    let installation_id_file =
        installation_id_path(&app.path().app_local_data_dir().map_err(|e| {
            HolziError::PathResolution {
                reason: format!("app_local_data_dir: {e}"),
            }
        })?);
    let installation_uuid =
        read_or_mint_installation_uuid(&installation_id_file).map_err(HolziError::from)?;

    let db = active_database(&state)?;
    let installation_for_query = installation_uuid;
    let row = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            use haex_crdt::rusqlite::{params, OptionalExtension};
            let raw: Option<(String, Option<String>)> = conn
                .query_row(
                    "SELECT vault_device_uuid, alias FROM known_devices \
                     WHERE installation_uuid = ?1",
                    params![installation_for_query.to_string()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(haex_crdt::Error::from)?;
            Ok(raw)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("current_device_info join: {e}"),
    })?
    .map_err(HolziError::from)?;

    let (vault_device_uuid, alias) = row.ok_or_else(|| HolziError::CrdtInit {
        reason: format!("no known_devices row for installation {installation_uuid}"),
    })?;
    let vault_device_uuid =
        Uuid::parse_str(&vault_device_uuid).map_err(|e| HolziError::CrdtInit {
            reason: format!("stored vault_device_uuid parse: {e}"),
        })?;
    if vault_device_uuid == VAULT_SCOPE_UUID {
        return Err(HolziError::CrdtInit {
            reason: "installation UUID resolved to the vault-scope sentinel".into(),
        });
    }

    Ok(DeviceInfoPayload {
        installation_uuid,
        vault_device_uuid,
        alias,
        hostname: hostname::suggested_alias(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeviceAliasArgs {
    pub alias: String,
}

/// Sets or renames the alias on the active device's `known_devices` row.
///
/// Whitespace-trimmed; a resulting empty string is rejected as
/// `InvalidInput` so the onboarding wizard's "required" contract holds
/// through the backend as well.
#[tauri::command]
pub async fn update_device_alias(
    app: AppHandle,
    state: State<'_, AppState>,
    args: UpdateDeviceAliasArgs,
) -> Result<()> {
    let trimmed = args.alias.trim().to_string();
    if trimmed.is_empty() {
        return Err(HolziError::InvalidInput {
            reason: "alias is empty or whitespace-only".into(),
        });
    }

    let installation_id_file =
        installation_id_path(&app.path().app_local_data_dir().map_err(|e| {
            HolziError::PathResolution {
                reason: format!("app_local_data_dir: {e}"),
            }
        })?);
    let installation_uuid =
        read_or_mint_installation_uuid(&installation_id_file).map_err(HolziError::from)?;

    let db = active_database(&state)?;
    let alias_owned = trimmed;
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            known_devices::update_alias(conn, installation_uuid, &alias_owned)
                .map_err(haex_crdt::Error::from)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("update_device_alias join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(())
}
