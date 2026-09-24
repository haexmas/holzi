//! Typed storage for the `workspaces` table (spec 015-workspace-shell,
//! [data-model.md](../../../specs/015-workspace-shell/data-model.md)).
//!
//! No name column — the frontend derives "Workspace N" from `position`
//! (FR-019), which stays dense (0…n−1) per device across creates and
//! deletes. Deleting a workspace also removes its `shell_windows` and
//! their `shell_window_tabs` rows via the FK cascade installed by
//! migration `0019_shell_layout` (I7; verified empirically in
//! `identity::migrations_tests`), so this module never touches those
//! tables itself.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, Result};
use uuid::Uuid;

/// One row of the `workspaces` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceRow {
    pub workspace_id: Uuid,
    pub position: i64,
}

/// Domain failures [`delete`] can report on top of a raw SQL error.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceDeleteError {
    #[error(transparent)]
    Sql(#[from] haex_crdt::rusqlite::Error),
    #[error("workspace not found")]
    NotFound,
    #[error("the last remaining workspace cannot be deleted")]
    LastWorkspace,
}

fn parse_workspace_id(raw: String) -> Result<Uuid> {
    Uuid::parse_str(&raw).map_err(|e| {
        haex_crdt::rusqlite::Error::FromSqlConversionFailure(
            0,
            haex_crdt::rusqlite::types::Type::Text,
            Box::new(e),
        )
    })
}

/// Lists every workspace for this device, ordered by `position` (I2).
pub fn list(conn: &Connection, vault_device_uuid: Uuid) -> Result<Vec<WorkspaceRow>> {
    let mut stmt = conn.prepare(
        "SELECT workspace_id, position FROM workspaces \
         WHERE vault_device_uuid = ?1 ORDER BY position ASC",
    )?;
    let rows = stmt.query_map(params![vault_device_uuid.to_string()], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    rows.map(|row| {
        let (id, position) = row?;
        Ok(WorkspaceRow {
            workspace_id: parse_workspace_id(id)?,
            position,
        })
    })
    .collect()
}

/// Appends a new workspace at the end (`position` = current count) with a
/// freshly minted `workspace_id` (data-model.md: "vom Backend vergeben").
/// Does not change the device's active workspace — the caller switches to
/// it explicitly (contract `shell_create_workspace`).
pub fn create(conn: &Connection, vault_device_uuid: Uuid) -> Result<WorkspaceRow> {
    let position: i64 = conn.query_row(
        "SELECT COUNT(*) FROM workspaces WHERE vault_device_uuid = ?1",
        params![vault_device_uuid.to_string()],
        |r| r.get(0),
    )?;
    let workspace_id = Uuid::new_v4();
    let sql = format!(
        "INSERT INTO workspaces (vault_device_uuid, workspace_id, position, {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, current_hlc())"
    );
    conn.execute(
        &sql,
        params![
            vault_device_uuid.to_string(),
            workspace_id.to_string(),
            position
        ],
    )?;
    Ok(WorkspaceRow {
        workspace_id,
        position,
    })
}

/// Ensures at least one workspace exists for this device (I1), creating a
/// default one if none do. Returns the up-to-date, `position`-ordered list
/// either way — used by `shell_load_layout`.
pub fn ensure_default(conn: &Connection, vault_device_uuid: Uuid) -> Result<Vec<WorkspaceRow>> {
    let existing = list(conn, vault_device_uuid)?;
    if !existing.is_empty() {
        return Ok(existing);
    }
    Ok(vec![create(conn, vault_device_uuid)?])
}

/// Deletes a workspace and densely renumbers the remaining ones (I2) in one
/// transaction. Refuses to delete the last remaining workspace (I3) or an
/// unknown one. Returns the surviving, renumbered, `position`-ordered list —
/// the caller (contract `shell_delete_workspace`) picks the new active
/// workspace from it and writes that to `preferences`.
pub fn delete(
    conn: &Connection,
    vault_device_uuid: Uuid,
    workspace_id: Uuid,
) -> std::result::Result<Vec<WorkspaceRow>, WorkspaceDeleteError> {
    let tx = conn.unchecked_transaction()?;

    let existing = list(&tx, vault_device_uuid)?;
    if !existing.iter().any(|w| w.workspace_id == workspace_id) {
        return Err(WorkspaceDeleteError::NotFound);
    }
    if existing.len() <= 1 {
        return Err(WorkspaceDeleteError::LastWorkspace);
    }

    tx.execute(
        "DELETE FROM workspaces WHERE vault_device_uuid = ?1 AND workspace_id = ?2",
        params![vault_device_uuid.to_string(), workspace_id.to_string()],
    )?;

    let update_sql = format!(
        "UPDATE workspaces SET position = ?1, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
         WHERE vault_device_uuid = ?2 AND workspace_id = ?3"
    );
    let mut renumbered = Vec::new();
    for (position, row) in existing
        .into_iter()
        .filter(|w| w.workspace_id != workspace_id)
        .enumerate()
    {
        let position = position as i64;
        if row.position != position {
            tx.execute(
                &update_sql,
                params![
                    position,
                    vault_device_uuid.to_string(),
                    row.workspace_id.to_string()
                ],
            )?;
        }
        renumbered.push(WorkspaceRow {
            workspace_id: row.workspace_id,
            position,
        });
    }

    tx.commit()?;
    Ok(renumbered)
}
