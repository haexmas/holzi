//! Model-lifecycle Tauri commands: loading a remote or local model into
//! the active session (with the mandatory local integrity check),
//! unloading it, and reporting which one is active.
//!
//! Split out of `chat/commands.rs` (2026-09-15 review) — second of five
//! steps. See `chat/commands.rs`'s own history for the rest of the split
//! plan.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[cfg(feature = "llm-cpu")]
use crate::adapters::local::LocalAdapter;
use crate::error::{HolziError, Result};
#[cfg(feature = "llm-cpu")]
use crate::llm::local::LocalModel;
#[cfg(feature = "llm-cpu")]
use crate::models::paths;
use crate::providers::build_adapter;
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::{models as models_store, providers as providers_store};

use super::events::{emit_load_error, emit_load_progress, emit_model_load_status};
use super::session::{ActiveSession, ChatState};

/// Semantic phase of a model-load. See spec 002 §FR-015b + contracts.
/// Backend never emits localised strings; frontend translates via
/// `chat.loading.<phase>` (FR-020 i18n boundary).
// `CudaJitWarmup` is only constructed under `feature = "llm-cuda"` —
// `dead_code` fires on default-feature builds. The variant is part of
// the wire contract (spec 002 §FR-015b), so silence the lint rather
// than hide the enum behind a cfg.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LoadPhase {
    Connecting,
    Loading,
    CudaJitWarmup,
    Ready,
}

/// Payload for `active_model_info` and `load_model`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedModelInfo {
    pub model_id: String,
    pub name: String,
    pub tokenizer_repo: String,
    pub context_window: Option<i64>,
}

#[derive(Clone, Copy)]
pub(crate) struct LoadIdentity {
    pub(crate) vault_generation: u64,
    pub(crate) load_id: u64,
}

pub(crate) enum LoadOutcome {
    Loaded(LoadedModelInfo),
    Cancelled,
}

impl LoadPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Loading => "loading",
            Self::CudaJitWarmup => "cuda-jit-warmup",
            Self::Ready => "ready",
        }
    }
}

fn cancelled(cancel: Option<&CancellationToken>) -> bool {
    cancel.is_some_and(CancellationToken::is_cancelled)
}

fn publish_load_phase(
    app: &AppHandle,
    chat: &ChatState,
    identity: LoadIdentity,
    model_id: &str,
    model_name: &str,
    phase: LoadPhase,
    provider_name: Option<String>,
) -> bool {
    if !chat.set_model_loading(
        identity.vault_generation,
        identity.load_id,
        model_id.to_string(),
        model_name.to_string(),
        phase.as_str().to_string(),
        provider_name.clone(),
    ) {
        return false;
    }
    emit_load_progress(
        app,
        identity.vault_generation,
        identity.load_id,
        model_id,
        model_name,
        phase,
        provider_name,
    );
    true
}

/// Shared implementation for manual model selection and the Vault-open
/// preload. The operation mutex is deliberately owned by the caller: the
/// preload must remain cancellable while Vault lifecycle commands proceed.
pub(crate) async fn load_model_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    chat: &ChatState,
    model_id: &str,
    identity: LoadIdentity,
    cancel: Option<&CancellationToken>,
    integrity_override: bool,
) -> Result<LoadOutcome> {
    #[cfg(not(feature = "llm-cpu"))]
    let _ = integrity_override;

    if cancelled(cancel) || !chat.is_current_load(identity.vault_generation, identity.load_id) {
        return Ok(LoadOutcome::Cancelled);
    }

    let name = resolve_display_name(state, model_id)
        .await
        .unwrap_or_else(|| model_id.to_string());

    let session = if let Some((provider_id_str, _remote_id)) = model_id.split_once(':') {
        let provider_name = resolve_provider_name(state, provider_id_str).await;
        if !publish_load_phase(
            app,
            chat,
            LoadIdentity { ..identity },
            model_id,
            &name,
            LoadPhase::Connecting,
            provider_name,
        ) || cancelled(cancel)
        {
            return Ok(LoadOutcome::Cancelled);
        }
        // This path only performs bounded database/adapter setup. Await it
        // to completion so cancellation cannot drop a spawn_blocking DB read
        // while a Vault close is waiting to release its Arc.
        load_api_key_model(state, model_id, provider_id_str).await?
    } else {
        #[cfg(feature = "llm-cpu")]
        {
            let phase = local_load_phase();
            if !publish_load_phase(
                app,
                chat,
                LoadIdentity { ..identity },
                model_id,
                &name,
                phase,
                None,
            ) || cancelled(cancel)
            {
                return Ok(LoadOutcome::Cancelled);
            }
            // Resolve all database metadata before entering the cancellable
            // model load. This guarantees that close never returns while a
            // detached spawn_blocking task still retains the database Arc.
            let metadata =
                resolve_local_model_metadata(app, state, model_id, integrity_override).await?;
            let future = load_local_model_from_metadata(model_id, metadata);
            match cancel {
                Some(cancel) => tokio::select! {
                    _ = cancel.cancelled() => return Ok(LoadOutcome::Cancelled),
                    result = future => result?,
                },
                None => future.await?,
            }
        }
        #[cfg(not(feature = "llm-cpu"))]
        {
            return Err(HolziError::InvalidInput {
                reason: "local inference is not enabled in this build".into(),
            });
        }
    };

    if cancelled(cancel) || !chat.is_current_load(identity.vault_generation, identity.load_id) {
        return Ok(LoadOutcome::Cancelled);
    }

    let info = LoadedModelInfo {
        model_id: session.model_id.clone(),
        name,
        tokenizer_repo: session.tokenizer_repo.clone(),
        context_window: session.context_window,
    };
    *chat.session.lock().map_err(|e| HolziError::CrdtInit {
        reason: format!("chat.session mutex poisoned: {e}"),
    })? = Some(session);
    chat.set_model_ready(
        identity.vault_generation,
        identity.load_id,
        info.model_id.clone(),
        info.name.clone(),
    );
    emit_load_progress(
        app,
        identity.vault_generation,
        identity.load_id,
        model_id,
        &info.name,
        LoadPhase::Ready,
        None,
    );
    Ok(LoadOutcome::Loaded(info))
}

/// Loads a model (local catalog id or `<provider_id>:<remote_id>`) into
/// the active session. If another model was already loaded, it is dropped
/// first. The preload, when present, is cancelled before acquiring the
/// serialised manual-operation guard.
///
/// For a local model this runs the mandatory pre-load integrity check
/// (contracts/tauri-commands.md §"load_model und lokale Integritätsprüfung")
/// — a hash mismatch, missing file, missing expected hash, or hashing
/// error returns a structured `ModelIntegrity*` error instead of loading.
/// The frontend renders that as a decision dialog; `load_untrusted` there
/// calls [`load_model_with_integrity_override`] instead of retrying this
/// command.
#[tauri::command]
pub async fn load_model(
    app: AppHandle,
    state: State<'_, AppState>,
    chat: State<'_, ChatState>,
    model_id: String,
) -> Result<LoadedModelInfo> {
    load_model_command(app, state, chat, model_id, false).await
}

/// Explicit, confirmation-gated bypass of the integrity check above
/// (contracts/tauri-commands.md: the `load_untrusted` option of the
/// integrity dialog). Loads whatever file is currently on disk and marks
/// `integrityStatus = 'untrusted'`; `file_sha256` is left untouched, so
/// the next normal `load_model` call will ask again.
#[tauri::command]
pub async fn load_model_with_integrity_override(
    app: AppHandle,
    state: State<'_, AppState>,
    chat: State<'_, ChatState>,
    model_id: String,
) -> Result<LoadedModelInfo> {
    load_model_command(app, state, chat, model_id, true).await
}

async fn load_model_command(
    app: AppHandle,
    state: State<'_, AppState>,
    chat: State<'_, ChatState>,
    model_id: String,
    integrity_override: bool,
) -> Result<LoadedModelInfo> {
    chat.cancel_preload_and_wait().await;
    emit_model_load_status(&app, &chat);
    let _operation = chat.acquire_operation()?;
    let (vault_generation, load_id) = chat.begin_model_load();
    let identity = LoadIdentity {
        vault_generation,
        load_id,
    };
    match load_model_inner(
        &app,
        &state,
        &chat,
        &model_id,
        identity,
        None,
        integrity_override,
    )
    .await
    {
        Ok(LoadOutcome::Loaded(info)) => Ok(info),
        Ok(LoadOutcome::Cancelled) => Err(HolziError::InvalidInput {
            reason: "model load was cancelled".into(),
        }),
        Err(error) => {
            chat.set_model_error(
                vault_generation,
                load_id,
                Some(model_id.clone()),
                None,
                "model_load_failed".into(),
            );
            emit_load_error(
                &app,
                vault_generation,
                load_id,
                Some(model_id),
                None,
                "model_load_failed",
            );
            Err(error)
        }
    }
}

/// Returns the provider's display name for a `<provider_uuid>:...`
/// composite id. Best-effort — `None` when the row is missing.
async fn resolve_provider_name(
    state: &State<'_, AppState>,
    provider_id_str: &str,
) -> Option<String> {
    let provider_id = Uuid::parse_str(provider_id_str).ok()?;
    let db = active_database(state).ok()?;
    let name = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            Ok(providers_store::get_provider(conn, provider_id)
                .map_err(haex_crdt::Error::from)?
                .map(|p| p.name))
        })
    })
    .await
    .ok()?
    .ok()?;
    name
}

/// Picks the load-phase for a local model.
///
/// Heuristic: on a CUDA build without an existing NV compute cache we
/// emit `cuda-jit-warmup` (the ~30 s kernel-JIT window per Etappe-0
/// finding #4). On CPU/Metal builds we always emit `loading` — no
/// comparable warmup exists there (spec 002 §FR-015c).
#[cfg(feature = "llm-cpu")]
fn local_load_phase() -> LoadPhase {
    #[cfg(feature = "llm-cuda")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let cache = std::path::PathBuf::from(home).join(".nv/ComputeCache");
            let has_cache = std::fs::read_dir(&cache)
                .map(|mut it| it.next().is_some())
                .unwrap_or(false);
            if !has_cache {
                return LoadPhase::CudaJitWarmup;
            }
        } else {
            // No HOME → we cannot tell. Prefer the warmup label so the
            // user is not surprised by a long first-load without an
            // explanation.
            return LoadPhase::CudaJitWarmup;
        }
    }
    LoadPhase::Loading
}

/// Resolves a provider-qualified model id and builds its remote adapter session.
pub(crate) async fn load_api_key_model(
    state: &State<'_, AppState>,
    composite_id: &str,
    provider_id_str: &str,
) -> Result<ActiveSession> {
    let provider_id = Uuid::parse_str(provider_id_str).map_err(|_| HolziError::InvalidInput {
        reason: format!("bad composite model id: {composite_id}"),
    })?;
    let db = active_database(state)?;
    let id_owned = composite_id.to_string();
    let db_read = db.clone();
    let (provider, row) = tauri::async_runtime::spawn_blocking(move || {
        db_read.with_connection(|conn| {
            let provider = providers_store::get_provider(conn, provider_id)
                .map_err(haex_crdt::Error::from)?
                .ok_or_else(|| {
                    haex_crdt::Error::from(haex_crdt::rusqlite::Error::QueryReturnedNoRows)
                })?;
            let row = models_store::get_model(conn, &id_owned)
                .map_err(haex_crdt::Error::from)?
                .ok_or_else(|| {
                    haex_crdt::Error::from(haex_crdt::rusqlite::Error::QueryReturnedNoRows)
                })?;
            Ok((provider, row))
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("resolve api_key model join: {e}"),
    })?
    .map_err(|_| HolziError::ModelNotFound {
        id: composite_id.to_string(),
    })?;

    let provider = crate::providers::repair_legacy_adapter(&db, &provider).await?;
    let adapter = build_adapter(&provider)?;
    Ok(ActiveSession {
        model_id: composite_id.to_string(),
        provider_id: Some(provider_id),
        adapter: Arc::from(adapter),
        tokenizer_repo: String::new(),
        context_window: row.context_window,
    })
}

/// Loads an installed local model and wraps it in an adapter-backed session.
///
/// Resolves the on-disk file via [`paths::canonical_model_file`] rather
/// than a persisted `relative_path` — the filesystem is authoritative
/// for what is installed (spec 002 §"Installed model"). Metadata
/// (`context_window`, `tokenizer_repo`) still comes from the
/// synchronised `models` catalog row.
///
/// `integrity_override` is `true` only for the explicit
/// `load_model_with_integrity_override` path (contracts/tauri-commands.md
/// §"load_model und lokale Integritätsprüfung"): it skips the blocking
/// hash comparison and marks the row `untrusted` instead of `verified`,
/// but never skips the file-existence check and never rewrites
/// `file_sha256` — only a successful download/update/import may do that
/// (research.md Entscheidung 6).
#[cfg(feature = "llm-cpu")]
async fn resolve_local_model_metadata(
    app: &AppHandle,
    state: &State<'_, AppState>,
    model_id: &str,
    integrity_override: bool,
) -> Result<(std::path::PathBuf, Option<i64>, String)> {
    let canonical =
        paths::canonical_model_file(app, model_id)?.ok_or_else(|| HolziError::ModelNotFound {
            id: model_id.to_string(),
        })?;

    let db = active_database(state)?;
    let id_owned = model_id.to_string();
    let row = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            let row = models_store::get_model(conn, &id_owned)
                .map_err(haex_crdt::Error::from)?
                .ok_or_else(|| {
                    haex_crdt::Error::from(haex_crdt::rusqlite::Error::QueryReturnedNoRows)
                })?;
            Ok(row)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("resolve local model join: {e}"),
    })?
    .map_err(|_| HolziError::ModelNotFound {
        id: model_id.to_string(),
    })?;

    if integrity_override {
        // Persist the explicit safety decision before loading. If this
        // write fails, continuing would create a successful load that is
        // no longer visibly marked as untrusted.
        let db = active_database(state)?;
        let id_for_update = model_id.to_string();
        let status_update = tauri::async_runtime::spawn_blocking(move || {
            db.with_connection(|conn| {
                models_store::set_integrity_status(
                    conn,
                    &id_for_update,
                    models_store::IntegrityStatus::Untrusted,
                )
                .map_err(haex_crdt::Error::from)
            })
        })
        .await
        .map_err(|e| HolziError::ModelIntegrityError {
            model_id: model_id.to_string(),
            reason: format!("persisting untrusted status task join: {e}"),
        })?
        .map_err(|e| HolziError::ModelIntegrityError {
            model_id: model_id.to_string(),
            reason: format!("persisting untrusted status: {e}"),
        })?;
        if status_update == 0 {
            return Err(HolziError::ModelIntegrityError {
                model_id: model_id.to_string(),
                reason: "model row disappeared while marking it untrusted".into(),
            });
        }
    } else {
        verify_local_model_integrity(
            state,
            model_id,
            &canonical.absolute_path,
            row.file_sha256.as_deref(),
        )
        .await?;
    }

    // Prefer the persisted `tokenizer_repo` (migration 0009 onwards).
    // The catalog fallback covers pre-0009 rows the lazy backfill in
    // `list_installed_models` has not yet touched.
    let tokenizer_repo = row
        .tokenizer_repo
        .or_else(|| crate::catalog::get(model_id).map(|e| e.tokenizer_repo.clone()))
        .ok_or_else(|| HolziError::InvalidInput {
            reason: format!("tokenizer_repo missing for model {model_id}"),
        })?;

    Ok((canonical.absolute_path, row.context_window, tokenizer_repo))
}

/// Mandatory pre-load integrity gate for local models (contracts/
/// tauri-commands.md §"load_model und lokale Integritätsprüfung",
/// research.md Entscheidung 6). Recomputes the full SHA-256 of the
/// canonical file in a blocking task and compares it against
/// `models.file_sha256`. Only an exact match lets the normal load
/// proceed; every other outcome returns a structured error the frontend
/// renders as the integrity decision dialog (`load_untrusted` /
/// `repair_source` / `choose_other`).
#[cfg(feature = "llm-cpu")]
async fn verify_local_model_integrity(
    state: &State<'_, AppState>,
    model_id: &str,
    absolute_path: &std::path::Path,
    expected_sha256: Option<&str>,
) -> Result<()> {
    let Some(expected) = expected_sha256.filter(|s| crate::models::hash::is_valid_sha256_hex(s))
    else {
        return Err(HolziError::ModelIntegrityUnknown {
            model_id: model_id.to_string(),
            expected_sha256: expected_sha256.map(str::to_string),
        });
    };

    let path_owned = absolute_path.to_path_buf();
    let hash_result =
        tauri::async_runtime::spawn_blocking(move || crate::models::hash::sha256_file(&path_owned))
            .await
            .map_err(|e| HolziError::ModelIntegrityError {
                model_id: model_id.to_string(),
                reason: format!("hashing task join: {e}"),
            })?;

    let actual = hash_result.map_err(|e| HolziError::ModelIntegrityError {
        model_id: model_id.to_string(),
        reason: e.to_string(),
    })?;

    if actual != expected {
        // Keep the persisted status aligned with the failed check. This is
        // best-effort because the mismatch itself must remain the reported
        // error and the expected file hash must never be rewritten here.
        if let Ok(db) = active_database(state) {
            let id_for_update = model_id.to_string();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                db.with_connection(|conn| {
                    models_store::set_integrity_status(
                        conn,
                        &id_for_update,
                        models_store::IntegrityStatus::Untrusted,
                    )
                    .map_err(haex_crdt::Error::from)
                })
            })
            .await;
        }
        return Err(HolziError::ModelIntegrityMismatch {
            model_id: model_id.to_string(),
            expected_sha256: expected.to_string(),
            actual_sha256: actual,
        });
    }

    // Match — record the confirmation so the model list reflects the
    // latest check. Never touches `file_sha256` itself (Entscheidung 6).
    // Best-effort: a failure here must not turn a successful integrity
    // check into a blocked load.
    if let Ok(db) = active_database(state) {
        let id_for_update = model_id.to_string();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            db.with_connection(|conn| {
                models_store::set_integrity_status(
                    conn,
                    &id_for_update,
                    models_store::IntegrityStatus::Verified,
                )
                .map_err(haex_crdt::Error::from)
            })
        })
        .await;
    }
    Ok(())
}

#[cfg(feature = "llm-cpu")]
async fn load_local_model_from_metadata(
    model_id: &str,
    (path, context_window, tokenizer_repo): (std::path::PathBuf, Option<i64>, String),
) -> Result<ActiveSession> {
    let model = LocalModel::load(&path, Some(&tokenizer_repo))
        .await
        .map_err(|e| HolziError::ModelDownload {
            reason: format!("mistralrs load: {e}"),
        })?;

    Ok(ActiveSession {
        model_id: model_id.to_string(),
        provider_id: None,
        adapter: Arc::new(LocalAdapter::new(model)),
        tokenizer_repo,
        context_window,
    })
}

/// Returns the cached display name for a model when its row can be read.
pub(crate) async fn resolve_display_name(
    state: &State<'_, AppState>,
    model_id: &str,
) -> Option<String> {
    let db = active_database(state).ok()?;
    let id_owned = model_id.to_string();
    let name = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            Ok(models_store::get_model(conn, &id_owned)
                .map_err(haex_crdt::Error::from)?
                .map(|r| r.name))
        })
    })
    .await
    .ok()?
    .ok()?;
    name
}

/// Drops the current session, if any. Idempotent.
#[tauri::command]
pub async fn unload_local_model(app: AppHandle, chat: State<'_, ChatState>) -> Result<()> {
    chat.cancel_preload_and_wait().await;
    emit_model_load_status(&app, &chat);
    let _operation = chat.acquire_operation()?;
    let mut guard = chat.session.lock().map_err(|e| HolziError::CrdtInit {
        reason: format!("chat.session mutex poisoned: {e}"),
    })?;
    *guard = None;
    Ok(())
}

/// Introspection — `None` when no model is loaded.
#[tauri::command]
pub async fn active_model_info(chat: State<'_, ChatState>) -> Result<Option<LoadedModelInfo>> {
    let guard = chat.session.lock().map_err(|e| HolziError::CrdtInit {
        reason: format!("chat.session mutex poisoned: {e}"),
    })?;
    Ok(guard.as_ref().map(|s| LoadedModelInfo {
        model_id: s.model_id.clone(),
        name: s.model_id.clone(),
        tokenizer_repo: s.tokenizer_repo.clone(),
        context_window: s.context_window,
    }))
}
