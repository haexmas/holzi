//! Tauri commands for the opt-in session restore (spec 022-session-restore,
//! [contracts/wm-session.md](../../../specs/022-session-restore/contracts/wm-session.md)).
//! Since spec 023 (FR-024) the setting applies to the whole vault: one value,
//! on or off; the saved session itself stays per device.
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
use crate::storage::preferences::{self, PrefScope};
use crate::storage::wm_session::{self, WmSessionError};
use crate::vault_gate::VaultDb;

/// Preference key of the setting "Sitzung wiederherstellen" (data-model.md).
pub const SESSION_RESTORE_KEY: &str = "wm.session_restore";

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// Whether the session is saved and restored, for every device of the vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SessionRestoreState {
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SessionRestoreSetArgs {
    pub enabled: bool,
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

/// The vault value; unset or unreadable means off (spec 022 FR-001).
fn restore_enabled(conn: &haex_crdt::rusqlite::Connection) -> haex_crdt::Result<bool> {
    let value = preferences::get(conn, PrefScope::Vault, SESSION_RESTORE_KEY)?;
    Ok(preferences::parse_bool(value.as_deref()).unwrap_or(false))
}

async fn restore_get(db: VaultDb) -> Result<SessionRestoreState> {
    blocking("wm_session_restore_get", move || {
        let enabled = db
            .with_connection(restore_enabled)
            .map_err(HolziError::from)?;
        Ok(SessionRestoreState { enabled })
    })
    .await
}

/// Writes the vault value and, in the same transaction, deletes this
/// device's saved session when turned off (FR-005, FR-007): no save can slip
/// in between. Other devices delete theirs on their next load (FR-008).
async fn restore_set(
    db: VaultDb,
    device: Uuid,
    args: SessionRestoreSetArgs,
) -> Result<SessionRestoreState> {
    blocking("wm_session_restore_set", move || {
        db.with_connection(|conn| {
            let tx = conn.unchecked_transaction()?;
            let value = if args.enabled { "true" } else { "false" };
            preferences::insert_or_update(&tx, PrefScope::Vault, SESSION_RESTORE_KEY, value)?;
            if !args.enabled {
                wm_session::delete(&tx, device)?;
            }
            tx.commit()?;
            Ok(())
        })
        .map_err(HolziError::from)?;
        Ok(SessionRestoreState {
            enabled: args.enabled,
        })
    })
    .await
}

/// Returns the setting and, when it applies, the saved session. When it
/// does not apply, a leftover session is deleted first (FR-008, FR-012).
async fn session_load(db: VaultDb, device: Uuid) -> Result<WmSessionLoad> {
    blocking("wm_session_load", move || {
        let result = db
            .with_connection(|conn| {
                if !restore_enabled(conn)? {
                    wm_session::delete(conn, device)?;
                    return Ok(Ok((false, None)));
                }
                Ok(wm_session::load(conn, device).map(|session| (true, session)))
            })
            .map_err(HolziError::from)?;
        let (enabled, session) = result.map_err(session_error_to_holzi)?;
        Ok(WmSessionLoad {
            restore: SessionRestoreState { enabled },
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
                if !restore_enabled(conn)? {
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
pub async fn wm_session_restore_get(state: State<'_, AppState>) -> Result<SessionRestoreState> {
    restore_get(active_database(&state)?).await
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
