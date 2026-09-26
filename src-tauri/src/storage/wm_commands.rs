//! Tauri command surface for the window manager layout (spec 015-workspace-shell,
//! [contracts/tauri-commands.md](../../../specs/015-workspace-shell/contracts/tauri-commands.md)).
//!
//! Each `#[tauri::command]` is a thin shim: resolve the active database and
//! the current device, then delegate to a plain `async fn` that takes both
//! directly (mirroring `models::commands::register_downloaded`) — that
//! keeps the actual logic callable from `wm_commands_tests.rs` without
//! a live Tauri `AppHandle`/`State`.
//!
//! No command takes a device UUID from the frontend (research R4): each
//! resolves it itself via `read_or_mint_installation_uuid` +
//! `known_devices::get_vault_device_uuid`, the same pair
//! `current_device_info` uses.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::identity::{installation_id_path, read_or_mint_installation_uuid};
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::known_devices;
use crate::storage::preferences::{self, PrefScope};
use crate::storage::wm_windows::{self, TabWrite, WindowRow, WindowWrite, WmWindowError};
use crate::storage::wm_workspaces::{self, WorkspaceDeleteError, WorkspaceRow};
use crate::vault_gate::VaultDb;

/// Device-scoped `preferences` key holding the active workspace id
/// (data-model.md).
const ACTIVE_WORKSPACE_PREF_KEY: &str = "shell.active_workspace_id";

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDto {
    pub workspace_id: Uuid,
    #[ts(type = "number")]
    pub position: i64,
}

impl From<WorkspaceRow> for WorkspaceDto {
    fn from(row: WorkspaceRow) -> Self {
        WorkspaceDto {
            workspace_id: row.workspace_id,
            position: row.position,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct TabDto {
    pub tab_id: Uuid,
    pub app_id: String,
}

impl From<wm_windows::TabRow> for TabDto {
    fn from(row: wm_windows::TabRow) -> Self {
        TabDto {
            tab_id: row.tab_id,
            app_id: row.app_id,
        }
    }
}

impl From<TabDto> for TabWrite {
    fn from(dto: TabDto) -> Self {
        TabWrite {
            tab_id: dto.tab_id,
            app_id: dto.app_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct WindowDto {
    pub window_id: Uuid,
    pub workspace_id: Uuid,
    #[ts(type = "number")]
    pub x: i64,
    #[ts(type = "number")]
    pub y: i64,
    #[ts(type = "number")]
    pub width: i64,
    #[ts(type = "number")]
    pub height: i64,
    pub is_minimized: bool,
    pub is_maximized: bool,
    #[ts(type = "number")]
    pub stack_order: i64,
    pub active_tab_id: Uuid,
    pub tabs: Vec<TabDto>,
}

impl From<WindowRow> for WindowDto {
    fn from(row: WindowRow) -> Self {
        WindowDto {
            window_id: row.window_id,
            workspace_id: row.workspace_id,
            x: row.x,
            y: row.y,
            width: row.width,
            height: row.height,
            is_minimized: row.is_minimized,
            is_maximized: row.is_maximized,
            stack_order: row.stack_order,
            active_tab_id: row.active_tab_id,
            tabs: row.tabs.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<WindowDto> for WindowWrite {
    fn from(dto: WindowDto) -> Self {
        WindowWrite {
            window_id: dto.window_id,
            workspace_id: dto.workspace_id,
            x: dto.x,
            y: dto.y,
            width: dto.width,
            height: dto.height,
            is_minimized: dto.is_minimized,
            is_maximized: dto.is_maximized,
            stack_order: dto.stack_order,
            active_tab_id: dto.active_tab_id,
            tabs: dto.tabs.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct WmLayoutDto {
    pub workspaces: Vec<WorkspaceDto>,
    pub windows: Vec<WindowDto>,
    pub active_workspace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct DeleteWorkspaceResult {
    pub workspaces: Vec<WorkspaceDto>,
    pub active_workspace_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdArgs {
    pub workspace_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWindowsArgs {
    pub windows: Vec<WindowDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseWindowsArgs {
    pub window_ids: Vec<Uuid>,
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

fn workspace_delete_error_to_holzi(err: WorkspaceDeleteError) -> HolziError {
    let reason = err.to_string();
    match err {
        WorkspaceDeleteError::Sql(e) => HolziError::from(haex_crdt::Error::from(e)),
        WorkspaceDeleteError::NotFound | WorkspaceDeleteError::LastWorkspace => {
            HolziError::InvalidInput { reason }
        }
    }
}

fn wm_window_error_to_holzi(err: WmWindowError) -> HolziError {
    let reason = err.to_string();
    match err {
        WmWindowError::Sql(e) => HolziError::from(haex_crdt::Error::from(e)),
        _ => HolziError::InvalidInput { reason },
    }
}

// ---------------------------------------------------------------------------
// Device resolution (research R4)
// ---------------------------------------------------------------------------

/// Resolves the current device's `vault_device_uuid`, the same pair
/// `current_device_info` uses. No command accepts this from the frontend —
/// a malicious or buggy caller passing a foreign device UUID could
/// otherwise read or write another device's layout (FR-024).
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

async fn load_layout(db: VaultDb, device: Uuid) -> Result<WmLayoutDto> {
    tauri::async_runtime::spawn_blocking(move || -> Result<WmLayoutDto> {
        let workspace_rows = db
            .with_connection(|conn| Ok(wm_workspaces::ensure_default(conn, device)?))
            .map_err(HolziError::from)?;
        let window_rows = db
            .with_connection(|conn| Ok(wm_windows::load_all(conn, device)?))
            .map_err(HolziError::from)?;

        let scope = PrefScope::Device(device);
        let stored = db
            .with_connection(|conn| {
                preferences::get(conn, scope, ACTIVE_WORKSPACE_PREF_KEY).map_err(Into::into)
            })
            .map_err(HolziError::from)?;

        let resolved = stored
            .as_deref()
            .and_then(|s| Uuid::parse_str(s).ok())
            .filter(|id| workspace_rows.iter().any(|w| w.workspace_id == *id));
        let active_workspace_id = match resolved {
            Some(id) => id,
            None => {
                let fallback = workspace_rows[0].workspace_id;
                db.with_connection(|conn| {
                    preferences::insert_or_update(
                        conn,
                        scope,
                        ACTIVE_WORKSPACE_PREF_KEY,
                        &fallback.to_string(),
                    )
                    .map(|_| ())
                    .map_err(Into::into)
                })
                .map_err(HolziError::from)?;
                fallback
            }
        };

        Ok(WmLayoutDto {
            workspaces: workspace_rows.into_iter().map(Into::into).collect(),
            windows: window_rows.into_iter().map(Into::into).collect(),
            active_workspace_id,
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("wm_load_layout join: {e}"),
    })?
}

async fn create_workspace(db: VaultDb, device: Uuid) -> Result<WorkspaceDto> {
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| Ok(wm_workspaces::create(conn, device)?))
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("wm_create_workspace join: {e}"),
    })?
    .map_err(HolziError::from)
    .map(Into::into)
}

/// Picks the workspace that becomes active after deleting the one at
/// `deleted_index` in the pre-delete, position-ordered list: the previous
/// neighbor, or the next one (now shifted into that same index) if the
/// deleted workspace was first (contract `wm_delete_workspace`). Mirrors
/// the frontend reducer's `Math.max(0, index - 1)` (`layoutState.ts`).
fn neighbor_after_delete(deleted_index: usize, remaining: &[WorkspaceRow]) -> Option<Uuid> {
    let target_index = deleted_index.saturating_sub(1);
    remaining.get(target_index).map(|w| w.workspace_id)
}

async fn delete_workspace(
    db: VaultDb,
    device: Uuid,
    workspace_id: Uuid,
) -> Result<DeleteWorkspaceResult> {
    tauri::async_runtime::spawn_blocking(move || -> Result<DeleteWorkspaceResult> {
        let scope = PrefScope::Device(device);
        let previously_active = db
            .with_connection(|conn| {
                preferences::get(conn, scope, ACTIVE_WORKSPACE_PREF_KEY).map_err(Into::into)
            })
            .map_err(HolziError::from)?
            .and_then(|s| Uuid::parse_str(&s).ok());

        let before = db
            .with_connection(|conn| Ok(wm_workspaces::list(conn, device)?))
            .map_err(HolziError::from)?;
        let deleted_index = before.iter().position(|w| w.workspace_id == workspace_id);

        let remaining = db
            .with_connection(|conn| Ok(wm_workspaces::delete(conn, device, workspace_id)))
            .map_err(HolziError::from)?
            .map_err(workspace_delete_error_to_holzi)?;

        let active_workspace_id = if previously_active == Some(workspace_id) {
            let candidate = deleted_index
                .and_then(|i| neighbor_after_delete(i, &remaining))
                .or_else(|| remaining.first().map(|w| w.workspace_id))
                .expect("delete() guarantees at least one remaining workspace");
            db.with_connection(|conn| {
                preferences::insert_or_update(
                    conn,
                    scope,
                    ACTIVE_WORKSPACE_PREF_KEY,
                    &candidate.to_string(),
                )
                .map(|_| ())
                .map_err(Into::into)
            })
            .map_err(HolziError::from)?;
            candidate
        } else {
            previously_active
                .filter(|id| remaining.iter().any(|w| w.workspace_id == *id))
                .unwrap_or_else(|| remaining[0].workspace_id)
        };

        Ok(DeleteWorkspaceResult {
            workspaces: remaining.into_iter().map(Into::into).collect(),
            active_workspace_id,
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("wm_delete_workspace join: {e}"),
    })?
}

async fn set_active_workspace(db: VaultDb, device: Uuid, workspace_id: Uuid) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        let workspaces = db
            .with_connection(|conn| Ok(wm_workspaces::list(conn, device)?))
            .map_err(HolziError::from)?;
        if !workspaces.iter().any(|w| w.workspace_id == workspace_id) {
            return Err(HolziError::InvalidInput {
                reason: "unknown workspaceId".to_string(),
            });
        }
        let scope = PrefScope::Device(device);
        db.with_connection(|conn| {
            preferences::insert_or_update(
                conn,
                scope,
                ACTIVE_WORKSPACE_PREF_KEY,
                &workspace_id.to_string(),
            )
            .map(|_| ())
            .map_err(Into::into)
        })
        .map_err(HolziError::from)
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("wm_set_active_workspace join: {e}"),
    })?
}

async fn save_windows(db: VaultDb, device: Uuid, windows: Vec<WindowWrite>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        db.with_connection(|conn| Ok(wm_windows::save_batch(conn, device, &windows)))
            .map_err(HolziError::from)?
            .map_err(wm_window_error_to_holzi)
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("wm_save_windows join: {e}"),
    })?
}

async fn close_windows_batch(db: VaultDb, device: Uuid, window_ids: Vec<Uuid>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        db.with_connection(|conn| Ok(wm_windows::close_windows(conn, device, &window_ids)?))
            .map_err(HolziError::from)
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("wm_close_windows join: {e}"),
    })?
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn wm_load_layout(app: AppHandle, state: State<'_, AppState>) -> Result<WmLayoutDto> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    load_layout(db, device).await
}

#[tauri::command]
pub async fn wm_create_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspaceDto> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    create_workspace(db, device).await
}

#[tauri::command]
pub async fn wm_delete_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    args: WorkspaceIdArgs,
) -> Result<DeleteWorkspaceResult> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    delete_workspace(db, device, args.workspace_id).await
}

#[tauri::command]
pub async fn wm_set_active_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    args: WorkspaceIdArgs,
) -> Result<()> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    set_active_workspace(db, device, args.workspace_id).await
}

#[tauri::command]
pub async fn wm_save_windows(
    app: AppHandle,
    state: State<'_, AppState>,
    args: SaveWindowsArgs,
) -> Result<()> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    let windows: Vec<WindowWrite> = args.windows.into_iter().map(Into::into).collect();
    save_windows(db, device, windows).await
}

#[tauri::command]
pub async fn wm_close_windows(
    app: AppHandle,
    state: State<'_, AppState>,
    args: CloseWindowsArgs,
) -> Result<()> {
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    close_windows_batch(db, device, args.window_ids).await
}

#[cfg(test)]
#[path = "wm_commands_tests.rs"]
mod tests;
