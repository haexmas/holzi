//! Typed writes and reads for the `chat_threads` table.
//!
//! Threads are conversation containers. Individual messages live in the
//! sibling `chat_messages` module and reference `thread_id` here.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::storage::chat_messages;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatThread {
    pub id: Uuid,
    pub title: String,
    pub last_provider_id: Option<Uuid>,
    pub last_model_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Inserts a fresh chat thread. Always sets `haex_hlc_no_sync = current_hlc()`.
pub fn insert_thread(conn: &Connection, t: &ChatThread) -> Result<usize> {
    let sql = format!(
        "INSERT INTO chat_threads \
           (id, title, last_provider_id, last_model_id, \
            created_at, updated_at, {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, current_hlc())"
    );
    conn.execute(
        &sql,
        params![
            t.id.to_string(),
            t.title,
            t.last_provider_id.map(|u| u.to_string()),
            t.last_model_id,
            t.created_at,
            t.updated_at,
        ],
    )
}

/// Removes a thread created solely for a generation that failed to start.
pub fn delete_thread(conn: &Connection, id: Uuid) -> Result<usize> {
    conn.execute(
        "DELETE FROM chat_threads WHERE id = ?1",
        params![id.to_string()],
    )
}

/// Renames a thread without changing its opening time or recency ordering.
pub fn rename_title(conn: &Connection, id: Uuid, title: &str) -> Result<usize> {
    let sql = format!(
        "UPDATE chat_threads SET title = ?1, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
         WHERE id = ?2"
    );
    conn.execute(&sql, params![title, id.to_string()])
}

/// Deletes a thread and its messages in one database transaction.
///
/// The boolean is false when the thread did not exist. In that case no
/// message row is touched, even if a malformed database contains rows with
/// the same thread id.
pub fn delete_thread_and_messages(conn: &Connection, id: Uuid) -> Result<bool> {
    let tx = conn.unchecked_transaction()?;
    let exists: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM chat_threads WHERE id = ?1",
            params![id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Ok(false);
    }

    chat_messages::delete_for_thread(&tx, id)?;
    let deleted = delete_thread(&tx, id)?;
    if deleted != 1 {
        return Err(haex_crdt::rusqlite::Error::QueryReturnedNoRows);
    }
    tx.commit()?;
    Ok(true)
}

/// Updates a thread's title, last provider/model pointer and
/// `updated_at`. HLC injected per Etappe-0 finding #2.
pub fn update_thread(
    conn: &Connection,
    id: Uuid,
    title: &str,
    last_provider_id: Option<Uuid>,
    last_model_id: Option<&str>,
    updated_at: i64,
) -> Result<usize> {
    let sql = format!(
        "UPDATE chat_threads SET \
           title = ?1, last_provider_id = COALESCE(?2, last_provider_id), last_model_id = ?3, \
           updated_at = ?4, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
         WHERE id = ?5"
    );
    conn.execute(
        &sql,
        params![
            title,
            last_provider_id.map(|u| u.to_string()),
            last_model_id,
            updated_at,
            id.to_string(),
        ],
    )
}

/// Lists all threads, most-recently-updated first.
pub fn list_threads(conn: &Connection) -> Result<Vec<ChatThread>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, last_provider_id, last_model_id, created_at, updated_at \
         FROM chat_threads ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], row_to_thread)?;
    rows.collect()
}

/// Fetches a single thread by id.
pub fn get_thread(conn: &Connection, id: Uuid) -> Result<Option<ChatThread>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, last_provider_id, last_model_id, created_at, updated_at \
         FROM chat_threads WHERE id = ?1",
    )?;
    stmt.query_row(params![id.to_string()], row_to_thread)
        .optional()
}

fn row_to_thread(row: &haex_crdt::rusqlite::Row<'_>) -> Result<ChatThread> {
    let id_str: String = row.get(0)?;
    let last_provider_str: Option<String> = row.get(2)?;
    Ok(ChatThread {
        id: parse_uuid(&id_str, 0)?,
        title: row.get(1)?,
        last_provider_id: last_provider_str.map(|s| parse_uuid(&s, 2)).transpose()?,
        last_model_id: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn parse_uuid(s: &str, col: usize) -> Result<Uuid> {
    Uuid::parse_str(s).map_err(|e| {
        haex_crdt::rusqlite::Error::FromSqlConversionFailure(
            col,
            haex_crdt::rusqlite::types::Type::Text,
            Box::new(e),
        )
    })
}
