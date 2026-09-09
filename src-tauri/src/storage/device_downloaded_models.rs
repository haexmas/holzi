//! Typed writes and reads for the `device_downloaded_models_no_sync` table.
//!
//! This is per-installation state: GGUF file paths and verification
//! metadata that must NOT sync (plan §"Datenmodell": local file paths
//! and model bytes stay under this host's `AppLocalData/models/`). The
//! table name carries the `_no_sync` suffix so haex-crdt skips it
//! entirely — no HLC column, no triggers.

use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadedModel {
    /// Matches `models.id` so a downloaded GGUF joins the catalog entry.
    pub id: String,
    /// Path relative to `<AppLocalData>/models/`. Relative on purpose:
    /// the same row round-trips through iOS/Android sandboxing where
    /// the absolute prefix changes across app restarts.
    pub relative_path: String,
    pub size_bytes: i64,
    /// Optional hash — the catalog may or may not ship a checksum, and
    /// HuggingFace does not return one in a stable form. When present
    /// the download path verifies before writing this row.
    pub sha256: Option<String>,
    pub verified_at: i64,
}

/// Inserts or replaces a downloaded-model row. INSERT OR REPLACE is
/// intentional: a re-download or re-import of the same catalog id
/// supersedes the previous row.
pub fn upsert_downloaded_model(conn: &Connection, dm: &DownloadedModel) -> Result<usize> {
    conn.execute(
        "INSERT OR REPLACE INTO device_downloaded_models_no_sync \
           (id, relative_path, size_bytes, sha256, verified_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            dm.id,
            dm.relative_path,
            dm.size_bytes,
            dm.sha256,
            dm.verified_at,
        ],
    )
}

/// Fetches a downloaded-model row by id.
pub fn get_downloaded_model(conn: &Connection, id: &str) -> Result<Option<DownloadedModel>> {
    let mut stmt = conn.prepare(
        "SELECT id, relative_path, size_bytes, sha256, verified_at \
         FROM device_downloaded_models_no_sync WHERE id = ?1",
    )?;
    stmt.query_row(params![id], row_to_dm).optional()
}

/// Lists all downloaded models on this installation.
pub fn list_downloaded_models(conn: &Connection) -> Result<Vec<DownloadedModel>> {
    let mut stmt = conn.prepare(
        "SELECT id, relative_path, size_bytes, sha256, verified_at \
         FROM device_downloaded_models_no_sync ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], row_to_dm)?;
    rows.collect()
}

/// Removes a downloaded-model registry entry. Callers are responsible
/// for deleting the actual GGUF file on disk afterwards; the registry
/// and the filesystem are kept in sync at the command layer.
pub fn delete_downloaded_model(conn: &Connection, id: &str) -> Result<usize> {
    conn.execute(
        "DELETE FROM device_downloaded_models_no_sync WHERE id = ?1",
        params![id],
    )
}

fn row_to_dm(row: &haex_crdt::rusqlite::Row<'_>) -> Result<DownloadedModel> {
    Ok(DownloadedModel {
        id: row.get(0)?,
        relative_path: row.get(1)?,
        size_bytes: row.get(2)?,
        sha256: row.get(3)?,
        verified_at: row.get(4)?,
    })
}
