//! Typed writes and reads for the `chat_threads` table.
//!
//! Threads are conversation containers. Individual messages live in the
//! sibling `chat_messages` module and reference `thread_id` here.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    stmt.query_row(params![id.to_string()], row_to_thread).optional()
}

fn row_to_thread(row: &haex_crdt::rusqlite::Row<'_>) -> Result<ChatThread> {
    let id_str: String = row.get(0)?;
    let last_provider_str: Option<String> = row.get(2)?;
    Ok(ChatThread {
        id: parse_uuid(&id_str, 0)?,
        title: row.get(1)?,
        last_provider_id: last_provider_str
            .map(|s| parse_uuid(&s, 2))
            .transpose()?,
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
