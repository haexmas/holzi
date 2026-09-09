//! Typed writes and reads for the `models` table.
//!
//! `models` is the cross-provider catalog cache. Rows are inserted when
//! a provider's model list is fetched (plan §"Anbietermodelle":
//! "Modelllisten werden immer abgefragt, nie hartkodiert") and when a
//! local GGUF is downloaded / imported.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Model row as stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRow {
    /// Cross-provider unique id. For catalog-seeded local models this
    /// matches the catalog entry id; for API providers it is
    /// `<provider_id>:<remote_model_id>` (plan §"Datenmodell").
    pub id: String,
    pub provider_id: Uuid,
    pub name: String,
    pub context_window: Option<i64>,
    /// Epoch milliseconds. `None` for entries seeded from the built-in
    /// catalog before any live refresh.
    pub fetched_at: Option<i64>,
    /// HuggingFace repo the tokenizer.json lives in. Required for
    /// local GGUFs at `load_local_model` time. `None` for api_key /
    /// cli_delegate rows — those do not tokenize on-device.
    pub tokenizer_repo: Option<String>,
}

/// Inserts or updates a model row while preserving CRDT column metadata on
/// conflicts. Always sets `haex_hlc_no_sync = current_hlc()` (Etappe-0
/// finding #2).
pub fn upsert_model(conn: &Connection, m: &ModelRow) -> Result<usize> {
    let sql = format!(
        "INSERT INTO models \
           (id, provider_id, name, context_window, fetched_at, tokenizer_repo, {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, current_hlc()) \
         ON CONFLICT(id) DO UPDATE SET \
           provider_id = excluded.provider_id, \
           name = excluded.name, \
           context_window = excluded.context_window, \
           fetched_at = excluded.fetched_at, \
           tokenizer_repo = COALESCE(excluded.tokenizer_repo, tokenizer_repo), \
           {HLC_TIMESTAMP_COLUMN} = current_hlc()"
    );
    conn.execute(
        &sql,
        params![
            m.id,
            m.provider_id.to_string(),
            m.name,
            m.context_window,
            m.fetched_at,
            m.tokenizer_repo,
        ],
    )
}

/// Lists all models for a provider, ordered by name.
pub fn list_models_by_provider(conn: &Connection, provider_id: Uuid) -> Result<Vec<ModelRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider_id, name, context_window, fetched_at, tokenizer_repo \
         FROM models WHERE provider_id = ?1 ORDER BY name ASC",
    )?;
    let rows = stmt.query_map(params![provider_id.to_string()], row_to_model)?;
    rows.collect()
}

/// Lists all models across every provider.
pub fn list_all_models(conn: &Connection) -> Result<Vec<ModelRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider_id, name, context_window, fetched_at, tokenizer_repo \
         FROM models ORDER BY name ASC",
    )?;
    let rows = stmt.query_map([], row_to_model)?;
    rows.collect()
}

/// Reads a single model row by id, or `None` when missing.
pub fn get_model(conn: &Connection, id: &str) -> Result<Option<ModelRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider_id, name, context_window, fetched_at, tokenizer_repo \
         FROM models WHERE id = ?1",
    )?;
    stmt.query_row(params![id], row_to_model).optional()
}

/// Replaces the entire model cache for a provider: rows for
/// `provider_id` that are absent from `fresh` are removed, and each
/// entry in `fresh` is upserted. Runs inside a single transaction so
/// a partial refresh cannot leave the cache in an inconsistent state.
///
/// A failed remote fetch never reaches this function — callers only
/// invoke it on success, keeping the previous cache intact on error
/// per plan §"Anbietermodelle" ("Fehlgeschlagener Abruf erhält den
/// vorhandenen Cache").
pub fn replace_provider_models(
    conn: &Connection,
    provider_id: Uuid,
    fresh: &[ModelRow],
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM models WHERE provider_id = ?1",
        params![provider_id.to_string()],
    )?;
    let sql = format!(
        "INSERT INTO models \
           (id, provider_id, name, context_window, fetched_at, tokenizer_repo, {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, current_hlc())"
    );
    for m in fresh {
        tx.execute(
            &sql,
            params![
                m.id,
                m.provider_id.to_string(),
                m.name,
                m.context_window,
                m.fetched_at,
                m.tokenizer_repo,
            ],
        )?;
    }
    tx.commit()
}

/// Fills `tokenizer_repo` for existing rows whose id matches an entry
/// in `catalog_lookup(id) -> Option<&str>`. Idempotent — after the
/// first successful pass the `WHERE tokenizer_repo IS NULL` clause
/// filters out any row already populated. Runs cheaply enough
/// (bounded by the catalog size) to be safe to call from a hot path.
pub fn backfill_tokenizer_repo<F>(conn: &Connection, catalog_lookup: F) -> Result<usize>
where
    F: Fn(&str) -> Option<String>,
{
    let mut stmt = conn.prepare("SELECT id FROM models WHERE tokenizer_repo IS NULL")?;
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>>>()?;

    let mut updated = 0usize;
    let update_sql = format!(
        "UPDATE models \
         SET tokenizer_repo = ?1, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
         WHERE id = ?2 AND tokenizer_repo IS NULL"
    );
    for id in ids {
        if let Some(repo) = catalog_lookup(&id) {
            updated += conn.execute(&update_sql, params![repo, id])?;
        }
    }
    Ok(updated)
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
        tokenizer_repo: row.get(5)?,
    })
}
