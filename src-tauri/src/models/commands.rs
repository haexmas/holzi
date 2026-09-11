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
use crate::storage::models::{self as models_store, ModelRow};

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
        tokenizer_repo: entry.tokenizer_repo,
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
    if let Some(existing) = paths::canonical_model_file(&app, &args.id)? {
        if existing.filename != args.hf_filename {
            return Err(HolziError::InvalidInput {
                reason: format!(
                    "model slug '{}' already has finalized GGUF '{}'; cannot add '{}'",
                    args.id, existing.filename, args.hf_filename
                ),
            });
        }
    }
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
        Some(args.tokenizer_repo),
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
    if let Some(existing) = paths::canonical_model_file(&app, &args.id)? {
        if existing.filename != filename {
            return Err(HolziError::InvalidInput {
                reason: format!(
                    "model slug '{}' already has finalized GGUF '{}'; cannot add '{}'",
                    args.id, existing.filename, filename
                ),
            });
        }
    }
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
        Some(args.tokenizer_repo),
    )
    .await
}

/// Lists all locally installed models on this device.
///
/// The filesystem under `<AppLocalData>/models/<slug>/<file.gguf>` is
/// authoritative for "installed here" (spec 002 §"Installed model" and
/// ADR-0001). Discovery scans that root, resolves each slug's canonical
/// file via [`paths::canonical_model_file`], and joins it with the
/// synchronised `models` catalog row. A slug without a matching row is
/// skipped — the catalog is the display metadata source and a stale
/// on-disk file we do not know about should not appear as installed.
///
/// Opportunistically back-fills `models.tokenizer_repo` for pre-0009
/// catalog rows in the same call (idempotent).
#[tauri::command]
pub async fn list_installed_models(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<InstalledModelPayload>> {
    let db = active_database(&state)?;

    // Enumerate the models root. A missing root means "nothing
    // installed", not an error — fresh installs never open this dir.
    let models_root = paths::models_root(&app)?;
    let mut slug_dirs: Vec<String> = Vec::new();
    match std::fs::read_dir(&models_root) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(HolziError::from)?;
                if !entry.file_type().map_err(HolziError::from)?.is_dir() {
                    continue;
                }
                if let Some(name) = entry.file_name().to_str() {
                    slug_dirs.push(name.to_string());
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(HolziError::from(e)),
    }

    // Deterministic ordering — the frontend can rely on stable listings
    // between calls without sorting itself.
    slug_dirs.sort();

    // Resolve canonical files on the async runtime's blocking pool so
    // syscalls do not hold the executor.
    let app_for_scan = app.clone();
    let scan_slugs = slug_dirs.clone();
    let canonical_files = tauri::async_runtime::spawn_blocking(move || {
        let mut resolved: Vec<(String, paths::CanonicalModelFile)> = Vec::new();
        for slug in scan_slugs {
            match paths::canonical_model_file(&app_for_scan, &slug) {
                Ok(Some(cf)) => resolved.push((slug, cf)),
                Ok(None) => {}
                Err(e) => return Err(e),
            }
        }
        Ok::<_, HolziError>(resolved)
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("list_installed_models scan join: {e}"),
    })??;

    let payload = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            let catalog_repos: Vec<(&str, &str)> = catalog::entries()
                .iter()
                .map(|entry| (entry.id.as_str(), entry.tokenizer_repo.as_str()))
                .collect();
            models_store::backfill_tokenizer_repo(conn, &catalog_repos)
                .map_err(haex_crdt::Error::from)?;

            let mut out = Vec::with_capacity(canonical_files.len());
            for (slug, cf) in canonical_files {
                let Some(row) =
                    models_store::get_model(conn, &slug).map_err(haex_crdt::Error::from)?
                else {
                    continue;
                };
                out.push(InstalledModelPayload {
                    id: row.id,
                    name: row.name,
                    provider_id: row.provider_id.to_string(),
                    context_window: row.context_window,
                    relative_path: cf.relative_path,
                    size_bytes: cf.size_bytes as i64,
                });
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

/// Removes the on-disk GGUF file for a model slug. The `models` row
/// stays in place because it is synced catalog metadata; deleting the
/// file only removes it from this device's installed list. Idempotent
/// when the file is already absent.
#[tauri::command]
pub async fn delete_installed_model(
    app: AppHandle,
    _state: State<'_, AppState>,
    id: String,
) -> Result<()> {
    let Some(canonical) = paths::canonical_model_file(&app, &id)? else {
        return Ok(());
    };
    tokio::fs::remove_file(&canonical.absolute_path)
        .await
        .map_err(|e| HolziError::Io {
            reason: format!("remove {}: {e}", canonical.absolute_path.display()),
        })?;
    Ok(())
}

/// Shared post-download / post-import work: `models` upsert + ensure
/// local provider. The on-disk file is authoritative for "installed
/// here" (spec 002 §"Installed model"); the caller has already written
/// the finalised `.gguf` before invoking this helper, so there is no
/// separate installed-registry to update.
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
                tokenizer_repo,
            };
            models_store::upsert_model(conn, &m).map_err(haex_crdt::Error::from)?;
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
