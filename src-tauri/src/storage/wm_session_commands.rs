//! Tauri commands for the opt-in session restore (spec 022-session-restore,
//! [contracts/wm-session.md](../../../specs/022-session-restore/contracts/wm-session.md)).
//!
//! Each `#[tauri::command]` resolves the active database and the current
//! device, then delegates to a plain `async fn` that the tests call directly.
//! No command takes a device UUID from the frontend: a caller passing a
//! foreign one could otherwise read or delete another device's session.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::identity::{installation_id_path, read_or_mint_installation_uuid};
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::known_devices;
use crate::storage::preferences::{self, PrefScope, ScopedBool};
use crate::storage::wm_session::{self, WmSessionError};
use crate::vault_gate::VaultDb;

/// Preference key of the setting "Sitzung wiederherstellen" (data-model.md).
pub const SESSION_RESTORE_KEY: &str = "wm.session_restore";

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// Device value, vault value and the value that applies on this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SessionRestoreState {
    pub device: Option<bool>,
    pub vault: Option<bool>,
    pub effective: bool,
}

impl From<ScopedBool> for SessionRestoreState {
    fn from(scoped: ScopedBool) -> Self {
        Self {
            device: scoped.device,
            vault: scoped.vault,
            effective: scoped.effective(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum SessionRestoreScope {
    Device,
    Vault,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SessionRestoreSetArgs {
    pub scope: SessionRestoreScope,
    /// `null` resets the value in `scope`.
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct WmSessionLoad {
    pub restore: SessionRestoreState,
    /// The saved session as the frontend wrote it, validated there.
    #[ts(type = "unknown")]
    pub session: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct WmSessionSaveArgs {
    #[ts(type = "unknown")]
    pub session: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct WmSessionSaved {
    pub saved: bool,
}

// ---------------------------------------------------------------------------
// Device resolution
// ---------------------------------------------------------------------------

/// Resolves the current device's `vault_device_uuid` from the installation
/// id, the same pair `current_device_info` uses.
fn current_device_uuid(app: &AppHandle, db: &VaultDb) -> Result<Uuid> {
    let installation_id_file =
        installation_id_path(&app.path().app_local_data_dir().map_err(|e| {
            HolziError::PathResolution {
                reason: format!("app_local_data_dir: {e}"),
            }
        })?);
    let installation_uuid =
        read_or_mint_installation_uuid(&installation_id_file).map_err(HolziError::from)?;
    db.with_connection(|conn| {
        Ok(known_devices::get_vault_device_uuid(
            conn,
            installation_uuid,
        )?)
    })
    .map_err(HolziError::from)?
    .ok_or_else(|| HolziError::InvalidInput {
        reason: "current device is not registered in known_devices".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Core logic (testable without a live Tauri State/AppHandle)
// ---------------------------------------------------------------------------

fn session_error_to_holzi(err: WmSessionError) -> HolziError {
    match err {
        WmSessionError::Sql(e) => HolziError::from(haex_crdt::Error::from(e)),
        WmSessionError::TooLarge { bytes } => HolziError::SessionTooLarge { bytes },
        other => HolziError::InvalidInput {
            reason: other.to_string(),
        },
    }
}

async fn blocking<T, F>(label: &'static str, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| HolziError::CrdtInit {
            reason: format!("{label} join: {e}"),
        })?
}

async fn restore_get(db: VaultDb, device: Uuid) -> Result<SessionRestoreState> {
    blocking("wm_session_restore_get", move || {
        let scoped = db
            .with_connection(|conn| {
                Ok(preferences::get_scoped_bool(
                    conn,
                    device,
                    SESSION_RESTORE_KEY,
                )?)
            })
            .map_err(HolziError::from)?;
        Ok(scoped.into())
    })
    .await
}

/// Writes or resets one scope's value and, in the same transaction, deletes
/// the device's saved session when restore no longer applies (FR-005,
/// FR-007): no save can slip in between turning it off and deleting.
async fn restore_set(
    db: VaultDb,
    device: Uuid,
    args: SessionRestoreSetArgs,
) -> Result<SessionRestoreState> {
    blocking("wm_session_restore_set", move || {
        let scope = match args.scope {
            SessionRestoreScope::Device => PrefScope::Device(device),
            SessionRestoreScope::Vault => PrefScope::Vault,
        };
        let scoped = db
            .with_connection(|conn| {
                let tx = conn.unchecked_transaction()?;
                match args.enabled {
                    Some(enabled) => {
                        let value = if enabled { "true" } else { "false" };
                        preferences::insert_or_update(&tx, scope, SESSION_RESTORE_KEY, value)?;
                    }
                    None => {
                        preferences::delete(&tx, scope, SESSION_RESTORE_KEY)?;
                    }
                }
                let scoped = preferences::get_scoped_bool(&tx, device, SESSION_RESTORE_KEY)?;
                if !scoped.effective() {
                    wm_session::delete(&tx, device)?;
                }
                tx.commit()?;
                Ok(scoped)
            })
            .map_err(HolziError::from)?;
        Ok(scoped.into())
    })
    .await
}

/// Returns the setting and, when it applies, the saved session. When it
/// does not apply, a leftover session is deleted first (FR-008, FR-012).
async fn session_load(db: VaultDb, device: Uuid) -> Result<WmSessionLoad> {
    blocking("wm_session_load", move || {
        let result = db
            .with_connection(|conn| {
                let scoped = preferences::get_scoped_bool(conn, device, SESSION_RESTORE_KEY)?;
                if !scoped.effective() {
                    wm_session::delete(conn, device)?;
                    return Ok(Ok((scoped, None)));
                }
                Ok(wm_session::load(conn, device).map(|session| (scoped, session)))
            })
            .map_err(HolziError::from)?;
        let (scoped, session) = result.map_err(session_error_to_holzi)?;
        Ok(WmSessionLoad {
            restore: scoped.into(),
            session,
        })
    })
    .await
}

/// Writes the session only while restore applies, so a late debounced save
/// after turning it off creates nothing.
async fn session_save(db: VaultDb, device: Uuid, session: Value) -> Result<WmSessionSaved> {
    blocking("wm_session_save", move || {
        let result = db
            .with_connection(|conn| {
                let scoped = preferences::get_scoped_bool(conn, device, SESSION_RESTORE_KEY)?;
                if !scoped.effective() {
                    return Ok(Ok(false));
                }
                Ok(wm_session::save(conn, device, &session).map(|()| true))
            })
            .map_err(HolziError::from)?;
        let saved = result.map_err(session_error_to_holzi)?;
        Ok(WmSessionSaved { saved })
    })
    .await
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn wm_session_restore_get(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SessionRestoreState> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    restore_get(db, device).await
}

#[tauri::command]
pub async fn wm_session_restore_set(
    app: AppHandle,
    state: State<'_, AppState>,
    args: SessionRestoreSetArgs,
) -> Result<SessionRestoreState> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    restore_set(db, device, args).await
}

#[tauri::command]
pub async fn wm_session_load(app: AppHandle, state: State<'_, AppState>) -> Result<WmSessionLoad> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    session_load(db, device).await
}

#[tauri::command]
pub async fn wm_session_save(
    app: AppHandle,
    state: State<'_, AppState>,
    args: WmSessionSaveArgs,
) -> Result<WmSessionSaved> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    session_save(db, device, args.session).await
}

#[cfg(test)]
#[path = "wm_session_commands_tests.rs"]
mod tests;
