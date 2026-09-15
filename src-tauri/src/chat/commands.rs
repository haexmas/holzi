//! Model-lifecycle + streaming chat commands.
//!
//! Maintainability exception (spaex 500-LoC rule): at ~3000 lines this is
//! the largest hand-maintained file in the repository. Model lifecycle,
//! send admission and the turn/step loop are coupled through one shared
//! layer — the `EVENT_*` names, every `*Event` payload struct, the
//! `ChatState` operation reservation and the cancellation token — and
//! moving any one of the three out before that layer has its own module
//! would duplicate the payload definitions across the split. It stays
//! whole until the ordered plan below runs, so the event-ordering and
//! ownership guarantees covered by `tests/chat_tool_loop.rs` and
//! `commands_tests.rs` keep a single reviewable home.
//!
//! Concrete split plan — five mechanical PRs in this order, each keeping
//! the registered command surface and every test unchanged. Steps 1
//! (`chat/events.rs`), 2 (`chat/model_loading.rs`), 3
//! (`chat/send_admission.rs`) and 4 (`chat/turn.rs`) are done; step 5
//! remains:
//!
//! 5. `chat/default_model.rs`: `resolve_default_model`,
//!    `resolve_default_local_model`, `start_default_model_preload`,
//!    `model_load_status` (~250 lines).
//!
//! What remains here is `send_message`, the abort commands and the
//! tool-permission commands — roughly 400 lines.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::adapters::types::{ChatMessage as LlmMessage, ChatRequest, ChatRole, ToolSpec};
use crate::chat::tools::{ApprovalDecision, ToolRegistry};
use crate::error::{HolziError, Result};
use crate::models::paths;
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::{
    chat_messages::{self as msg_store, ChatMessage, MessageRole},
    chat_threads as thread_store, models as models_store, preferences,
    preferences::PrefScope,
};

use super::events::emit_load_error;
use super::model_loading::{
    load_api_key_model, load_model_inner, resolve_display_name, LoadIdentity, LoadOutcome,
};
use super::send_admission::{
    persist_send_transaction, resolve_idempotent_send, IdempotentSend, PersistedSend,
};
use super::session::{ChatState, ModelLoadStatus};
use super::turn::{now_ms, run_turn, start_step_stream, StreamStartError};

const PREF_LAST_ACTIVE_MODEL: &str = "chat.last_active_model_id";
const PREF_DEFAULT_MODEL: &str = "chat.default_model_id";

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

/// Returns true only for model families with a known native reasoning mode.
/// Unknown/custom models stay conservative and can still render reasoning
/// deltas if an adapter supplies them, but are not sent an explicit enable
/// flag that their API may reject.
pub fn model_supports_reasoning(model_id: &str) -> bool {
    let id = model_id.to_ascii_lowercase();
    // Some published checkpoints use a family name that is otherwise
    // reasoning-capable but explicitly disable thinking. Keep these exact
    // model exceptions ahead of the family fallback below.
    const EXACT_CAPABILITIES: &[(&str, bool)] = &[
        ("qwen3-4b-instruct-2507", false),
        ("qwen3-30b-a3b-instruct-2507", false),
        ("claude-haiku-4-5", true),
        ("claude-haiku-4-5-20251001", true),
    ];
    if let Some((_, supported)) = EXACT_CAPABILITIES
        .iter()
        .find(|(known_id, _)| id == *known_id)
    {
        return *supported;
    }
    if id.contains("qwen3") && id.contains("instruct-2507") {
        return false;
    }
    id.contains("qwen3")
        || id.contains("deepseek-r1")
        || id.contains("gpt-oss")
        || id.contains("claude-3-7")
        || id.contains("claude-sonnet-4")
        || id.contains("claude-opus-4")
        || id.contains("claude-haiku-4-5")
}

/// Qwen3's native tool-call path must run with thinking disabled. The
/// mistralrs tool-calling example deliberately leaves thinking unspecified;
/// enabling it makes Qwen3 prone to spending the whole turn narrating a
/// prospective tool call instead of emitting the structured call. Keep
/// reasoning enabled for ordinary Qwen3 replies and other providers.
fn reasoning_requested_for(model_id: &str, tools: &[ToolSpec]) -> bool {
    let is_qwen3_tool_request =
        model_id.to_ascii_lowercase().contains("qwen3") && !tools.is_empty();
    model_supports_reasoning(model_id) && !is_qwen3_tool_request
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

    // Reserve before reading the vault, while still letting an idempotent
    // retry return its existing ids when a turn already owns the reservation.
    let operation = chat.acquire_operation();
    let db = active_database(&state)?;
    let cancel_token = CancellationToken::new();
    if operation.is_ok() {
        // Reserve cancellation before the first await, including DB staging.
        // A busy idempotent replay must never replace the live turn's token.
        *chat
            .tool_cancellation
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("chat.tool_cancellation mutex poisoned: {e}"),
            })? = Some(cancel_token.clone());
    }

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

    let operation = operation?;
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
        PersistedSend::UnknownThread { thread_id } => {
            return Err(HolziError::NotFound {
                name: thread_id.to_string(),
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
    let tools = {
        let registry = chat
            .tool_registry
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("chat.tool_registry mutex poisoned: {e}"),
            })?;
        tool_specs(&registry)
    };
    let reasoning_requested = reasoning_requested_for(&request_model_id, &tools);

    let request = ChatRequest {
        model_id: request_model_id,
        system_prompt: args.system_prompt.clone(),
        messages: history_to_messages(&history),
        reasoning_requested,
        max_new_tokens: args.max_new_tokens,
        tools,
    };

    let mut attempt = 0;
    let mut emit = |name: &'static str, payload: Value| {
        let _ = app.emit(name, payload);
    };
    let stream = match start_step_stream(
        &session,
        &chat,
        &request,
        &cancel_token,
        &mut attempt,
        thread_id,
        assistant_message_id,
        &mut emit,
    )
    .await
    {
        Ok(stream) => stream,
        Err(error) => {
            let error = match error {
                StreamStartError::Cancelled => "generation cancelled".to_string(),
                StreamStartError::Failed(reason) => reason,
            };
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

    let app_for_task = app.clone();
    let session_for_task = session.clone();
    let assistant_db = db.clone();

    tauri::async_runtime::spawn(async move {
        let _operation = operation;
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
            attempt,
            cancel_token,
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

/// Resolves only local candidates for the background Vault-open preload.
/// Provider preferences are skipped and the next fallback tier is checked.
async fn resolve_default_local_model(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<Option<String>> {
    let db = active_database(state)?;
    let this_device = db.device_id();
    let db_clone = db.clone();
    let (preferences_to_try, installed_local_ids) =
        tauri::async_runtime::spawn_blocking(move || {
            db_clone.with_connection(|conn| {
                let last_active =
                    preferences::get(conn, PrefScope::Device(this_device), PREF_LAST_ACTIVE_MODEL)
                        .map_err(haex_crdt::Error::from)?;
                let default_device =
                    preferences::get(conn, PrefScope::Device(this_device), PREF_DEFAULT_MODEL)
                        .map_err(haex_crdt::Error::from)?;
                let default_vault = preferences::get(conn, PrefScope::Vault, PREF_DEFAULT_MODEL)
                    .map_err(haex_crdt::Error::from)?;
                let local_ids = models_store::list_all_models(conn)
                    .map_err(haex_crdt::Error::from)?
                    .into_iter()
                    .filter(|row| !row.id.contains(':'))
                    .map(|row| row.id)
                    .collect::<Vec<_>>();
                Ok::<_, haex_crdt::Error>(([last_active, default_device, default_vault], local_ids))
            })
        })
        .await
        .map_err(|e| HolziError::CrdtInit {
            reason: format!("resolve_default_local_model join: {e}"),
        })?
        .map_err(HolziError::from)?;

    let loadable_local = installed_local_ids
        .into_iter()
        .filter(|id| {
            paths::canonical_model_file(app, id)
                .ok()
                .flatten()
                .is_some()
        })
        .collect::<Vec<_>>();
    let is_loadable = |id: &str| loadable_local.iter().any(|candidate| candidate == id);

    for preference in preferences_to_try.into_iter().flatten() {
        if !preference.contains(':') && is_loadable(&preference) {
            return Ok(Some(preference));
        }
    }
    Ok(loadable_local.into_iter().next())
}

/// Starts the single background preload for the currently-published Vault.
/// The task owns no operation mutex, so a Vault switch can cancel it promptly.
pub fn start_default_model_preload(app: AppHandle, chat: ChatState) {
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let task_chat = chat.clone();
    let task_app = app.clone();
    let join = tauri::async_runtime::spawn(async move {
        let state = task_app.state::<AppState>();
        let model_id = match resolve_default_local_model(&task_app, &state).await {
            Ok(Some(model_id)) => model_id,
            Ok(None) | Err(_) => return,
        };
        if task_cancel.is_cancelled() {
            return;
        }

        let (vault_generation, load_id) = task_chat.begin_model_load();
        let identity = LoadIdentity {
            vault_generation,
            load_id,
        };
        match load_model_inner(
            &task_app,
            &state,
            &task_chat,
            &model_id,
            identity,
            Some(&task_cancel),
            false,
        )
        .await
        {
            Ok(LoadOutcome::Loaded(_)) | Ok(LoadOutcome::Cancelled) => {}
            Err(error) if !task_cancel.is_cancelled() => {
                let model_name = resolve_display_name(&state, &model_id).await;
                if task_chat.set_model_error(
                    vault_generation,
                    load_id,
                    Some(model_id.clone()),
                    model_name.clone(),
                    "model_load_failed".into(),
                ) {
                    emit_load_error(
                        &task_app,
                        vault_generation,
                        load_id,
                        Some(model_id),
                        model_name,
                        "model_load_failed",
                    );
                }
                log::warn!("background model preload failed: {error}");
            }
            Err(_) => {}
        }
    });
    chat.install_preload_handle(cancel, join);
}

/// Read-only snapshot of the active Vault's model-load state.
#[tauri::command]
pub async fn model_load_status(chat: State<'_, ChatState>) -> Result<ModelLoadStatus> {
    Ok(chat.model_load_status())
}

/// Cancels the in-flight generation, if any. Idempotent — safe to
/// call when nothing is running. Split from the `#[tauri::command]`
/// wrapper so integration tests (`tests/chat_tool_loop.rs`) can trigger the
/// same cancellation without a live `AppHandle`/`State` — same pattern as
/// `resolve_idempotent_send` etc. above.
pub fn abort_turn(chat_state: &ChatState) -> Result<()> {
    {
        let mut guard = chat_state
            .current_generation
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("chat.current_generation mutex poisoned: {e}"),
            })?;
        if let Some(abort) = guard.take() {
            abort.abort();
        }
    }
    // Ends any in-flight `Tool::execute` (T032) — CLI/MCP implementations
    // race their own work against this signal and tear it down on the spot.
    {
        let guard = chat_state
            .tool_cancellation
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("chat.tool_cancellation mutex poisoned: {e}"),
            })?;
        if let Some(token) = guard.as_ref() {
            token.cancel();
        }
    }
    // Dropping every pending sender resolves its `oneshot::Receiver` with
    // an `Err`, which the turn loop already treats as cancelled rather
    // than denied (distinct from a `respond_tool_permission` deny).
    {
        let mut pending =
            chat_state
                .pending_tool_approvals
                .lock()
                .map_err(|e| HolziError::CrdtInit {
                    reason: format!("chat.pending_tool_approvals mutex poisoned: {e}"),
                })?;
        let mut cancelled =
            chat_state
                .cancelled_tool_approvals
                .lock()
                .map_err(|e| HolziError::CrdtInit {
                    reason: format!("chat.cancelled_tool_approvals mutex poisoned: {e}"),
                })?;
        cancelled.extend(pending.keys().copied());
        pending.clear();
    }
    Ok(())
}

#[tauri::command]
pub async fn abort_current_generation(chat: State<'_, ChatState>) -> Result<()> {
    abort_turn(&chat)
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

/// Resolves one open permission request; a late reply to a cancelled
/// request is a no-op, while an unknown id remains an input error.
#[tauri::command]
pub async fn respond_tool_permission(
    chat: State<'_, ChatState>,
    args: RespondToolPermissionArgs,
) -> Result<()> {
    resolve_tool_permission(&chat, args)
}

fn resolve_tool_permission(chat: &ChatState, args: RespondToolPermissionArgs) -> Result<()> {
    let sender = {
        let mut pending = chat
            .pending_tool_approvals
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("chat.pending_tool_approvals mutex poisoned: {e}"),
            })?;
        pending.remove(&args.request_id)
    };
    let Some(sender) = sender else {
        if chat
            .cancelled_tool_approvals
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("chat.cancelled_tool_approvals mutex poisoned: {e}"),
            })?
            .contains(&args.request_id)
        {
            return Ok(());
        }
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

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
