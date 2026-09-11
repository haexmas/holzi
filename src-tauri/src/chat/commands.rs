//! Model-lifecycle + streaming chat commands.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

#[cfg(feature = "llm-cpu")]
use crate::adapters::local::LocalAdapter;
use crate::adapters::types::{
    ChatMessage as LlmMessage, ChatRequest, ChatRole, StreamChunk, ToolCall as LlmToolCall,
    ToolSpec,
};
use crate::chat::tools::permission::{self, PermissionMode};
use crate::chat::tools::{ApprovalDecision, Tool, ToolRegistry, ToolResult as ToolExecResult};
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
const EVENT_CHAT_TOOL_CALL: &str = "chat-tool-call";
const EVENT_CHAT_TOOL_RESULT: &str = "chat-tool-result";
const EVENT_CHAT_TURN_COMPLETE: &str = "chat-turn-complete";
const EVENT_TOOL_PERMISSION_REQUEST: &str = "tool-permission-request";

const PREF_LAST_ACTIVE_MODEL: &str = "chat.last_active_model_id";
const PREF_DEFAULT_MODEL: &str = "chat.default_model_id";
/// Device preference read fresh before every tool call (T024B); parsed via
/// `PermissionMode::parse`, defaulting to `Manual` when unset or invalid
/// (spec.md Assumptions). No dedicated get/set command — read/written
/// through the existing generic `get_pref`/`set_pref` (data-model.md).
const PREF_PERMISSION_MODE: &str = "chat.permission_mode";

/// Fixed cap on the number of tool-calling rounds within one turn
/// (spec.md FR-016). A "round" is one step whose response contained at
/// least one tool call. Reaching the cap ends the turn with
/// `FinishReason::ToolLimitReached` instead of issuing a further step.
pub const MAX_TOOL_ROUNDS: usize = 8;

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

/// Payload for `chat-tool-call` (contracts/tauri-commands.md). Emitted at
/// the persistence boundary — a `tool_call` row exists from this point,
/// even if the call is later blocked under `plan` mode (Phase 4).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCallEvent {
    message_id: Uuid,
    thread_id: Uuid,
    tool_name: String,
    tool_input: Value,
    tool_source: String,
}

/// Payload for `chat-tool-result` (contracts/tauri-commands.md).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolResultEvent {
    message_id: Uuid,
    thread_id: Uuid,
    tool_call_id: String,
    content: String,
    is_error: bool,
}

/// Payload for `tool-permission-request` (contracts/tauri-commands.md).
/// Answered via `respond_tool_permission`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolPermissionRequestEvent {
    request_id: Uuid,
    thread_id: Uuid,
    tool_name: String,
    tool_input: Value,
    risk_class: &'static str,
}

fn risk_class_str(risk: crate::chat::tools::RiskClass) -> &'static str {
    match risk {
        crate::chat::tools::RiskClass::Safe => "safe",
        crate::chat::tools::RiskClass::Risky => "risky",
    }
}

/// Payload for `chat-turn-complete` (contracts/tauri-commands.md). Fires
/// exactly once per `send_message` call, after the last step's own
/// per-step event — this is the frontend's sole signal to clear
/// `streamingMessageId`/`busy`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TurnCompleteEvent {
    thread_id: Uuid,
    assistant_message_id: Option<Uuid>,
    finish_reason: FinishReason,
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
                        tool_name: None,
                        tool_call_id: None,
                        tool_input: None,
                        tool_is_error: None,
                        tool_source: None,
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

/// Converts persisted history rows into the adapter-neutral message shape
/// an adapter groups into its own wire format (data-model.md). `System`
/// rows are dropped — `system_prompt` carries system content out of band.
fn history_to_messages(history: &[ChatMessage]) -> Vec<LlmMessage> {
    history
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
            MessageRole::ToolCall => {
                let input = m
                    .tool_input
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(Value::Object(Default::default()));
                Some(LlmMessage {
                    role: ChatRole::ToolCall {
                        id: m.tool_call_id.clone().unwrap_or_default(),
                        name: m.tool_name.clone().unwrap_or_default(),
                        input,
                    },
                    content: String::new(),
                })
            }
            MessageRole::ToolResult => Some(LlmMessage {
                role: ChatRole::ToolResult {
                    call_id: m.tool_call_id.clone().unwrap_or_default(),
                    content: m.content.clone(),
                    is_error: m.tool_is_error.unwrap_or(false),
                },
                content: String::new(),
            }),
        })
        .collect()
}

/// Builds the `ToolSpec` list offered to the model this step from every
/// tool currently in the registry.
fn tool_specs(registry: &ToolRegistry) -> Vec<ToolSpec> {
    registry
        .iter()
        .map(|t| ToolSpec {
            name: t.name().to_string(),
            description: t.description().to_string(),
            input_schema: t.input_schema(),
        })
        .collect()
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
        return Err(HolziError::InvalidIdempotencyKey);
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
            return Err(HolziError::IdempotencyKeyConflict);
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
            return Err(HolziError::IdempotencyKeyConflict);
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

    let tools = {
        let registry = chat.tool_registry.lock().map_err(|e| HolziError::CrdtInit {
            reason: format!("chat.tool_registry mutex poisoned: {e}"),
        })?;
        tool_specs(&registry)
    };

    let request = ChatRequest {
        model_id: request_model_id,
        system_prompt: args.system_prompt.clone(),
        messages: history_to_messages(&history),
        max_new_tokens: args.max_new_tokens,
        tools,
    };

    // Start the generation. `stream_chat` may fail before the first
    // byte (credential rejection, transport error); surface those to
    // the caller instead of hiding them inside the streaming task.
    let stream = match session.adapter.stream_chat(request.clone()).await {
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
        let chat_state = app_for_task.state::<ChatState>();
        let mut emit = |name: &'static str, payload: Value| {
            let _ = app_for_task.emit(name, payload);
        };
        run_turn(
            &assistant_db,
            &chat_state,
            &session_for_task,
            thread_id,
            user_message_id,
            assistant_message_id,
            request,
            stream,
            &mut emit,
        )
        .await;
    });

    Ok(SendMessageResult {
        thread_id,
        user_message_id,
        assistant_message_id,
    })
}

/// Persists one row. Used for every row except the turn's terminal
/// assistant row, which also needs `chat_threads` updated atomically
/// (see [`persist_final_message`]).
async fn persist_message(
    db: &haex_crdt::Database,
    msg: ChatMessage,
) -> std::result::Result<(), String> {
    let db = db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            msg_store::insert_message(conn, &msg)
                .map(|_| ())
                .map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| format!("persist join: {e}"))?
    .map_err(|e| format!("persist failed: {e}"))
}

/// Persists the turn's terminal assistant row and updates `chat_threads`
/// in the same connection call — mirrors the pre-tool-loop behavior where
/// both happened atomically together.
async fn persist_final_message(
    db: &haex_crdt::Database,
    msg: ChatMessage,
    provider_id: Option<Uuid>,
    model_id: String,
) -> std::result::Result<(), String> {
    let db = db.clone();
    let thread_id = msg.thread_id;
    let now = msg.created_at;
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            msg_store::insert_message(conn, &msg).map_err(haex_crdt::Error::from)?;
            thread_store::update_thread(
                conn,
                thread_id,
                &current_title(conn, thread_id).unwrap_or_default(),
                provider_id,
                Some(&model_id),
                now,
            )
            .map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| format!("persist join: {e}"))?
    .map_err(|e| format!("persist failed: {e}"))?;
    Ok(())
}

/// Reads `chat.permission_mode` for this device, defaulting to `Manual`
/// when unset or unparseable (spec.md Assumptions).
async fn read_permission_mode(db: &haex_crdt::Database) -> PermissionMode {
    let db = db.clone();
    let this_device = db.device_id();
    let raw = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            preferences::get(conn, PrefScope::Device(this_device), PREF_PERMISSION_MODE)
                .map_err(haex_crdt::Error::from)
        })
    })
    .await
    .ok()
    .and_then(|r| r.ok())
    .flatten();
    raw.as_deref()
        .and_then(PermissionMode::parse)
        .unwrap_or_default()
}

/// What to do with one tool call, decided before any concurrent
/// waiting/execution begins (see the comment at its only call site).
enum ToolPlan {
    Allow(Arc<dyn Tool>),
    Ask {
        tool: Arc<dyn Tool>,
        rx: tokio::sync::oneshot::Receiver<ApprovalDecision>,
    },
    Deny(Arc<dyn Tool>),
    Unknown,
}

fn empty_tool_message(id: Uuid, thread_id: Uuid, parent_id: Option<Uuid>) -> ChatMessage {
    ChatMessage {
        id,
        thread_id,
        parent_id,
        role: MessageRole::User, // overwritten by every caller
        content: String::new(),
        provider_id: None,
        model_id: None,
        prompt_tokens: None,
        completion_tokens: None,
        finish_reason: None,
        created_at: now_ms(),
        idempotency_key: None,
        tool_name: None,
        tool_call_id: None,
        tool_input: None,
        tool_is_error: None,
        tool_source: None,
    }
}

/// Drives one `send_message` turn to completion: consumes the
/// already-started first step's stream, executes any tool calls the model
/// requests, issues further steps as needed (bounded by
/// `MAX_TOOL_ROUNDS`), and persists every row along the way. Takes a plain
/// `emit` callback rather than an `AppHandle` so it runs without a live
/// Tauri app — `tests/chat_tool_loop.rs` passes a closure that records
/// events instead of dispatching them; the production caller above wraps
/// `app.emit`. `request` already carries the model/system-prompt/tools
/// used for `stream`'s already-in-flight first step; its `messages` grow
/// as tool rounds are appended for subsequent steps.
#[allow(clippy::too_many_arguments)]
pub async fn run_turn(
    db: &haex_crdt::Database,
    chat_state: &ChatState,
    session: &ActiveSession,
    thread_id: Uuid,
    user_message_id: Uuid,
    assistant_message_id: Uuid,
    mut request: ChatRequest,
    mut stream: crate::adapters::types::AdapterStream,
    emit: &mut (dyn FnMut(&'static str, Value) + Send),
) {
    let mut parent_id = user_message_id;
    let mut rounds_used = 0usize;

    loop {
        let mut assembled = String::new();
        let mut prompt_tokens: Option<usize> = None;
        let mut completion_tokens: Option<usize> = None;
        let mut ttft_ms: Option<u64> = None;
        let mut tool_calls: Vec<LlmToolCall> = Vec::new();
        let mut error_reason: Option<String> = None;
        let mut saw_done = false;

        while let Some(item) = stream.next().await {
            match item {
                Ok(StreamChunk::Delta { content, reasoning }) => {
                    if !content.is_empty() || reasoning.is_some() {
                        assembled.push_str(&content);
                        emit(
                            EVENT_CHAT_TOKEN,
                            serde_json::to_value(TokenEvent {
                                message_id: assistant_message_id,
                                delta: content,
                                reasoning,
                            })
                            .expect("TokenEvent always serializes"),
                        );
                    }
                }
                Ok(StreamChunk::ToolCalls(calls)) => {
                    tool_calls = calls;
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

        if error_reason.is_none() && !tool_calls.is_empty() {
            // A step that ends by calling tools. Any text the model
            // emitted first becomes its own interim assistant row
            // (data-model.md's `assistant(*)` — optional, only present
            // when the step actually produced text before its tool use).
            if !assembled.is_empty() {
                let interim_id = Uuid::new_v4();
                let msg = ChatMessage {
                    role: MessageRole::Assistant,
                    content: assembled.clone(),
                    provider_id: session.provider_id,
                    model_id: Some(session.model_id.clone()),
                    ..empty_tool_message(interim_id, thread_id, Some(parent_id))
                };
                if let Err(reason) = persist_message(db, msg).await {
                    emit(
                        EVENT_CHAT_MESSAGE_ERROR,
                        serde_json::to_value(MessageErrorEvent {
                            message_id: assistant_message_id,
                            thread_id,
                            reason,
                        })
                        .expect("MessageErrorEvent always serializes"),
                    );
                    emit(
                        EVENT_CHAT_TURN_COMPLETE,
                        serde_json::to_value(TurnCompleteEvent {
                            thread_id,
                            assistant_message_id: None,
                            finish_reason: FinishReason::Error,
                        })
                        .expect("TurnCompleteEvent always serializes"),
                    );
                    return;
                }
                parent_id = interim_id;
                request.messages.push(LlmMessage {
                    role: ChatRole::Assistant,
                    content: assembled,
                });
            }

            // Decide + (for `Ask`) mint the approval wait sequentially —
            // `emit` is a single `&mut` closure, not shareable across
            // concurrent futures, so every `tool-permission-request` fires
            // here, before any concurrent waiting/execution begins below.
            // This is also what lets two independent Risky calls each get
            // their own simultaneously-pending approval (T024A): each gets
            // its own oneshot the moment its `Ask` is decided, well before
            // either one's wait resolves.
            let mut plans: Vec<(LlmToolCall, ToolPlan)> = Vec::with_capacity(tool_calls.len());
            for call in tool_calls {
                let tool = {
                    let registry = chat_state
                        .tool_registry
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    registry.get(&call.name)
                };
                let Some(tool) = tool else {
                    plans.push((call, ToolPlan::Unknown));
                    continue;
                };
                // Read fresh per call, not cached for the round: a mode
                // change must not retroactively affect a decision already
                // made for an earlier call, but the very next tool use
                // must observe it (T024B).
                let mode = read_permission_mode(db).await;
                match permission::decide(mode, tool.risk_class()) {
                    permission::Decision::Allow => plans.push((call, ToolPlan::Allow(tool))),
                    permission::Decision::Deny => plans.push((call, ToolPlan::Deny(tool))),
                    permission::Decision::Ask => {
                        let request_id = Uuid::new_v4();
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        chat_state
                            .pending_tool_approvals
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(request_id, tx);
                        emit(
                            EVENT_TOOL_PERMISSION_REQUEST,
                            serde_json::to_value(ToolPermissionRequestEvent {
                                request_id,
                                thread_id,
                                tool_name: call.name.clone(),
                                tool_input: call.input.clone(),
                                risk_class: risk_class_str(tool.risk_class()),
                            })
                            .expect("ToolPermissionRequestEvent always serializes"),
                        );
                        plans.push((call, ToolPlan::Ask { tool, rx }));
                    }
                }
            }

            let executed: Vec<(LlmToolCall, ToolExecResult, &'static str)> =
                futures::future::join_all(plans.into_iter().map(|(call, plan)| async move {
                    match plan {
                        ToolPlan::Allow(tool) => {
                            let source = tool.source();
                            let result = tool.execute(call.input.clone()).await;
                            (call, result, source)
                        }
                        ToolPlan::Ask { tool, rx } => {
                            // No cancellation race yet — Phase 5 (US3, out
                            // of scope here) extends `ChatState`/the loop
                            // to also resolve this wait on
                            // `abort_current_generation` (T032).
                            match rx.await {
                                Ok(ApprovalDecision::Allow) => {
                                    let source = tool.source();
                                    let result = tool.execute(call.input.clone()).await;
                                    (call, result, source)
                                }
                                Ok(ApprovalDecision::Deny) => (
                                    call,
                                    ToolExecResult::error("denied_by_user"),
                                    tool.source(),
                                ),
                                Err(_) => (
                                    call,
                                    ToolExecResult::error("tool_call_cancelled"),
                                    tool.source(),
                                ),
                            }
                        }
                        ToolPlan::Deny(tool) => (
                            call,
                            // Fixed, non-localized marker — the frontend
                            // translates it (CONTEXT.md i18n boundary),
                            // same convention as `LoadPhase` above.
                            ToolExecResult::error("blocked_by_plan_mode"),
                            tool.source(),
                        ),
                        ToolPlan::Unknown => {
                            // The model named a tool no longer in the
                            // registry (e.g. its MCP server disconnected
                            // mid-conversation, spec.md Edge Cases) or one
                            // that never existed. `cli` never disappears
                            // (registered unconditionally, T019), so `mcp`
                            // is the more plausible source to record here.
                            let name = call.name.clone();
                            (
                                call,
                                ToolExecResult::error(format!("unknown tool: {name}")),
                                "mcp",
                            )
                        }
                    }
                }))
                .await;

            for (call, result, source) in executed {
                let tool_call_row_id = Uuid::new_v4();
                let input_json =
                    serde_json::to_string(&call.input).unwrap_or_else(|_| "{}".to_string());
                let call_msg = ChatMessage {
                    role: MessageRole::ToolCall,
                    provider_id: session.provider_id,
                    model_id: Some(session.model_id.clone()),
                    tool_name: Some(call.name.clone()),
                    tool_call_id: Some(call.id.clone()),
                    tool_input: Some(input_json),
                    tool_source: Some(source.to_string()),
                    ..empty_tool_message(tool_call_row_id, thread_id, Some(parent_id))
                };
                if let Err(reason) = persist_message(db, call_msg).await {
                    emit(
                        EVENT_CHAT_MESSAGE_ERROR,
                        serde_json::to_value(MessageErrorEvent {
                            message_id: assistant_message_id,
                            thread_id,
                            reason,
                        })
                        .expect("MessageErrorEvent always serializes"),
                    );
                    emit(
                        EVENT_CHAT_TURN_COMPLETE,
                        serde_json::to_value(TurnCompleteEvent {
                            thread_id,
                            assistant_message_id: None,
                            finish_reason: FinishReason::Error,
                        })
                        .expect("TurnCompleteEvent always serializes"),
                    );
                    return;
                }
                parent_id = tool_call_row_id;
                emit(
                    EVENT_CHAT_TOOL_CALL,
                    serde_json::to_value(ToolCallEvent {
                        message_id: tool_call_row_id,
                        thread_id,
                        tool_name: call.name.clone(),
                        tool_input: call.input.clone(),
                        tool_source: source.to_string(),
                    })
                    .expect("ToolCallEvent always serializes"),
                );

                let tool_result_row_id = Uuid::new_v4();
                let result_msg = ChatMessage {
                    role: MessageRole::ToolResult,
                    content: result.content.clone(),
                    provider_id: session.provider_id,
                    model_id: Some(session.model_id.clone()),
                    tool_call_id: Some(call.id.clone()),
                    tool_is_error: Some(result.is_error),
                    ..empty_tool_message(tool_result_row_id, thread_id, Some(parent_id))
                };
                if let Err(reason) = persist_message(db, result_msg).await {
                    emit(
                        EVENT_CHAT_MESSAGE_ERROR,
                        serde_json::to_value(MessageErrorEvent {
                            message_id: assistant_message_id,
                            thread_id,
                            reason,
                        })
                        .expect("MessageErrorEvent always serializes"),
                    );
                    emit(
                        EVENT_CHAT_TURN_COMPLETE,
                        serde_json::to_value(TurnCompleteEvent {
                            thread_id,
                            assistant_message_id: None,
                            finish_reason: FinishReason::Error,
                        })
                        .expect("TurnCompleteEvent always serializes"),
                    );
                    return;
                }
                parent_id = tool_result_row_id;
                emit(
                    EVENT_CHAT_TOOL_RESULT,
                    serde_json::to_value(ToolResultEvent {
                        message_id: tool_result_row_id,
                        thread_id,
                        tool_call_id: call.id.clone(),
                        content: result.content.clone(),
                        is_error: result.is_error,
                    })
                    .expect("ToolResultEvent always serializes"),
                );

                request.messages.push(LlmMessage {
                    role: ChatRole::ToolCall {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        input: call.input.clone(),
                    },
                    content: String::new(),
                });
                request.messages.push(LlmMessage {
                    role: ChatRole::ToolResult {
                        call_id: call.id.clone(),
                        content: result.content.clone(),
                        is_error: result.is_error,
                    },
                    content: String::new(),
                });
            }

            rounds_used += 1;
            if rounds_used >= MAX_TOOL_ROUNDS {
                let final_msg = ChatMessage {
                    role: MessageRole::Assistant,
                    finish_reason: Some(FinishReason::ToolLimitReached),
                    ..empty_tool_message(assistant_message_id, thread_id, Some(parent_id))
                };
                if let Err(reason) = persist_final_message(
                    db,
                    final_msg,
                    session.provider_id,
                    session.model_id.clone(),
                )
                .await
                {
                    emit(
                        EVENT_CHAT_MESSAGE_ERROR,
                        serde_json::to_value(MessageErrorEvent {
                            message_id: assistant_message_id,
                            thread_id,
                            reason,
                        })
                        .expect("MessageErrorEvent always serializes"),
                    );
                    emit(
                        EVENT_CHAT_TURN_COMPLETE,
                        serde_json::to_value(TurnCompleteEvent {
                            thread_id,
                            assistant_message_id: None,
                            finish_reason: FinishReason::Error,
                        })
                        .expect("TurnCompleteEvent always serializes"),
                    );
                    return;
                }
                emit(
                    EVENT_CHAT_MESSAGE_COMPLETE,
                    serde_json::to_value(MessageCompleteEvent {
                        message_id: assistant_message_id,
                        thread_id,
                        prompt_tokens: None,
                        completion_tokens: None,
                        ttft_ms: None,
                    })
                    .expect("MessageCompleteEvent always serializes"),
                );
                emit(
                    EVENT_CHAT_TURN_COMPLETE,
                    serde_json::to_value(TurnCompleteEvent {
                        thread_id,
                        assistant_message_id: Some(assistant_message_id),
                        finish_reason: FinishReason::ToolLimitReached,
                    })
                    .expect("TurnCompleteEvent always serializes"),
                );
                return;
            }

            match session.adapter.stream_chat(request.clone()).await {
                Ok(next_stream) => {
                    stream = next_stream;
                    continue;
                }
                Err(error) => {
                    let final_msg = ChatMessage {
                        role: MessageRole::Assistant,
                        finish_reason: Some(FinishReason::Error),
                        ..empty_tool_message(assistant_message_id, thread_id, Some(parent_id))
                    };
                    let _ = persist_final_message(
                        db,
                        final_msg,
                        session.provider_id,
                        session.model_id.clone(),
                    )
                    .await;
                    emit(
                        EVENT_CHAT_MESSAGE_ERROR,
                        serde_json::to_value(MessageErrorEvent {
                            message_id: assistant_message_id,
                            thread_id,
                            reason: format!("adapter start: {error}"),
                        })
                        .expect("MessageErrorEvent always serializes"),
                    );
                    emit(
                        EVENT_CHAT_TURN_COMPLETE,
                        serde_json::to_value(TurnCompleteEvent {
                            thread_id,
                            assistant_message_id: Some(assistant_message_id),
                            finish_reason: FinishReason::Error,
                        })
                        .expect("TurnCompleteEvent always serializes"),
                    );
                    return;
                }
            }
        }

        // Final step: a plain answer (no tool calls), a step-level
        // adapter error, or cancellation (the stream closed without
        // `Done` and without an error — `abort_current_generation`).
        let finish_reason = if error_reason.is_some() {
            FinishReason::Error
        } else if saw_done {
            FinishReason::Complete
        } else {
            FinishReason::Cancelled
        };
        let final_msg = ChatMessage {
            role: MessageRole::Assistant,
            content: assembled,
            provider_id: session.provider_id,
            model_id: Some(session.model_id.clone()),
            prompt_tokens: prompt_tokens.map(|n| n as i64),
            completion_tokens: completion_tokens.map(|n| n as i64),
            finish_reason: Some(finish_reason),
            ..empty_tool_message(assistant_message_id, thread_id, Some(parent_id))
        };
        if let Err(reason) =
            persist_final_message(db, final_msg, session.provider_id, session.model_id.clone())
                .await
        {
            emit(
                EVENT_CHAT_MESSAGE_ERROR,
                serde_json::to_value(MessageErrorEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    reason,
                })
                .expect("MessageErrorEvent always serializes"),
            );
            emit(
                EVENT_CHAT_TURN_COMPLETE,
                serde_json::to_value(TurnCompleteEvent {
                    thread_id,
                    assistant_message_id: None,
                    finish_reason: FinishReason::Error,
                })
                .expect("TurnCompleteEvent always serializes"),
            );
            return;
        }

        if let Some(reason) = error_reason {
            emit(
                EVENT_CHAT_MESSAGE_ERROR,
                serde_json::to_value(MessageErrorEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    reason,
                })
                .expect("MessageErrorEvent always serializes"),
            );
        } else {
            emit(
                EVENT_CHAT_MESSAGE_COMPLETE,
                serde_json::to_value(MessageCompleteEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    prompt_tokens,
                    completion_tokens,
                    ttft_ms,
                })
                .expect("MessageCompleteEvent always serializes"),
            );
        }
        emit(
            EVENT_CHAT_TURN_COMPLETE,
            serde_json::to_value(TurnCompleteEvent {
                thread_id,
                assistant_message_id: Some(assistant_message_id),
                finish_reason,
            })
            .expect("TurnCompleteEvent always serializes"),
        );
        return;
    }
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RespondToolPermissionArgs {
    pub request_id: Uuid,
    pub decision: ApprovalDecisionWire,
}

/// Wire shape for `decision` — a bare string, no wrapper object, matching
/// contracts/tauri-commands.md's `'allow' | 'deny'`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecisionWire {
    Allow,
    Deny,
}

/// Resolves one open `tool-permission-request` (contracts/tauri-commands.md
/// §respond_tool_permission). `InvalidInput` for a `request_id` that was
/// never known — there is no cancellation-tombstone tracking yet (Phase 5,
/// T032 adds resolving a pending request on abort; until then the only way
/// an id leaves `pending_tool_approvals` is through this command itself).
#[tauri::command]
pub async fn respond_tool_permission(
    chat: State<'_, ChatState>,
    args: RespondToolPermissionArgs,
) -> Result<()> {
    let sender = {
        let mut pending =
            chat.pending_tool_approvals
                .lock()
                .map_err(|e| HolziError::CrdtInit {
                    reason: format!("chat.pending_tool_approvals mutex poisoned: {e}"),
                })?;
        pending.remove(&args.request_id)
    };
    let Some(sender) = sender else {
        return Err(HolziError::InvalidInput {
            reason: format!("no pending tool permission request: {}", args.request_id),
        });
    };
    let decision = match args.decision {
        ApprovalDecisionWire::Allow => ApprovalDecision::Allow,
        ApprovalDecisionWire::Deny => ApprovalDecision::Deny,
    };
    // The receiver may already be gone (e.g. the turn ended some other
    // way) — that is not an error for the caller.
    let _ = sender.send(decision);
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
