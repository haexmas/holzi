//! Typed writes and reads for the `models` table.
//!
//! `models` is the cross-provider catalog cache. Rows are inserted when
//! a provider's model list is fetched (plan §"Anbietermodelle":
//! "Modelllisten werden immer abgefragt, nie hartkodiert") and when a
//! local GGUF is downloaded / imported.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Model row as stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRow {
    /// Cross-provider unique id. For catalog-seeded local models this
    /// matches the catalog entry id; for API providers it is
    /// `<provider_id>:<remote_model_id>`.
    pub id: String,
    pub provider_id: Uuid,
    pub name: String,
    pub context_window: Option<i64>,
    /// Epoch milliseconds. `None` for entries seeded from the built-in
    /// catalog before any live refresh.
    pub fetched_at: Option<i64>,
}

/// Inserts or replaces a model row. INSERT-OR-REPLACE is safe here
/// because `id` is a stable key and the row content is derived data —
/// LWW is exactly the semantics we want. Always sets
/// `haex_hlc_no_sync = current_hlc()` (Etappe-0 finding #2).
pub fn upsert_model(conn: &Connection, m: &ModelRow) -> Result<usize> {
    let sql = format!(
        "INSERT OR REPLACE INTO models \
           (id, provider_id, name, context_window, fetched_at, {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, ?4, ?5, current_hlc())"
    );
    conn.execute(
        &sql,
        params![
            m.id,
            m.provider_id.to_string(),
            m.name,
            m.context_window,
            m.fetched_at,
        ],
    )
}

/// Lists all models for a provider, ordered by name.
pub fn list_models_by_provider(
    conn: &Connection,
    provider_id: Uuid,
) -> Result<Vec<ModelRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider_id, name, context_window, fetched_at \
         FROM models WHERE provider_id = ?1 ORDER BY name ASC",
    )?;
    let rows = stmt.query_map(params![provider_id.to_string()], row_to_model)?;
    rows.collect()
}

/// Lists all models across every provider.
pub fn list_all_models(conn: &Connection) -> Result<Vec<ModelRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider_id, name, context_window, fetched_at \
         FROM models ORDER BY name ASC",
    )?;
    let rows = stmt.query_map([], row_to_model)?;
    rows.collect()
}

fn row_to_model(row: &haex_crdt::rusqlite::Row<'_>) -> Result<ModelRow> {
    let provider_id_str: String = row.get(1)?;
    Ok(ModelRow {
        id: row.get(0)?,
        provider_id: Uuid::parse_str(&provider_id_str).map_err(|e| {
            haex_crdt::rusqlite::Error::FromSqlConversionFailure(
                1,
                haex_crdt::rusqlite::types::Type::Text,
                Box::new(e),
            )
        })?,
        name: row.get(2)?,
        context_window: row.get(3)?,
        fetched_at: row.get(4)?,
    })
}
