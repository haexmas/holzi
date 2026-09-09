//! Tauri commands for downloading, importing and managing local GGUFs.
//!
//! Every download command emits a `model-download-progress` event with
//! `{ modelId, bytesDownloaded, bytesTotal }` while the transfer runs
//! and a `model-download-complete` event on success. Errors surface
//! through the command return value; there is no "error" event.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::catalog;
use crate::error::{HolziError, Result};
use crate::providers::local::ensure_local_provider;
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::{
    device_downloaded_models::{self as dm_store, DownloadedModel},
    models::{self as models_store, ModelRow},
};

use super::{download, import, paths};

const EVENT_PROGRESS: &str = "model-download-progress";
const EVENT_COMPLETE: &str = "model-download-complete";

/// Payload for the frontend: joins the model row and the local file
/// info when present.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModelPayload {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    pub context_window: Option<i64>,
    pub relative_path: String,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    model_id: String,
    bytes_downloaded: u64,
    bytes_total: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFromHfArgs {
    /// Stable id under which this model is stored. Must not collide
    /// with an existing catalog id.
    pub id: String,
    pub name: String,
    pub hf_repo: String,
    pub hf_filename: String,
    pub tokenizer_repo: String,
    pub context_window: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportModelArgs {
    pub id: String,
    pub name: String,
    pub source_path: String,
    pub tokenizer_repo: String,
    pub context_window: Option<i64>,
    /// The GGUF filename inside the managed slug dir. Defaults to the
    /// source file's basename if not given.
    pub filename: Option<String>,
}

/// Downloads a model from the built-in catalog. Wraps
/// [`download_from_hf_url`] with the catalog entry's fields.
#[tauri::command]
pub async fn download_model_from_catalog(
    app: AppHandle,
    state: State<'_, AppState>,
    catalog_id: String,
) -> Result<InstalledModelPayload> {
    let entry =
        catalog::get(&catalog_id)
            .cloned()
            .ok_or_else(|| HolziError::CatalogEntryNotFound {
                id: catalog_id.clone(),
            })?;
    let args = DownloadFromHfArgs {
        id: entry.id.clone(),
        name: entry.name.clone(),
        hf_repo: entry.hf_repo,
        hf_filename: entry.hf_filename,
        tokenizer_repo: entry.tokenizer_repo.clone(),
        context_window: Some(entry.context_window as i64),
    };
    download_model_from_hf(app, state, args).await
}

/// Downloads a model from an arbitrary HuggingFace repo/filename pair.
#[tauri::command]
pub async fn download_model_from_hf(
    app: AppHandle,
    state: State<'_, AppState>,
    args: DownloadFromHfArgs,
) -> Result<InstalledModelPayload> {
    let destination = paths::model_file_path(&app, &args.id, &args.hf_filename)?;
    let relative = paths::relative_path(&args.id, &args.hf_filename)?;
    let url = catalog::hf_resolve_url(&args.hf_repo, &args.hf_filename);

    let app_for_progress = app.clone();
    let model_id_for_progress = args.id.clone();

    let downloaded = download::download_to_file(&url, destination.clone(), move |p| {
        let _ = app_for_progress.emit(
            EVENT_PROGRESS,
            ProgressEvent {
                model_id: model_id_for_progress.clone(),
                bytes_downloaded: p.bytes_downloaded,
                bytes_total: p.bytes_total,
            },
        );
    })
    .await?;

    let payload = register_downloaded(
        &state,
        &args.id,
        &args.name,
        &relative,
        downloaded.bytes_written as i64,
        args.context_window,
        Some(args.tokenizer_repo.clone()),
    )
    .await?;

    let _ = app.emit(EVENT_COMPLETE, &payload);
    Ok(payload)
}

/// Imports a GGUF from an operator-picked path.
#[tauri::command]
pub async fn import_model_from_file(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ImportModelArgs,
) -> Result<InstalledModelPayload> {
    let source = PathBuf::from(&args.source_path);
    let filename = args
        .filename
        .or_else(|| {
            source
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| HolziError::InvalidInput {
            reason: "source path has no filename".into(),
        })?;
    let destination = paths::model_file_path(&app, &args.id, &filename)?;
    let relative = paths::relative_path(&args.id, &filename)?;

    let bytes = import::copy_into_managed(&source, destination).await?;
    register_downloaded(
        &state,
        &args.id,
        &args.name,
        &relative,
        bytes as i64,
        args.context_window,
        Some(args.tokenizer_repo.clone()),
    )
    .await
}

/// Lists all installed local models by joining `models` and
/// `device_downloaded_models_no_sync`. Rows without a filesystem
/// registration are skipped — they represent catalog entries the user
/// has not yet downloaded.
///
/// Also opportunistically back-fills `models.tokenizer_repo` for any
/// catalog rows that predate migration 0009 (idempotent — the UPDATE
/// only touches rows where the column is still NULL and the id
/// matches a compiled-in catalog entry).
#[tauri::command]
pub async fn list_installed_models(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledModelPayload>> {
    let db = active_database(&state)?;
    let payload = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            let _ = models_store::backfill_tokenizer_repo(conn, |id| {
                catalog::get(id).map(|entry| entry.tokenizer_repo.clone())
            })
            .map_err(haex_crdt::Error::from)?;

            let installed =
                dm_store::list_downloaded_models(conn).map_err(haex_crdt::Error::from)?;
            let mut out = Vec::with_capacity(installed.len());
            for dm in installed {
                let mut stmt = conn.prepare(
                    "SELECT id, provider_id, name, context_window \
                     FROM models WHERE id = ?1",
                )?;
                let row: Option<(String, String, String, Option<i64>)> = stmt
                    .query_row(haex_crdt::rusqlite::params![dm.id], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                    })
                    .ok();
                if let Some((id, provider_id, name, context_window)) = row {
                    out.push(InstalledModelPayload {
                        id,
                        name,
                        provider_id,
                        context_window,
                        relative_path: dm.relative_path,
                        size_bytes: dm.size_bytes,
                    });
                }
            }
            Ok(out)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("list_installed_models join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(payload)
}

/// Removes both the registry row and the on-disk GGUF file.
#[tauri::command]
pub async fn delete_installed_model(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<()> {
    let db = active_database(&state)?;
    let id_for_db = id.clone();
    let existing = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            let existing =
                dm_store::get_downloaded_model(conn, &id_for_db).map_err(haex_crdt::Error::from)?;
            dm_store::delete_downloaded_model(conn, &id_for_db).map_err(haex_crdt::Error::from)?;
            Ok(existing)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("delete_installed_model join: {e}"),
    })?
    .map_err(HolziError::from)?;

    if let Some(dm) = existing {
        let path = paths::resolve_relative(&app, &dm.relative_path)?;
        if path.is_file() {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| HolziError::Io {
                    reason: format!("remove {}: {e}", path.display()),
                })?;
        }
    }
    Ok(())
}

/// Shared post-download / post-import work: `models` upsert + registry
/// row + ensure local provider. Executed under a single DB lock.
async fn register_downloaded(
    state: &State<'_, AppState>,
    id: &str,
    name: &str,
    relative: &str,
    size_bytes: i64,
    context_window: Option<i64>,
    tokenizer_repo: Option<String>,
) -> Result<InstalledModelPayload> {
    let db = active_database(state)?;
    let id_owned = id.to_string();
    let name_owned = name.to_string();
    let relative_owned = relative.to_string();

    let payload = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            let provider_id = ensure_local_provider(conn).map_err(haex_crdt::Error::from)?;
            let m = ModelRow {
                id: id_owned.clone(),
                provider_id,
                name: name_owned.clone(),
                context_window,
                fetched_at: Some(now_ms()),
                tokenizer_repo: tokenizer_repo.clone(),
            };
            models_store::upsert_model(conn, &m).map_err(haex_crdt::Error::from)?;
            let dm = DownloadedModel {
                id: id_owned.clone(),
                relative_path: relative_owned.clone(),
                size_bytes,
                sha256: None,
                verified_at: now_ms(),
            };
            dm_store::upsert_downloaded_model(conn, &dm).map_err(haex_crdt::Error::from)?;
            Ok(InstalledModelPayload {
                id: id_owned,
                name: name_owned,
                provider_id: provider_id.to_string(),
                context_window,
                relative_path: relative_owned,
                size_bytes,
            })
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("register_downloaded join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(payload)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
