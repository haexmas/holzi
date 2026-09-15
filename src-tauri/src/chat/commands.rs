//! `send_message` admission and kickoff, generation abort, and
//! tool-permission resolution.
//!
//! Maintainability exception (spaex 500-LoC rule): this is what's left of
//! the original ~3000-line file after the 2026-09-15 review's five-step
//! split into `chat/events.rs`, `chat/model_loading.rs`,
//! `chat/send_admission.rs`, `chat/turn.rs` and `chat/default_model.rs`.
//! `send_message` alone is still ~290 lines: idempotency admission and
//! transactional persistence (delegating to `send_admission`), building
//! the `ChatRequest` from history, starting the first step (delegating to
//! `turn::start_step_stream`), and — on failure — rolling back the staged
//! message/thread before spawning `turn::run_turn` for the rest of the
//! turn. Every phase shares `db`/`chat`/`app` and the early-return
//! cleanup path, so splitting it without a small context struct to carry
//! that state would mostly move code around rather than shrink it.
//!
//! Concrete split plan, if this grows further: extract the idempotency
//! dedup + `persist_send_transaction` call (roughly the first third,
//! ending once `thread_id`/`user_message_id`/`assistant_message_id` are
//! known) into a `send_admission::resolve_and_persist_send` helper next
//! to the functions it already calls, and the request-building +
//! `start_step_stream` + cleanup-on-failure + `run_turn` spawn (the rest)
//! into a `turn::start_turn` helper next to `run_turn`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::adapters::types::{ChatMessage as LlmMessage, ChatRequest, ChatRole, ToolSpec};
use crate::chat::tools::{ApprovalDecision, ToolRegistry};
use crate::error::{HolziError, Result};
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::{
    chat_messages::{self as msg_store, ChatMessage, MessageRole},
    chat_threads as thread_store, preferences,
    preferences::PrefScope,
};

use super::send_admission::{
    persist_send_transaction, resolve_idempotent_send, IdempotentSend, PersistedSend,
};
use super::session::ChatState;
use super::turn::{now_ms, run_turn, start_step_stream, StreamStartError};

pub(crate) const PREF_LAST_ACTIVE_MODEL: &str = "chat.last_active_model_id";

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
