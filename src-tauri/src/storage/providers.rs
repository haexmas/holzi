//! Typed writes and reads for the `providers` table.
//!
//! Etappe 2 groundwork: this slice ships the schema and storage helpers
//! only. Live provider invocation (Anthropic/OpenAI/CLI delegate) is a
//! later slice; this module makes sure the row layout the deferred sync
//! path expects is already frozen.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Provider row as stored in SQLite. `credentials` is opaque bytes — for
/// `api_key` providers it is the API key (SQLCipher protects at rest,
/// and the deferred sync payload transports it only over the encrypted
/// channel per plan §"Anbietermodelle"). For `local` and `cli_delegate`
/// providers it is `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: Uuid,
    pub kind: ProviderKind,
    pub name: String,
    pub base_url: Option<String>,
    pub credentials: Option<Vec<u8>>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Local,
    ApiKey,
    CliDelegate,
}

impl ProviderKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ProviderKind::Local => "local",
            ProviderKind::ApiKey => "api_key",
            ProviderKind::CliDelegate => "cli_delegate",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "local" => Some(ProviderKind::Local),
            "api_key" => Some(ProviderKind::ApiKey),
            "cli_delegate" => Some(ProviderKind::CliDelegate),
            _ => None,
        }
    }
}

/// Inserts a provider row. Always sets `haex_hlc_no_sync = current_hlc()`
/// so the write is visible to the sync scanner (Etappe-0 finding #2).
pub fn insert_provider(conn: &Connection, p: &Provider) -> Result<usize> {
    let sql = format!(
        "INSERT INTO providers \
           (id, kind, name, base_url, credentials, created_at, {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, current_hlc())"
    );
    conn.execute(
        &sql,
        params![
            p.id.to_string(),
            p.kind.as_str(),
            p.name,
            p.base_url,
            p.credentials,
            p.created_at,
        ],
    )
}

/// Deletes a provider by id.
pub fn delete_provider(conn: &Connection, id: Uuid) -> Result<usize> {
    conn.execute(
        "DELETE FROM providers WHERE id = ?1",
        params![id.to_string()],
    )
}

/// Lists all providers ordered by creation time (oldest first).
pub fn list_providers(conn: &Connection) -> Result<Vec<Provider>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, base_url, credentials, created_at \
         FROM providers ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([], row_to_provider)?;
    rows.collect()
}

/// Fetches one provider by id.
pub fn get_provider(conn: &Connection, id: Uuid) -> Result<Option<Provider>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, base_url, credentials, created_at \
         FROM providers WHERE id = ?1",
    )?;
    stmt.query_row(params![id.to_string()], row_to_provider)
        .optional()
}

fn row_to_provider(row: &haex_crdt::rusqlite::Row<'_>) -> Result<Provider> {
    let id_str: String = row.get(0)?;
    let kind_str: String = row.get(1)?;
    Ok(Provider {
        id: Uuid::parse_str(&id_str).map_err(|e| {
            haex_crdt::rusqlite::Error::FromSqlConversionFailure(
                0,
                haex_crdt::rusqlite::types::Type::Text,
                Box::new(e),
            )
        })?,
        kind: ProviderKind::parse(&kind_str).ok_or_else(|| {
            haex_crdt::rusqlite::Error::FromSqlConversionFailure(
                1,
                haex_crdt::rusqlite::types::Type::Text,
                format!("unknown provider kind: {kind_str}").into(),
            )
        })?,
        name: row.get(2)?,
        base_url: row.get(3)?,
        credentials: row.get(4)?,
        created_at: row.get(5)?,
    })
}
