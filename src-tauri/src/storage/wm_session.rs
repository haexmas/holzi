//! Device-local storage of the window manager session (spec
//! 022-session-restore, research R1/R2): which workspaces, windows and tabs
//! are open, with each tab's back/forward history, as one JSON document per
//! device in `wm_sessions_no_sync`.
//!
//! The `_no_sync` suffix keeps the table out of CRDT sync completely: no
//! HLC columns, no triggers, no delete-log entries. A saved session never
//! reaches another device, and `delete` is a real local delete (FR-010).
//! The content is opaque here; the frontend validates it on load.

use haex_crdt::rusqlite::{self, params, Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

/// Upper bound for one serialized session. Typical sessions are a few KB;
/// the frontend drops the tab histories and retries once if it is exceeded.
pub const MAX_SESSION_BYTES: usize = 4 << 20;

#[derive(Debug, thiserror::Error)]
pub enum WmSessionError {
    #[error("the session must be a JSON object")]
    NotAnObject,
    #[error("the session is {bytes} bytes, more than the {MAX_SESSION_BYTES}-byte limit")]
    TooLarge { bytes: usize },
    #[error("the stored session is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
}

/// Returns the device's saved session, or `None` if there is none.
pub fn load(conn: &Connection, device: Uuid) -> Result<Option<Value>, WmSessionError> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT session_json FROM wm_sessions_no_sync WHERE vault_device_uuid = ?1",
            params![device.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    Ok(stored.map(|json| serde_json::from_str(&json)).transpose()?)
}

/// Writes the device's session, replacing any previous one.
///
/// `ON CONFLICT DO UPDATE` is fine here, unlike on CRDT tables
/// (`preferences::insert_or_update`): a `_no_sync` table has no triggers
/// whose dirty-table writes could collide.
pub fn save(conn: &Connection, device: Uuid, session: &Value) -> Result<(), WmSessionError> {
    if !session.is_object() {
        return Err(WmSessionError::NotAnObject);
    }
    let json = serde_json::to_string(session)?;
    if json.len() > MAX_SESSION_BYTES {
        return Err(WmSessionError::TooLarge { bytes: json.len() });
    }
    conn.execute(
        "INSERT INTO wm_sessions_no_sync (vault_device_uuid, session_json, updated_at) \
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         ON CONFLICT(vault_device_uuid) DO UPDATE SET \
           session_json = excluded.session_json, updated_at = excluded.updated_at",
        params![device.to_string(), json],
    )?;
    Ok(())
}

/// Deletes the device's saved session. Idempotent.
///
/// ponytail: no foreign key to `known_devices`, so retiring a device on
/// another peer leaves this row behind. haex-crdt applies remote deletes
/// with foreign keys off, so a cascade would not fire anyway; an orphan
/// row is harmless because only the own device's row is ever read.
pub fn delete(conn: &Connection, device: Uuid) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM wm_sessions_no_sync WHERE vault_device_uuid = ?1",
        params![device.to_string()],
    )
}
