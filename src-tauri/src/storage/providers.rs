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
/// `api_key` providers it is the API key, and for `cli_delegate`
/// providers (spec 007-cli-delegate) it is a Claude OAuth token or a
/// Codex `auth.json`'s raw bytes, depending on `adapter` (SQLCipher
/// protects at rest either way, and the deferred sync payload transports
/// it only over the encrypted channel per plan §"Anbietermodelle"). For
/// `local` providers it is `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: Uuid,
    pub kind: ProviderKind,
    /// Vendor discriminator: for `api_key` providers a vendor like
    /// `"anthropic"`; for `cli_delegate` providers (spec 007-cli-delegate)
    /// `"claude"` or `"codex"`. `None` for legacy rows and `local`.
    pub adapter: Option<String>,
    pub name: String,
    pub base_url: Option<String>,
    pub credentials: Option<Vec<u8>>,
    pub created_at: i64,
    /// What this row is used for — orthogonal to `kind` (spec 008: a
    /// `Local` row can be either the chat model or the bundled Whisper
    /// transcription adapter). Defaults to `Chat` for every row that
    /// predates this column (migration `0016_providers_add_capability`).
    pub capability: ProviderCapability,
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

/// What a provider row is used for (spec 008 data-model.md
/// §"Schemaänderung: providers.capability"). Orthogonal to `ProviderKind` —
/// e.g. a `Local` row can be the chat model (`Chat`) or the bundled Whisper
/// adapter (`Transcription`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapability {
    Chat,
    Transcription,
}

impl ProviderCapability {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ProviderCapability::Chat => "chat",
            ProviderCapability::Transcription => "transcription",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "chat" => Some(ProviderCapability::Chat),
            "transcription" => Some(ProviderCapability::Transcription),
            _ => None,
        }
    }
}

/// Inserts a provider row. Always sets `haex_hlc_no_sync = current_hlc()`
/// so the write is visible to the sync scanner (Etappe-0 finding #2).
pub fn insert_provider(conn: &Connection, p: &Provider) -> Result<usize> {
    let sql = format!(
        "INSERT INTO providers \
           (id, kind, adapter, name, base_url, credentials, created_at, capability, {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, current_hlc())"
    );
    conn.execute(
        &sql,
        params![
            p.id.to_string(),
            p.kind.as_str(),
            p.adapter,
            p.name,
            p.base_url,
            p.credentials,
            p.created_at,
            p.capability.as_str(),
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

/// Persists a repaired adapter discriminator and marks the CRDT row dirty.
pub fn set_adapter(conn: &Connection, id: Uuid, adapter: &str) -> Result<usize> {
    let sql = format!(
        "UPDATE providers SET adapter = ?1, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
         WHERE id = ?2"
    );
    conn.execute(&sql, params![adapter, id.to_string()])
}

/// Overwrites a provider's stored credentials in place (spec 007-cli-delegate
/// `connect_cli_delegate`/`submit_cli_delegate_code` upsert semantics,
/// contracts/tauri-commands.md — a reconnect updates the existing
/// `cli_delegate` row instead of inserting a duplicate).
pub fn update_credentials(conn: &Connection, id: Uuid, credentials: &[u8]) -> Result<usize> {
    let sql = format!(
        "UPDATE providers SET credentials = ?1, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
         WHERE id = ?2"
    );
    conn.execute(&sql, params![credentials, id.to_string()])
}

/// Finds the singleton `cli_delegate` provider row for one vendor, if any
/// (mirrors `providers::local::find_local_provider`'s one-row-per-kind
/// pattern, scoped further by `adapter`).
pub fn find_cli_delegate_provider(conn: &Connection, vendor: &str) -> Result<Option<Uuid>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM providers WHERE kind = ?1 AND adapter = ?2 \
         ORDER BY created_at ASC LIMIT 1",
    )?;
    let raw: Option<String> = stmt
        .query_row(params![ProviderKind::CliDelegate.as_str(), vendor], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(raw.and_then(|s| Uuid::parse_str(&s).ok()))
}

/// Finds the singleton row for one `(kind, capability)` pair, if any —
/// shared by `providers::local::find_local_provider` (capability `chat`)
/// and `find_local_transcription_provider` (capability `transcription`).
/// Without the `capability` filter, two `kind = 'local'` rows exist once a
/// local transcription provider is created (spec 008 data-model.md), and
/// which one an unqualified `kind`-only query returns would depend on
/// `created_at` ordering.
pub fn find_provider_by_kind_and_capability(
    conn: &Connection,
    kind: ProviderKind,
    capability: ProviderCapability,
) -> Result<Option<Uuid>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM providers WHERE kind = ?1 AND capability = ?2 \
         ORDER BY created_at ASC LIMIT 1",
    )?;
    let raw: Option<String> = stmt
        .query_row(params![kind.as_str(), capability.as_str()], |r| r.get(0))
        .optional()?;
    Ok(raw.and_then(|s| Uuid::parse_str(&s).ok()))
}

/// Lists all providers ordered by creation time (oldest first).
pub fn list_providers(conn: &Connection) -> Result<Vec<Provider>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, adapter, name, base_url, credentials, created_at, capability \
         FROM providers ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([], row_to_provider)?;
    rows.collect()
}

/// Fetches one provider by id.
pub fn get_provider(conn: &Connection, id: Uuid) -> Result<Option<Provider>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, adapter, name, base_url, credentials, created_at, capability \
         FROM providers WHERE id = ?1",
    )?;
    stmt.query_row(params![id.to_string()], row_to_provider)
        .optional()
}

fn row_to_provider(row: &haex_crdt::rusqlite::Row<'_>) -> Result<Provider> {
    let id_str: String = row.get(0)?;
    let kind_str: String = row.get(1)?;
    let capability_str: String = row.get(7)?;
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
        adapter: row.get(2)?,
        name: row.get(3)?,
        base_url: row.get(4)?,
        credentials: row.get(5)?,
        created_at: row.get(6)?,
        capability: ProviderCapability::parse(&capability_str).ok_or_else(|| {
            haex_crdt::rusqlite::Error::FromSqlConversionFailure(
                7,
                haex_crdt::rusqlite::types::Type::Text,
                format!("unknown provider capability: {capability_str}").into(),
            )
        })?,
    })
}
