//! Model-lifecycle + streaming chat commands.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[cfg(feature = "llm-cpu")]
use crate::adapters::local::LocalAdapter;
use crate::adapters::types::{ChatMessage as LlmMessage, ChatRequest, ChatRole, StreamChunk};
use crate::error::{HolziError, Result};
#[cfg(feature = "llm-cpu")]
use crate::llm::local::LocalModel;
use crate::models::paths;
use crate::providers::build_adapter;
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::{
    chat_messages::{self as msg_store, ChatMessage, FinishReason, MessageRole},
    chat_threads::{self as thread_store, ChatThread},
    models as models_store, preferences,
    preferences::PrefScope,
    providers as providers_store,
};

use super::session::{ActiveSession, ChatState};

const EVENT_CHAT_TOKEN: &str = "chat-token";
const EVENT_CHAT_MESSAGE_COMPLETE: &str = "chat-message-complete";
const EVENT_CHAT_MESSAGE_ERROR: &str = "chat-message-error";
const EVENT_MODEL_LOAD_PROGRESS: &str = "model-load-progress";

const PREF_LAST_ACTIVE_MODEL: &str = "chat.last_active_model_id";
const PREF_DEFAULT_MODEL: &str = "chat.default_model_id";

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
enum LoadPhase {
    Connecting,
    Loading,
    CudaJitWarmup,
    Ready,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelLoadProgress {
    model_id: String,
    model_name: String,
    phase: LoadPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_name: Option<String>,
}

/// Emits a `model-load-progress` event for the given phase. Frontend
/// translates the label via `$t('chat.loading.<phase>', ...)` and
/// gates the chat input on `phase === 'ready'`.
fn emit_load_progress(
    app: &AppHandle,
    model_id: &str,
    model_name: &str,
    phase: LoadPhase,
    provider_name: Option<String>,
) {
    let _ = app.emit(
        EVENT_MODEL_LOAD_PROGRESS,
        ModelLoadProgress {
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            phase,
            provider_name,
        },
    );
}

/// Which fallback branch the session resolver picked. Wire payload for
/// `resolve_default_model`.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolveSource {
    LastActive,
    DefaultDevice,
    DefaultVault,
    FirstAvailable,
    None,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveDefaultModelResult {
    pub model_id: Option<String>,
    pub source: ResolveSource,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageArgs {
    /// If `None`, a new thread is created and its id is returned.
    pub thread_id: Option<Uuid>,
    pub content: String,
    /// Optional system prompt applied for this generation. Not
    /// persisted; a persistent system prompt is a later slice.
    pub system_prompt: Option<String>,
    /// Cap on generated tokens. `None` uses the adapter default.
    pub max_new_tokens: Option<usize>,
    /// Stable across every retry of the same user send (contract
    /// §send_message). Non-empty; deduplicates a frontend retry of its
    /// own `invoke()` call without resuming a failed generation — see
    /// [`resolve_idempotent_send`].
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResult {
    pub thread_id: Uuid,
    pub user_message_id: Uuid,
    pub assistant_message_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenEvent {
    message_id: Uuid,
    delta: String,
    /// Reasoning-content delta from Harmony-format local models or
    /// Anthropic `thinking_delta` events. `None` when the chunk has
    /// no reasoning.
    reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageCompleteEvent {
    message_id: Uuid,
    thread_id: Uuid,
    prompt_tokens: Option<usize>,
    completion_tokens: Option<usize>,
    ttft_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageErrorEvent {
    message_id: Uuid,
    thread_id: Uuid,
    reason: String,
}

/// Loads a model (local catalog id or `<provider_id>:<remote_id>`) into
/// the active session. If another model was already loaded, it is
/// dropped first.
///
/// Emits `model-load-progress` events with a structured payload
/// (`{ modelId, modelName, phase, providerName? }`) around the load:
/// `connecting` for api_key providers, `loading` or `cuda-jit-warmup`
/// for local models, `ready` on success. Frontend translates the
/// labels via `$t('chat.loading.<phase>', …)`.
///
/// Never writes `chat.last_active_model_id` — that preference is
/// touched exclusively by a successful `send_message` (spec 002
/// §FR-009 post-clarify correction).
#[tauri::command]
pub async fn load_model(
    app: AppHandle,
    state: State<'_, AppState>,
    chat: State<'_, ChatState>,
    model_id: String,
) -> Result<LoadedModelInfo> {
    // Pre-fetch display metadata for the progress payload so the
    // frontend does not have to look it up separately per event.
    let name = resolve_display_name(&state, &model_id)
        .await
        .unwrap_or_else(|| model_id.clone());

    let session = if let Some((provider_id_str, _remote_id)) = model_id.split_once(':') {
        let provider_name = resolve_provider_name(&state, provider_id_str).await;
        emit_load_progress(&app, &model_id, &name, LoadPhase::Connecting, provider_name);
        load_api_key_model(&state, &model_id, provider_id_str).await?
    } else {
        #[cfg(feature = "llm-cpu")]
        {
            let phase = local_load_phase();
            emit_load_progress(&app, &model_id, &name, phase, None);
            load_local_model_by_id(&app, &state, &model_id).await?
        }
        #[cfg(not(feature = "llm-cpu"))]
        {
            return Err(HolziError::InvalidInput {
                reason: "local inference is not enabled in this build".into(),
            });
        }
    };

    let info = LoadedModelInfo {
        model_id: session.model_id.clone(),
        name,
        tokenizer_repo: session.tokenizer_repo.clone(),
        context_window: session.context_window,
    };

    let mut guard = chat.session.lock().map_err(|e| HolziError::CrdtInit {
        reason: format!("chat.session mutex poisoned: {e}"),
    })?;
    *guard = Some(session);
    drop(guard);
    emit_load_progress(&app, &model_id, &info.name, LoadPhase::Ready, None);
    Ok(info)
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
async fn load_api_key_model(
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
#[cfg(feature = "llm-cpu")]
async fn load_local_model_by_id(
    app: &AppHandle,
    state: &State<'_, AppState>,
    model_id: &str,
) -> Result<ActiveSession> {
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
            Ok((row.context_window, row.tokenizer_repo))
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("resolve local model join: {e}"),
    })?
    .map_err(|_| HolziError::ModelNotFound {
        id: model_id.to_string(),
    })?;
    let (context_window, db_tokenizer_repo) = row;

    // Prefer the persisted `tokenizer_repo` (migration 0009 onwards).
    // The catalog fallback covers pre-0009 rows the lazy backfill in
    // `list_installed_models` has not yet touched.
    let tokenizer_repo = db_tokenizer_repo
        .or_else(|| crate::catalog::get(model_id).map(|e| e.tokenizer_repo.clone()))
        .ok_or_else(|| HolziError::InvalidInput {
            reason: format!("tokenizer_repo missing for model {model_id}"),
        })?;

    let model = LocalModel::load(&canonical.absolute_path, Some(&tokenizer_repo))
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
async fn resolve_display_name(state: &State<'_, AppState>, model_id: &str) -> Option<String> {
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
pub async fn unload_local_model(chat: State<'_, ChatState>) -> Result<()> {
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

/// Deterministically derives the (user, assistant) message ids for a
/// `send_message` call from its `idempotencyKey`. The same key always
/// yields the same pair, so a retried `invoke()` can be recognised and
/// answered with the original ids without any extra state — no cache,
/// no second column (contract §send_message: "die zugehörigen
/// User-/Assistant-IDs werden aus diesem Send-Vorgang wiederverwendet").
pub fn derive_message_ids(idempotency_key: &str) -> (Uuid, Uuid) {
    let user_message_id = Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("user:{idempotency_key}").as_bytes(),
    );
    let assistant_message_id = Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("assistant:{idempotency_key}").as_bytes(),
    );
    (user_message_id, assistant_message_id)
}

/// Outcome of deduping a `send_message` call against any prior send
/// sharing the same `idempotencyKey`. This only guards against the
/// frontend retrying its own uncertain `invoke()` call — it never
/// resumes or restarts a generation that failed after the user message
/// was accepted (see contracts/tauri-commands.md §send_message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotentSend {
    /// No prior send used this key — proceed with a normal insert
    /// using the given (deterministic) ids.
    Fresh {
        user_message_id: Uuid,
        assistant_message_id: Uuid,
    },
    /// A prior send with this key already exists and matches the
    /// requested thread/content — reuse its ids; insert nothing, start
    /// no new stream.
    Duplicate {
        thread_id: Uuid,
        user_message_id: Uuid,
        assistant_message_id: Uuid,
    },
    /// A prior send with this key exists but its thread or content
    /// differs from this request — the caller must reject with
    /// `HolziError::InvalidInput`.
    Mismatch,
}

/// Resolves a `send_message` call's idempotency against the DB.
/// `requested_thread_id` is the caller's raw `args.thread_id`, not yet
/// resolved to a real thread: `None` means "any thread", since a
/// retried call may not know the thread a prior, possibly
/// unacknowledged, attempt created.
pub fn resolve_idempotent_send(
    conn: &haex_crdt::rusqlite::Connection,
    idempotency_key: &str,
    requested_thread_id: Option<Uuid>,
    content: &str,
) -> haex_crdt::rusqlite::Result<IdempotentSend> {
    let (user_message_id, assistant_message_id) = derive_message_ids(idempotency_key);
    let Some(existing) = msg_store::find_by_idempotency_key(conn, idempotency_key)? else {
        return Ok(IdempotentSend::Fresh {
            user_message_id,
            assistant_message_id,
        });
    };
    let thread_matches = requested_thread_id.map_or(true, |t| t == existing.thread_id);
    if !thread_matches || existing.content != content {
        return Ok(IdempotentSend::Mismatch);
    }
    // Rows written before the role-separated derivation used the raw key for
    // the user id and `{key}:assistant` for the assistant id. Keep returning
    // that pair for those rows so a retry remains byte-for-byte compatible.
    let legacy_user_message_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, idempotency_key.as_bytes());
    let assistant_message_id = if existing.id == legacy_user_message_id {
        Uuid::new_v5(
            &Uuid::NAMESPACE_OID,
            format!("{idempotency_key}:assistant").as_bytes(),
        )
    } else {
        assistant_message_id
    };
    Ok(IdempotentSend::Duplicate {
        thread_id: existing.thread_id,
        user_message_id: existing.id,
        assistant_message_id,
    })
}

/// Result of atomically resolving and persisting a fresh send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedSend {
    Fresh {
        thread_id: Uuid,
        user_message_id: Uuid,
        assistant_message_id: Uuid,
    },
    Duplicate {
        thread_id: Uuid,
        user_message_id: Uuid,
        assistant_message_id: Uuid,
    },
    Mismatch,
}

/// Resolves, reserves, and persists a send in one SQLite transaction.
///
/// `BEGIN IMMEDIATE` closes the gap between the idempotency lookup and the
/// unique-key insert. A concurrent loser therefore re-reads the committed
/// winner and receives the same result instead of a primary-key/unique error.
pub fn persist_send_transaction(
    conn: &haex_crdt::rusqlite::Connection,
    idempotency_key: &str,
    requested_thread_id: Option<Uuid>,
    content: &str,
    provider_id: Option<Uuid>,
    model_id: &str,
    now: i64,
) -> haex_crdt::rusqlite::Result<PersistedSend> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let decision =
            resolve_idempotent_send(conn, idempotency_key, requested_thread_id, content)?;
        match decision {
            IdempotentSend::Mismatch => Ok(PersistedSend::Mismatch),
            IdempotentSend::Duplicate {
                thread_id,
                user_message_id,
                assistant_message_id,
            } => Ok(PersistedSend::Duplicate {
                thread_id,
                user_message_id,
                assistant_message_id,
            }),
            IdempotentSend::Fresh {
                user_message_id,
                assistant_message_id,
            } => {
                let thread_id = requested_thread_id.unwrap_or_else(Uuid::new_v4);
                if requested_thread_id.is_none() {
                    thread_store::insert_thread(
                        conn,
                        &ChatThread {
                            id: thread_id,
                            title: default_thread_title(content),
                            last_provider_id: provider_id,
                            last_model_id: Some(model_id.to_string()),
                            created_at: now,
                            updated_at: now,
                        },
                    )?;
                } else {
                    thread_store::update_thread(
                        conn,
                        thread_id,
                        &current_title(conn, thread_id).unwrap_or_default(),
                        provider_id,
                        Some(model_id),
                        now,
                    )?;
                }
                let parent_id = last_message_id(conn, thread_id)?;
                msg_store::insert_message(
                    conn,
                    &ChatMessage {
                        id: user_message_id,
                        thread_id,
                        parent_id,
                        role: MessageRole::User,
                        content: content.to_string(),
                        provider_id,
                        model_id: Some(model_id.to_string()),
                        prompt_tokens: None,
                        completion_tokens: None,
                        finish_reason: Some(FinishReason::Complete),
                        created_at: now,
                        idempotency_key: Some(idempotency_key.to_string()),
                    },
                )?;
                Ok(PersistedSend::Fresh {
                    thread_id,
                    user_message_id,
                    assistant_message_id,
                })
            }
        }
    })();
    match result {
        Ok(result) => match conn.execute_batch("COMMIT") {
            Ok(()) => Ok(result),
            Err(error) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(error)
            }
        },
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

/// Persists a user message, spawns a streaming generation, returns
/// both message ids so the frontend can subscribe. The assistant
/// message is inserted on completion.
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    chat: State<'_, ChatState>,
    args: SendMessageArgs,
) -> Result<SendMessageResult> {
    if args.idempotency_key.trim().is_empty() {
        return Err(HolziError::InvalidInput {
            reason: "idempotencyKey must not be empty".into(),
        });
    }

    let db = active_database(&state)?;

    // Dedup against any prior send sharing this idempotencyKey before
    // minting anything new (contract §send_message). This only guards
    // the frontend retrying its own uncertain `invoke()` call — it
    // never resumes a generation that failed after acceptance.
    let dedup_db = db.clone();
    let dedup_key = args.idempotency_key.clone();
    let dedup_thread_id = args.thread_id;
    let dedup_content = args.content.clone();
    let decision = tauri::async_runtime::spawn_blocking(move || {
        dedup_db.with_connection(|conn| {
            resolve_idempotent_send(conn, &dedup_key, dedup_thread_id, &dedup_content)
                .map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("resolve_idempotent_send join: {e}"),
    })?
    .map_err(HolziError::from)?;

    match decision {
        IdempotentSend::Mismatch => {
            return Err(HolziError::InvalidInput {
                reason: "idempotencyKey already used with a different thread or content".into(),
            });
        }
        IdempotentSend::Duplicate {
            thread_id,
            user_message_id,
            assistant_message_id,
        } => {
            return Ok(SendMessageResult {
                thread_id,
                user_message_id,
                assistant_message_id,
            });
        }
        IdempotentSend::Fresh { .. } => {}
    }

    let session = {
        let guard = chat.session.lock().map_err(|e| HolziError::CrdtInit {
            reason: format!("chat.session mutex poisoned: {e}"),
        })?;
        guard.clone().ok_or_else(|| HolziError::InvalidInput {
            reason: "no model loaded".into(),
        })?
    };

    let persist_key = args.idempotency_key.clone();
    let persist_thread_id = args.thread_id;
    let persist_content = args.content.clone();
    let persist_provider_id = session.provider_id;
    let persist_model_id = session.model_id.clone();
    let persist_db = db.clone();
    let persisted = tauri::async_runtime::spawn_blocking(move || {
        persist_db.with_connection(|conn| {
            persist_send_transaction(
                conn,
                &persist_key,
                persist_thread_id,
                &persist_content,
                persist_provider_id,
                &persist_model_id,
                now_ms(),
            )
            .map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("persist send join: {e}"),
    })?
    .map_err(HolziError::from)?;

    let (thread_id, user_message_id, assistant_message_id) = match persisted {
        PersistedSend::Mismatch => {
            return Err(HolziError::InvalidInput {
                reason: "idempotencyKey already used with a different thread or content".into(),
            });
        }
        PersistedSend::Duplicate {
            thread_id,
            user_message_id,
            assistant_message_id,
        } => {
            return Ok(SendMessageResult {
                thread_id,
                user_message_id,
                assistant_message_id,
            });
        }
        PersistedSend::Fresh {
            thread_id,
            user_message_id,
            assistant_message_id,
        } => (thread_id, user_message_id, assistant_message_id),
    };

    // The thread and user message were persisted atomically above.
    // Spec 002 §FR-009: `chat.last_active_model_id` is written
    // EXCLUSIVELY here, never by `load_model` — the message is the user's
    // intent signal, a picker-only click is not. The preference is
    // committed after adapter startup succeeds so a failed send cannot
    // roll back a newer overlapping send's value.
    let session_model_id = session.model_id.clone();
    let this_device = db.device_id();
    let preference_model_id = session_model_id.clone();
    let is_new_thread = args.thread_id.is_none();

    // Build the request from full history + the just-inserted user turn.
    let history_db = db.clone();
    let history = tauri::async_runtime::spawn_blocking(move || {
        history_db.with_connection(|conn| {
            let msgs = msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from)?;
            Ok(msgs)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("history load join: {e}"),
    })?
    .map_err(HolziError::from)?;

    // For api_key models the adapter needs the raw remote id, not the
    // composite one — split it off here so the adapter stays vendor-
    // scoped and does not know about holzi's composite scheme.
    let request_model_id = session
        .model_id
        .split_once(':')
        .map(|(_, remote)| remote.to_string())
        .unwrap_or_else(|| session.model_id.clone());

    let request = ChatRequest {
        model_id: request_model_id,
        system_prompt: args.system_prompt.clone(),
        messages: history
            .iter()
            .filter_map(|m| match m.role {
                MessageRole::User => Some(LlmMessage {
                    role: ChatRole::User,
                    content: m.content.clone(),
                }),
                MessageRole::Assistant => Some(LlmMessage {
                    role: ChatRole::Assistant,
                    content: m.content.clone(),
                }),
                MessageRole::System => None,
            })
            .collect(),
        max_new_tokens: args.max_new_tokens,
    };

    // Start the generation. `stream_chat` may fail before the first
    // byte (credential rejection, transport error); surface those to
    // the caller instead of hiding them inside the streaming task.
    let mut stream = match session.adapter.stream_chat(request).await {
        Ok(stream) => stream,
        Err(error) => {
            let cleanup_db = db.clone();
            let cleanup = tauri::async_runtime::spawn_blocking(move || {
                cleanup_db.with_connection(|conn| {
                    msg_store::delete_message(conn, user_message_id)
                        .map_err(haex_crdt::Error::from)?;
                    if is_new_thread {
                        thread_store::delete_thread(conn, thread_id)
                            .map_err(haex_crdt::Error::from)?;
                    }
                    Ok(())
                })
            })
            .await;
            match cleanup {
                Ok(Ok(())) => {}
                Ok(Err(cleanup_error)) => {
                    return Err(HolziError::CrdtInit {
                        reason: format!(
                            "adapter start: {error}; failed to clean up staged message or thread: {cleanup_error}"
                        ),
                    });
                }
                Err(cleanup_error) => {
                    return Err(HolziError::CrdtInit {
                        reason: format!(
                            "adapter start: {error}; failed to clean up staged message or thread: {cleanup_error}"
                        ),
                    });
                }
            }
            return Err(HolziError::InvalidInput {
                reason: format!("adapter start: {error}"),
            });
        }
    };

    let preference_db = db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        preference_db.with_connection(|conn| {
            preferences::insert_or_update(
                conn,
                PrefScope::Device(this_device),
                PREF_LAST_ACTIVE_MODEL,
                &preference_model_id,
            )
            .map(|_| ())
            .map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("persist last active model join: {e}"),
    })?
    .map_err(HolziError::from)?;

    let abort = stream.abort_handle();
    {
        let mut g = chat
            .current_generation
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("chat.current_generation mutex poisoned: {e}"),
            })?;
        if let Some(prev) = g.take() {
            prev.abort();
        }
        *g = Some(abort);
    }

    let app_for_task = app.clone();
    let session_for_task = session.clone();
    let assistant_db = db.clone();

    tauri::async_runtime::spawn(async move {
        let mut assembled = String::new();
        let mut prompt_tokens: Option<usize> = None;
        let mut completion_tokens: Option<usize> = None;
        let mut ttft_ms: Option<u64> = None;
        let mut error_reason: Option<String> = None;
        let mut saw_done = false;

        while let Some(item) = stream.next().await {
            match item {
                Ok(StreamChunk::Delta { content, reasoning }) => {
                    if !content.is_empty() || reasoning.is_some() {
                        assembled.push_str(&content);
                        let _ = app_for_task.emit(
                            EVENT_CHAT_TOKEN,
                            TokenEvent {
                                message_id: assistant_message_id,
                                delta: content,
                                reasoning,
                            },
                        );
                    }
                }
                Ok(StreamChunk::Done {
                    prompt_tokens: pt,
                    completion_tokens: ct,
                    ttft_ms: t,
                    ..
                }) => {
                    saw_done = true;
                    prompt_tokens = pt;
                    completion_tokens = ct;
                    ttft_ms = t;
                    break;
                }
                Err(e) => {
                    error_reason = Some(e.to_string());
                    break;
                }
            }
        }

        let finish_reason = if error_reason.is_some() {
            FinishReason::Error
        } else if saw_done {
            FinishReason::Complete
        } else {
            FinishReason::Cancelled
        };
        let now2 = now_ms();
        let final_content = assembled.clone();
        let insert_result = tauri::async_runtime::spawn_blocking(move || {
            assistant_db.with_connection(|conn| {
                let msg = ChatMessage {
                    id: assistant_message_id,
                    thread_id,
                    parent_id: Some(user_message_id),
                    role: MessageRole::Assistant,
                    content: final_content,
                    provider_id: session_for_task.provider_id,
                    model_id: Some(session_for_task.model_id.clone()),
                    prompt_tokens: prompt_tokens.map(|n| n as i64),
                    completion_tokens: completion_tokens.map(|n| n as i64),
                    finish_reason: Some(finish_reason),
                    created_at: now2,
                    idempotency_key: None,
                };
                msg_store::insert_message(conn, &msg).map_err(haex_crdt::Error::from)?;
                thread_store::update_thread(
                    conn,
                    thread_id,
                    &current_title(conn, thread_id).unwrap_or_default(),
                    session_for_task.provider_id,
                    Some(&session_for_task.model_id),
                    now2,
                )
                .map_err(haex_crdt::Error::from)?;
                Ok(())
            })
        })
        .await;

        let persist_error = match insert_result {
            Err(e) => Some(format!("assistant persist join: {e}")),
            Ok(Err(e)) => Some(format!("assistant persist failed: {e}")),
            Ok(Ok(())) => None,
        };
        if let Some(reason) = persist_error {
            let _ = app_for_task.emit(
                EVENT_CHAT_MESSAGE_ERROR,
                MessageErrorEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    reason,
                },
            );
            return;
        }
        if let Some(reason) = error_reason {
            let _ = app_for_task.emit(
                EVENT_CHAT_MESSAGE_ERROR,
                MessageErrorEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    reason,
                },
            );
        } else {
            let _ = app_for_task.emit(
                EVENT_CHAT_MESSAGE_COMPLETE,
                MessageCompleteEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    prompt_tokens,
                    completion_tokens,
                    ttft_ms,
                },
            );
        }
    });

    Ok(SendMessageResult {
        thread_id,
        user_message_id,
        assistant_message_id,
    })
}

/// Runs the session-start resolver chain (spec 002 §FR-014) and
/// returns which model the caller should load, plus the branch that
/// picked it. Pure read — no preferences are written along the chain.
/// If the returned `source` is `FirstAvailable`, the caller MUST NOT
/// echo the id back into `chat.last_active_model_id`; the passive
/// selection is a "temporary choice for this session" (FR-015).
#[tauri::command]
pub async fn resolve_default_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ResolveDefaultModelResult> {
    let db = active_database(&state)?;
    let this_device = db.device_id();
    let db_clone = db.clone();
    let (result, installed_local_ids, api_key_ids) =
        tauri::async_runtime::spawn_blocking(move || {
            db_clone.with_connection(|conn| {
                // 1. last_active on this device.
                let last_active =
                    preferences::get(conn, PrefScope::Device(this_device), PREF_LAST_ACTIVE_MODEL)
                        .map_err(haex_crdt::Error::from)?;
                // 2. default on this device.
                let default_device =
                    preferences::get(conn, PrefScope::Device(this_device), PREF_DEFAULT_MODEL)
                        .map_err(haex_crdt::Error::from)?;
                // 3. default vault-wide.
                let default_vault = preferences::get(conn, PrefScope::Vault, PREF_DEFAULT_MODEL)
                    .map_err(haex_crdt::Error::from)?;
                // 4. Everything on this device that could load.
                let all_models =
                    models_store::list_all_models(conn).map_err(haex_crdt::Error::from)?;
                let mut installed_local = Vec::new();
                let mut api_key_ids = Vec::new();
                for row in &all_models {
                    if row.id.contains(':') {
                        api_key_ids.push(row.id.clone());
                    } else {
                        // For local models the loadability check is
                        // "canonical file exists". We do that outside
                        // the DB call so `paths::canonical_model_file`
                        // can use the AppHandle.
                        installed_local.push(row.id.clone());
                    }
                }
                let candidate_pref = |value: Option<String>| value.filter(|s| !s.is_empty());
                Ok::<_, haex_crdt::Error>((
                    (
                        candidate_pref(last_active),
                        candidate_pref(default_device),
                        candidate_pref(default_vault),
                    ),
                    installed_local,
                    api_key_ids,
                ))
            })
        })
        .await
        .map_err(|e| HolziError::CrdtInit {
            reason: format!("resolve_default_model join: {e}"),
        })?
        .map_err(HolziError::from)?;
    let (last_active, default_device, default_vault) = result;

    // Validate API-key candidates through the same provider-row, model-row,
    // credential, legacy-repair, and adapter checks used by load_model.
    let mut loadable_api_key_ids = Vec::new();
    for composite_id in api_key_ids {
        let Some((provider_id_str, _)) = composite_id.split_once(':') else {
            continue;
        };
        if load_api_key_model(&state, &composite_id, provider_id_str)
            .await
            .is_ok()
        {
            loadable_api_key_ids.push(composite_id);
        }
    }

    // Filter installed_local down to those that actually have a
    // canonical file on disk right now.
    let mut loadable_local: Vec<String> = Vec::new();
    for slug in installed_local_ids {
        if let Ok(Some(_)) = paths::canonical_model_file(&app, &slug) {
            loadable_local.push(slug);
        }
    }
    let is_loadable = |id: &str| -> bool {
        loadable_api_key_ids.iter().any(|s| s == id) || loadable_local.iter().any(|s| s == id)
    };

    if let Some(id) = last_active.filter(|id| is_loadable(id)) {
        return Ok(ResolveDefaultModelResult {
            model_id: Some(id),
            source: ResolveSource::LastActive,
        });
    }
    if let Some(id) = default_device.filter(|id| is_loadable(id)) {
        return Ok(ResolveDefaultModelResult {
            model_id: Some(id),
            source: ResolveSource::DefaultDevice,
        });
    }
    if let Some(id) = default_vault.filter(|id| is_loadable(id)) {
        return Ok(ResolveDefaultModelResult {
            model_id: Some(id),
            source: ResolveSource::DefaultVault,
        });
    }
    // First-available: prefer a local model (no network), else any
    // api_key row. Deterministic within each bucket (models are
    // ordered by name; loadable_local mirrors that order).
    if let Some(id) = loadable_local.into_iter().next() {
        return Ok(ResolveDefaultModelResult {
            model_id: Some(id),
            source: ResolveSource::FirstAvailable,
        });
    }
    if let Some(id) = loadable_api_key_ids.into_iter().next() {
        return Ok(ResolveDefaultModelResult {
            model_id: Some(id),
            source: ResolveSource::FirstAvailable,
        });
    }
    Ok(ResolveDefaultModelResult {
        model_id: None,
        source: ResolveSource::None,
    })
}

/// Cancels the in-flight generation, if any. Idempotent — safe to
/// call when nothing is running.
#[tauri::command]
pub async fn abort_current_generation(chat: State<'_, ChatState>) -> Result<()> {
    let mut guard = chat
        .current_generation
        .lock()
        .map_err(|e| HolziError::CrdtInit {
            reason: format!("chat.current_generation mutex poisoned: {e}"),
        })?;
    if let Some(abort) = guard.take() {
        abort.abort();
    }
    Ok(())
}

fn default_thread_title(first_message: &str) -> String {
    let trimmed = first_message.trim();
    let s: String = trimmed.chars().take(60).collect();
    if s.is_empty() {
        "New chat".to_string()
    } else {
        s
    }
}

fn current_title(conn: &haex_crdt::rusqlite::Connection, thread_id: Uuid) -> Option<String> {
    let mut stmt = conn
        .prepare("SELECT title FROM chat_threads WHERE id = ?1")
        .ok()?;
    stmt.query_row(haex_crdt::rusqlite::params![thread_id.to_string()], |r| {
        r.get::<_, String>(0)
    })
    .ok()
}

fn last_message_id(
    conn: &haex_crdt::rusqlite::Connection,
    thread_id: Uuid,
) -> haex_crdt::rusqlite::Result<Option<Uuid>> {
    let msgs = msg_store::list_messages(conn, thread_id)?;
    Ok(msgs.last().map(|m| m.id))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
