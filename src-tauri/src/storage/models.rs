//! Typed writes and reads for the `models` table.
//!
//! `models` is the cross-provider catalog cache. Rows are inserted when
//! a provider's model list is fetched (plan §"Anbietermodelle":
//! "Modelllisten werden immer abgefragt, nie hartkodiert") and when a
//! local GGUF is downloaded / imported.

use std::collections::HashSet;

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where a `models` row's file came from. Persisted in `source_kind`
/// (migration `0015_models_add_huggingface_source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Seeded from the built-in curated catalog.
    Catalog,
    /// Installed via the free HuggingFace discovery/download flow.
    Huggingface,
    /// Copied in from an operator-picked local file.
    Imported,
    /// An `api_key` / remote provider's model — never has a local file.
    Provider,
}

impl SourceKind {
    fn as_str(self) -> &'static str {
        match self {
            SourceKind::Catalog => "catalog",
            SourceKind::Huggingface => "huggingface",
            SourceKind::Imported => "imported",
            SourceKind::Provider => "provider",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "catalog" => SourceKind::Catalog,
            "huggingface" => SourceKind::Huggingface,
            "imported" => SourceKind::Imported,
            // Unknown/legacy values fall back to `provider`, the migration
            // default — never misclassified as `catalog`/`huggingface`.
            _ => SourceKind::Provider,
        }
    }
}

/// Visible integrity state of a local model file against its stored
/// `file_sha256` (spec 005 Entscheidung 6). Persisted in
/// `integrity_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityStatus {
    /// The last hash check matched `file_sha256` exactly.
    Verified,
    /// The operator explicitly accepted a mismatch via
    /// `load_model_with_integrity_override`; `file_sha256` is unchanged.
    Untrusted,
    /// No check has run yet, or the row predates hashing
    /// (migration default for every pre-existing row).
    Unknown,
}

impl IntegrityStatus {
    fn as_str(self) -> &'static str {
        match self {
            IntegrityStatus::Verified => "verified",
            IntegrityStatus::Untrusted => "untrusted",
            IntegrityStatus::Unknown => "unknown",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "verified" => IntegrityStatus::Verified,
            "untrusted" => IntegrityStatus::Untrusted,
            _ => IntegrityStatus::Unknown,
        }
    }
}

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
    /// HuggingFace repository id (`owner/name`) this file was installed
    /// from. `None` for catalog/imported/provider rows.
    pub hf_repo: Option<String>,
    /// The exact GGUF filename selected inside `hf_repo`. `None` unless
    /// `hf_repo` is set.
    pub hf_filename: Option<String>,
    /// Commit SHA resolved and used for the install/update that produced
    /// the current local file. `None` unless `hf_repo` is set.
    pub hf_revision: Option<String>,
    /// Optional mutable branch/tag tracked for update checks. `None` for
    /// a direct SHA pin or a non-HF row.
    pub hf_revision_ref: Option<String>,
    /// Full SHA-256 hash of the local file's content, set only after a
    /// successful atomic download/update/import publish.
    pub file_sha256: Option<String>,
    pub integrity_status: IntegrityStatus,
    pub source_kind: SourceKind,
}

const SELECT_COLUMNS: &str = "id, provider_id, name, context_window, fetched_at, tokenizer_repo, \
     hf_repo, hf_filename, hf_revision, hf_revision_ref, file_sha256, integrity_status, source_kind";

/// Inserts or updates a model row while preserving CRDT column metadata on
/// conflicts. Always sets `haex_hlc_no_sync = current_hlc()` (Etappe-0
/// finding #2).
///
/// The HF source, hash and status columns are overwritten unconditionally
/// (unlike `tokenizer_repo`'s `COALESCE`) — every caller of this function
/// passes the authoritative value for that write (a provider refresh's
/// `None`s, or a download/update/import's freshly computed source +
/// `file_sha256`). A narrower in-place status change (e.g. the integrity
/// override) uses [`set_integrity_status`] instead of a full upsert.
pub fn upsert_model(conn: &Connection, m: &ModelRow) -> Result<usize> {
    let sql = format!(
        "INSERT INTO models \
           (id, provider_id, name, context_window, fetched_at, tokenizer_repo, \
            hf_repo, hf_filename, hf_revision, hf_revision_ref, file_sha256, \
            integrity_status, source_kind, {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, current_hlc()) \
         ON CONFLICT(id) DO UPDATE SET \
           provider_id = excluded.provider_id, \
           name = excluded.name, \
           context_window = excluded.context_window, \
           fetched_at = excluded.fetched_at, \
           tokenizer_repo = COALESCE(excluded.tokenizer_repo, tokenizer_repo), \
           hf_repo = excluded.hf_repo, \
           hf_filename = excluded.hf_filename, \
           hf_revision = excluded.hf_revision, \
           hf_revision_ref = excluded.hf_revision_ref, \
           file_sha256 = excluded.file_sha256, \
           integrity_status = excluded.integrity_status, \
           source_kind = excluded.source_kind, \
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
            m.hf_repo,
            m.hf_filename,
            m.hf_revision,
            m.hf_revision_ref,
            m.file_sha256,
            m.integrity_status.as_str(),
            m.source_kind.as_str(),
        ],
    )
}

/// Lists all models for a provider, ordered by name.
pub fn list_models_by_provider(conn: &Connection, provider_id: Uuid) -> Result<Vec<ModelRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM models WHERE provider_id = ?1 ORDER BY name ASC"
    ))?;
    let rows = stmt.query_map(params![provider_id.to_string()], row_to_model)?;
    rows.collect()
}

/// Lists all models across every provider.
pub fn list_all_models(conn: &Connection) -> Result<Vec<ModelRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM models ORDER BY name ASC"
    ))?;
    let rows = stmt.query_map([], row_to_model)?;
    rows.collect()
}

/// Reads a single model row by id, or `None` when missing.
pub fn get_model(conn: &Connection, id: &str) -> Result<Option<ModelRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM models WHERE id = ?1"
    ))?;
    stmt.query_row(params![id], row_to_model).optional()
}

/// Narrowly updates `integrity_status` without touching any other column —
/// in particular never `file_sha256` (research.md Entscheidung 6: "Das
/// Setzen einer neuen erwarteten SHA ist nur Bestandteil eines
/// erfolgreichen Downloads, Updates oder Imports; ein Load darf sie
/// niemals aktualisieren"). Used both by a successful pre-load hash match
/// (-> `Verified`) and by the explicit `load_model_with_integrity_override`
/// path (-> `Untrusted`).
pub fn set_integrity_status(conn: &Connection, id: &str, status: IntegrityStatus) -> Result<usize> {
    let sql = format!(
        "UPDATE models SET integrity_status = ?1, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
         WHERE id = ?2"
    );
    conn.execute(&sql, params![status.as_str(), id])
}

/// Records a successful upstream revision replacement for the same model
/// id: new commit SHA, new file hash, status reset to `verified`. Does not
/// touch `hf_revision_ref` — the tracked branch/tag is unchanged by an
/// update. Only called after the new file has been atomically published
/// (data-model.md "Ein Update ersetzt die Datei unter derselben Modell-ID
/// erst nach erfolgreicher atomarer Veröffentlichung").
pub fn update_hf_revision_and_hash(
    conn: &Connection,
    id: &str,
    new_revision: &str,
    new_file_sha256: &str,
) -> Result<usize> {
    let sql = format!(
        "UPDATE models SET hf_revision = ?1, file_sha256 = ?2, integrity_status = ?3, \
         {HLC_TIMESTAMP_COLUMN} = current_hlc() WHERE id = ?4"
    );
    conn.execute(
        &sql,
        params![
            new_revision,
            new_file_sha256,
            IntegrityStatus::Verified.as_str(),
            id
        ],
    )
}

/// Replaces the entire model cache for a provider: every entry in
/// `fresh` is upserted, then rows for `provider_id` that are absent
/// from `fresh` are removed. Runs inside a single transaction so a
/// partial refresh cannot leave the cache in an inconsistent state.
///
/// Order matters, and so does upserting rather than delete-then-insert.
/// `models` is CRDT-tracked: a DELETE fires the BEFORE-DELETE trigger,
/// which appends a tombstone to `haex_deleted_rows` carrying
/// `current_hlc()`. That value is pinned per transaction, so a
/// re-INSERT of the same id in the same transaction lands on the *same*
/// HLC — and `delete_shadows_insert` resolves that tie in favour of the
/// delete, meaning a peer applying the payload would drop every
/// refreshed row. Touching only the rows that actually changed keeps
/// the delete-log free of tombstones for surviving models.
///
/// Upserting also tolerates a provider that reports the same model id
/// twice (cursor pagination can overlap when the remote catalog changes
/// mid-listing) — a plain INSERT would abort the whole refresh on the
/// primary-key conflict.
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

    for m in fresh {
        upsert_model(&tx, m)?;
    }

    let keep: HashSet<&str> = fresh.iter().map(|m| m.id.as_str()).collect();
    let stale: Vec<String> = {
        let mut stmt = tx.prepare("SELECT id FROM models WHERE provider_id = ?1")?;
        let rows = stmt.query_map(params![provider_id.to_string()], |r| r.get::<_, String>(0))?;
        let mut stale = Vec::new();
        for row in rows {
            let id = row?;
            if !keep.contains(id.as_str()) {
                stale.push(id);
            }
        }
        stale
    };
    for id in stale {
        tx.execute("DELETE FROM models WHERE id = ?1", params![id])?;
    }

    tx.commit()
}

/// Fills `tokenizer_repo` for the ids in `catalog` that still have the
/// column NULL. Idempotent — the `WHERE tokenizer_repo IS NULL` clause
/// filters out any row already populated, and a second pass updates
/// nothing.
///
/// Driven by the catalog rather than by a scan of the table: rows
/// written by `replace_provider_models` keep `tokenizer_repo` NULL
/// forever (API providers tokenize server-side), so scanning for NULLs
/// would re-read every remote model on every call. Bounded by the
/// catalog size, which is what makes this safe on a hot path.
pub fn backfill_tokenizer_repo(conn: &Connection, catalog: &[(&str, &str)]) -> Result<usize> {
    let update_sql = format!(
        "UPDATE models \
         SET tokenizer_repo = ?1, {HLC_TIMESTAMP_COLUMN} = current_hlc() \
         WHERE id = ?2 AND tokenizer_repo IS NULL"
    );
    let mut updated = 0usize;
    for (id, repo) in catalog {
        updated += conn.execute(&update_sql, params![repo, id])?;
    }
    Ok(updated)
}

/// Reclassifies pre-`0015` local-provider rows from the migration default
/// `source_kind = 'provider'` into `catalog` or `imported`. A SQL-only
/// migration cannot see the compiled catalog id list, so this mirrors the
/// existing [`backfill_tokenizer_repo`] idiom: driven by the (small,
/// compile-time) catalog rather than a full-table scan, called
/// opportunistically from `list_installed_models`. Idempotent — every
/// `WHERE` clause re-selects only rows still at the migration default.
///
/// Rows under an `api_key` provider are never touched: the `provider_id =
/// ?1` (local provider) predicate excludes them, so a true provider row
/// keeps `source_kind = 'provider'` regardless of its id.
pub fn backfill_source_kind(
    conn: &Connection,
    local_provider_id: Uuid,
    catalog_ids: &[&str],
) -> Result<usize> {
    let local_provider = local_provider_id.to_string();
    let mut updated = 0usize;
    for id in catalog_ids {
        updated += conn.execute(
            &format!(
                "UPDATE models SET source_kind = 'catalog', {HLC_TIMESTAMP_COLUMN} = current_hlc() \
                 WHERE id = ?1 AND provider_id = ?2 AND source_kind = 'provider'"
            ),
            params![id, local_provider],
        )?;
    }
    // `NOT IN ()` is invalid SQLite syntax for an empty list — fall back
    // to the unconditional form when there is nothing to exclude.
    let sql = if catalog_ids.is_empty() {
        format!(
            "UPDATE models SET source_kind = 'imported', {HLC_TIMESTAMP_COLUMN} = current_hlc() \
             WHERE provider_id = ?1 AND source_kind = 'provider'"
        )
    } else {
        let placeholders = std::iter::repeat_n("?", catalog_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "UPDATE models SET source_kind = 'imported', {HLC_TIMESTAMP_COLUMN} = current_hlc() \
             WHERE provider_id = ?1 AND source_kind = 'provider' AND id NOT IN ({placeholders})"
        )
    };
    let mut params_vec: Vec<&dyn haex_crdt::rusqlite::ToSql> = vec![&local_provider];
    for id in catalog_ids {
        params_vec.push(id);
    }
    updated += conn.execute(&sql, params_vec.as_slice())?;
    Ok(updated)
}

fn row_to_model(row: &haex_crdt::rusqlite::Row<'_>) -> Result<ModelRow> {
    let provider_id_str: String = row.get(1)?;
    let integrity_status_str: String = row.get(11)?;
    let source_kind_str: String = row.get(12)?;
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
        hf_repo: row.get(6)?,
        hf_filename: row.get(7)?,
        hf_revision: row.get(8)?,
        hf_revision_ref: row.get(9)?,
        file_sha256: row.get(10)?,
        integrity_status: IntegrityStatus::from_str(&integrity_status_str),
        source_kind: SourceKind::from_str(&source_kind_str),
    })
}
