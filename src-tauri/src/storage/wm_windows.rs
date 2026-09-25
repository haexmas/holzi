//! Typed storage for `shell_windows` and `shell_window_tabs` (spec
//! 015-workspace-shell, [data-model.md](../../../specs/015-workspace-shell/data-model.md),
//! [contracts/tauri-commands.md](../../../specs/015-workspace-shell/contracts/tauri-commands.md)
//! `wm_save_windows` / `wm_close_windows`).
//!
//! `save_batch` is all-or-nothing (I6: a window's tab set is replaced
//! wholesale) and validates I4/I5/I9 itself, inside its own transaction, so
//! a caller only needs to resolve the device and forward the parsed batch.
//! Tab replacement upserts kept ids and deletes only genuinely removed ones
//! — never a blind delete-then-reinsert of the whole set — mirroring
//! `storage::models::replace_provider_models`'s reasoning: a delete and a
//! same-id insert in one transaction can land on the same `current_hlc()`,
//! and `delete_shadows_insert` then resolves that tie in favour of the
//! delete, silently dropping the "reinserted" row for any peer applying the
//! sync payload.

use std::collections::HashSet;

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result, Transaction};
use uuid::Uuid;

/// Maximum windows per device (I9) — abuse protection, not a UI rule.
const MAX_WINDOWS_PER_DEVICE: i64 = 500;
/// Tab-count bounds per window (I5).
const MIN_TABS_PER_WINDOW: usize = 1;
const MAX_TABS_PER_WINDOW: usize = 100;
const MAX_APP_ID_LEN: usize = 128;
const GEOMETRY_POSITION_BOUND: i64 = 100_000;
const GEOMETRY_SIZE_MAX: i64 = 100_000;

/// One tab to write as part of a window (position = index in the window's
/// `tabs` list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabWrite {
    pub tab_id: Uuid,
    pub app_id: String,
}

/// One window to insert or update, replacing its full tab set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowWrite {
    pub window_id: Uuid,
    pub workspace_id: Uuid,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub is_minimized: bool,
    pub is_maximized: bool,
    pub stack_order: i64,
    pub active_tab_id: Uuid,
    pub tabs: Vec<TabWrite>,
}

/// One persisted tab, in bar order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabRow {
    pub tab_id: Uuid,
    pub app_id: String,
}

/// One persisted window with its tabs already loaded, in bar order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRow {
    pub window_id: Uuid,
    pub workspace_id: Uuid,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub is_minimized: bool,
    pub is_maximized: bool,
    pub stack_order: i64,
    pub active_tab_id: Uuid,
    pub tabs: Vec<TabRow>,
}

/// Everything `save_batch` can refuse, on top of a raw SQL error.
#[derive(Debug, thiserror::Error)]
pub enum WmWindowError {
    #[error(transparent)]
    Sql(#[from] haex_crdt::rusqlite::Error),
    #[error("window must have between 1 and 100 tabs")]
    TabCountOutOfRange,
    #[error("activeTabId must reference one of the window's own tabs")]
    ActiveTabNotInWindow,
    #[error("duplicate tabId within the call or against another window")]
    DuplicateTabId,
    #[error("appId must be 1-128 characters with no control characters")]
    InvalidAppId,
    #[error("geometry is outside the allowed bounds")]
    GeometryOutOfBounds,
    #[error("workspace not found for this device")]
    UnknownWorkspace,
    #[error("device window limit ({MAX_WINDOWS_PER_DEVICE}) exceeded")]
    TooManyWindows,
}

/// Validates everything that does not need a database round trip (I5's
/// shape rules, appId format, geometry bounds, and in-batch tab-id
/// uniqueness). `save_batch` runs this before opening its transaction;
/// device-scoped checks (I4, cross-window tab ownership, I9) need the
/// database and live there instead.
fn validate_shape(windows: &[WindowWrite]) -> std::result::Result<(), WmWindowError> {
    let mut seen_tab_ids = HashSet::new();
    for w in windows {
        if w.tabs.len() < MIN_TABS_PER_WINDOW || w.tabs.len() > MAX_TABS_PER_WINDOW {
            return Err(WmWindowError::TabCountOutOfRange);
        }
        if !w.tabs.iter().any(|t| t.tab_id == w.active_tab_id) {
            return Err(WmWindowError::ActiveTabNotInWindow);
        }
        let x_ok = w.x.abs() <= GEOMETRY_POSITION_BOUND;
        let y_ok = w.y.abs() <= GEOMETRY_POSITION_BOUND;
        let width_ok = (1..=GEOMETRY_SIZE_MAX).contains(&w.width);
        let height_ok = (1..=GEOMETRY_SIZE_MAX).contains(&w.height);
        if !(x_ok && y_ok && width_ok && height_ok) {
            return Err(WmWindowError::GeometryOutOfBounds);
        }
        for t in &w.tabs {
            let app_id_ok = !t.app_id.is_empty()
                && t.app_id.chars().count() <= MAX_APP_ID_LEN
                && !t.app_id.chars().any(|c| c.is_control());
            if !app_id_ok {
                return Err(WmWindowError::InvalidAppId);
            }
            if !seen_tab_ids.insert(t.tab_id) {
                return Err(WmWindowError::DuplicateTabId);
            }
        }
    }
    Ok(())
}

fn upsert_window(tx: &Transaction, vault_device_uuid: Uuid, w: &WindowWrite) -> Result<()> {
    let device = vault_device_uuid.to_string();
    let window_id = w.window_id.to_string();
    let exists: i64 = tx.query_row(
        "SELECT COUNT(*) FROM shell_windows WHERE vault_device_uuid = ?1 AND window_id = ?2",
        params![device, window_id],
        |r| r.get(0),
    )?;
    if exists > 0 {
        let sql = format!(
            "UPDATE shell_windows \
             SET workspace_id = ?1, x = ?2, y = ?3, width = ?4, height = ?5, \
                 is_minimized = ?6, is_maximized = ?7, stack_order = ?8, active_tab_id = ?9, \
                 {HLC_TIMESTAMP_COLUMN} = current_hlc() \
             WHERE vault_device_uuid = ?10 AND window_id = ?11"
        );
        tx.execute(
            &sql,
            params![
                w.workspace_id.to_string(),
                w.x,
                w.y,
                w.width,
                w.height,
                w.is_minimized as i64,
                w.is_maximized as i64,
                w.stack_order,
                w.active_tab_id.to_string(),
                device,
                window_id,
            ],
        )?;
    } else {
        let sql = format!(
            "INSERT INTO shell_windows \
             (vault_device_uuid, window_id, workspace_id, x, y, width, height, \
              is_minimized, is_maximized, stack_order, active_tab_id, {HLC_TIMESTAMP_COLUMN}) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, current_hlc())"
        );
        tx.execute(
            &sql,
            params![
                device,
                window_id,
                w.workspace_id.to_string(),
                w.x,
                w.y,
                w.width,
                w.height,
                w.is_minimized as i64,
                w.is_maximized as i64,
                w.stack_order,
                w.active_tab_id.to_string(),
            ],
        )?;
    }
    Ok(())
}

/// Replaces a window's tab set (I6): upserts every tab in `w.tabs` (kept
/// ids are updated in place, never deleted-then-reinserted) and deletes
/// only the ids that are no longer present.
fn replace_tabs(tx: &Transaction, vault_device_uuid: Uuid, w: &WindowWrite) -> Result<()> {
    let device = vault_device_uuid.to_string();
    let window_id = w.window_id.to_string();

    let existing: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT tab_id FROM shell_window_tabs \
             WHERE vault_device_uuid = ?1 AND window_id = ?2",
        )?;
        let rows = stmt.query_map(params![device, window_id], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>>>()?
    };
    let target_ids: HashSet<String> = w.tabs.iter().map(|t| t.tab_id.to_string()).collect();

    for (position, t) in w.tabs.iter().enumerate() {
        let tab_id = t.tab_id.to_string();
        let position = position as i64;
        if existing.contains(&tab_id) {
            let sql = format!(
                "UPDATE shell_window_tabs \
                 SET window_id = ?1, app_id = ?2, position = ?3, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
                 WHERE vault_device_uuid = ?4 AND tab_id = ?5"
            );
            tx.execute(&sql, params![window_id, t.app_id, position, device, tab_id])?;
        } else {
            let sql = format!(
                "INSERT INTO shell_window_tabs \
                 (vault_device_uuid, tab_id, window_id, app_id, position, {HLC_TIMESTAMP_COLUMN}) \
                 VALUES (?1, ?2, ?3, ?4, ?5, current_hlc())"
            );
            tx.execute(&sql, params![device, tab_id, window_id, t.app_id, position])?;
        }
    }

    for tab_id in &existing {
        if !target_ids.contains(tab_id) {
            tx.execute(
                "DELETE FROM shell_window_tabs WHERE vault_device_uuid = ?1 AND tab_id = ?2",
                params![device, tab_id],
            )?;
        }
    }
    Ok(())
}

/// Saves a batch of windows (with their tabs) in one transaction —
/// all-or-nothing (contract `wm_save_windows`). Validates I4 (workspace
/// exists for this device), I5 (tab count, `activeTabId` membership, appId
/// shape, tab-id uniqueness in-call and against other windows) and I9 (≤500
/// windows/device) before writing anything.
pub fn save_batch(
    conn: &Connection,
    vault_device_uuid: Uuid,
    windows: &[WindowWrite],
) -> std::result::Result<(), WmWindowError> {
    validate_shape(windows)?;

    let tx = conn.unchecked_transaction()?;
    let device = vault_device_uuid.to_string();

    for w in windows {
        let workspace_exists: i64 = tx.query_row(
            "SELECT COUNT(*) FROM workspaces WHERE vault_device_uuid = ?1 AND workspace_id = ?2",
            params![device, w.workspace_id.to_string()],
            |r| r.get(0),
        )?;
        if workspace_exists == 0 {
            return Err(WmWindowError::UnknownWorkspace);
        }
    }

    for w in windows {
        let window_id = w.window_id.to_string();
        for t in &w.tabs {
            let owner: Option<String> = tx
                .query_row(
                    "SELECT window_id FROM shell_window_tabs \
                     WHERE vault_device_uuid = ?1 AND tab_id = ?2",
                    params![device, t.tab_id.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(owner) = owner {
                if owner != window_id {
                    return Err(WmWindowError::DuplicateTabId);
                }
            }
        }
    }

    let existing_window_ids: HashSet<String> = {
        let mut stmt =
            tx.prepare("SELECT window_id FROM shell_windows WHERE vault_device_uuid = ?1")?;
        let rows = stmt.query_map(params![device], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<HashSet<_>>>()?
    };
    let current_total = existing_window_ids.len() as i64;
    let incoming_new = windows
        .iter()
        .filter(|w| !existing_window_ids.contains(&w.window_id.to_string()))
        .count() as i64;
    if current_total + incoming_new > MAX_WINDOWS_PER_DEVICE {
        return Err(WmWindowError::TooManyWindows);
    }

    for w in windows {
        upsert_window(&tx, vault_device_uuid, w)?;
        replace_tabs(&tx, vault_device_uuid, w)?;
    }

    tx.commit()?;
    Ok(())
}

/// Deletes windows (and, via the migration `0019` FK cascade, their tabs).
/// Idempotent — unknown ids are silently ignored (contract
/// `wm_close_windows`).
pub fn close_windows(
    conn: &Connection,
    vault_device_uuid: Uuid,
    window_ids: &[Uuid],
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let device = vault_device_uuid.to_string();
    for id in window_ids {
        tx.execute(
            "DELETE FROM shell_windows WHERE vault_device_uuid = ?1 AND window_id = ?2",
            params![device, id.to_string()],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn parse_uuid(raw: String) -> Result<Uuid> {
    Uuid::parse_str(&raw).map_err(|e| {
        haex_crdt::rusqlite::Error::FromSqlConversionFailure(
            0,
            haex_crdt::rusqlite::types::Type::Text,
            Box::new(e),
        )
    })
}

/// Loads every window for this device with its tabs, ordered by
/// `stack_order` ascending (contract `wm_load_layout`'s
/// `windows` — "nach stackOrder aufsteigend"). A window with an unknown
/// `workspace_id` (excluded by the join) or zero tabs (a transient sync
/// race, data-model.md I5) is not delivered — cleanup is `hydrate`'s job on
/// the frontend, this just never sends it something incomplete.
pub fn load_all(conn: &Connection, vault_device_uuid: Uuid) -> Result<Vec<WindowRow>> {
    let mut stmt = conn.prepare(
        "SELECT sw.window_id, sw.workspace_id, sw.x, sw.y, sw.width, sw.height, \
                sw.is_minimized, sw.is_maximized, sw.stack_order, sw.active_tab_id \
         FROM shell_windows sw \
         INNER JOIN workspaces w \
           ON w.vault_device_uuid = sw.vault_device_uuid \
          AND w.workspace_id = sw.workspace_id \
         WHERE sw.vault_device_uuid = ?1 \
         ORDER BY sw.stack_order ASC",
    )?;
    let device = vault_device_uuid.to_string();
    let rows = stmt.query_map(params![device], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, i64>(8)?,
            r.get::<_, String>(9)?,
        ))
    })?;

    let mut windows = Vec::new();
    for row in rows {
        let (
            window_id,
            workspace_id,
            x,
            y,
            width,
            height,
            is_minimized,
            is_maximized,
            stack_order,
            active_tab_id,
        ) = row?;
        let window_id = parse_uuid(window_id)?;

        let mut tab_stmt = conn.prepare(
            "SELECT tab_id, app_id FROM shell_window_tabs \
             WHERE vault_device_uuid = ?1 AND window_id = ?2 \
             ORDER BY position ASC",
        )?;
        let tab_rows = tab_stmt.query_map(
            params![vault_device_uuid.to_string(), window_id.to_string()],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )?;
        let mut tabs = Vec::new();
        for tab_row in tab_rows {
            let (tab_id, app_id) = tab_row?;
            tabs.push(TabRow {
                tab_id: parse_uuid(tab_id)?,
                app_id,
            });
        }
        if tabs.is_empty() {
            continue;
        }

        windows.push(WindowRow {
            window_id,
            workspace_id: parse_uuid(workspace_id)?,
            x,
            y,
            width,
            height,
            is_minimized: is_minimized != 0,
            is_maximized: is_maximized != 0,
            stack_order,
            active_tab_id: parse_uuid(active_tab_id)?,
            tabs,
        });
    }
    Ok(windows)
}
