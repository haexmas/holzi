//! Model-lifecycle + streaming chat commands.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::llm::local::{ChatMessage as LlmMessage, ChatRequest, ChatRole, LocalModel, StreamChunk};
use crate::models::paths;
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::{
    chat_messages::{self as msg_store, ChatMessage, FinishReason, MessageRole},
    chat_threads::{self as thread_store, ChatThread},
    device_downloaded_models,
};

use super::session::{ActiveSession, ChatState};

const EVENT_CHAT_TOKEN: &str = "chat-token";
const EVENT_CHAT_MESSAGE_COMPLETE: &str = "chat-message-complete";
const EVENT_CHAT_MESSAGE_ERROR: &str = "chat-message-error";

/// Payload for `active_model_info` and `load_local_model`.
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
    /// Cap on generated tokens. `None` uses the mistralrs default.
    pub max_new_tokens: Option<usize>,
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

/// Loads a downloaded local model into the active session. If another
/// model was already loaded, it is dropped first.
#[tauri::command]
pub async fn load_local_model(
    app: AppHandle,
    state: State<'_, AppState>,
    chat: State<'_, ChatState>,
    model_id: String,
) -> Result<LoadedModelInfo> {
    // Resolve the file path and metadata from the DB.
    let db = active_database(&state)?;
    let id_owned = model_id.clone();
    let resolved = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            let dm = device_downloaded_models::get_downloaded_model(conn, &id_owned)
                .map_err(haex_crdt::Error::from)?
                .ok_or_else(|| {
                    haex_crdt::Error::from(haex_crdt::rusqlite::Error::QueryReturnedNoRows)
                })?;
            let mut stmt = conn
                .prepare("SELECT name, context_window FROM models WHERE id = ?1")
                .map_err(haex_crdt::Error::from)?;
            let row: (String, Option<i64>) = stmt
                .query_row(haex_crdt::rusqlite::params![id_owned], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .map_err(haex_crdt::Error::from)?;
            Ok((dm.relative_path, row.0, row.1))
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("resolve model join: {e}"),
    })?
    .map_err(|_| HolziError::ModelNotFound {
        id: model_id.clone(),
    })?;
    let (relative_path, name, context_window) = resolved;

    let absolute = paths::resolve_relative(&app, &relative_path)?;

    // Tokenizer repo lives in the catalog for now — arbitrary HF
    // downloads carry it directly; look either up.
    let tokenizer_repo = crate::catalog::get(&model_id)
        .map(|e| e.tokenizer_repo.clone())
        .or_else(|| tokenizer_repo_lookup_from_db(&state, &model_id))
        .ok_or_else(|| HolziError::InvalidInput {
            reason: format!("tokenizer_repo missing for model {model_id}"),
        })?;

    let model = LocalModel::load(&absolute, Some(&tokenizer_repo))
        .await
        .map_err(|e| HolziError::ModelDownload {
            reason: format!("mistralrs load: {e}"),
        })?;

    // Replace any previously-loaded session.
    let session = ActiveSession {
        model_id: model_id.clone(),
        model,
        tokenizer_repo: tokenizer_repo.clone(),
        context_window,
    };
    let info = LoadedModelInfo {
        model_id: model_id.clone(),
        name: name.clone(),
        tokenizer_repo: tokenizer_repo.clone(),
        context_window,
    };
    let mut guard = chat.session.lock().map_err(|e| HolziError::CrdtInit {
        reason: format!("chat.session mutex poisoned: {e}"),
    })?;
    *guard = Some(session);
    Ok(info)
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
pub async fn active_model_info(
    chat: State<'_, ChatState>,
) -> Result<Option<LoadedModelInfo>> {
    let guard = chat.session.lock().map_err(|e| HolziError::CrdtInit {
        reason: format!("chat.session mutex poisoned: {e}"),
    })?;
    Ok(guard.as_ref().map(|s| LoadedModelInfo {
        model_id: s.model_id.clone(),
        name: s.model_id.clone(), // TODO: cache display name in state
        tokenizer_repo: s.tokenizer_repo.clone(),
        context_window: s.context_window,
    }))
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
    let session = {
        let guard = chat.session.lock().map_err(|e| HolziError::CrdtInit {
            reason: format!("chat.session mutex poisoned: {e}"),
        })?;
        guard.clone().ok_or_else(|| HolziError::InvalidInput {
            reason: "no local model loaded".into(),
        })?
    };

    let db = active_database(&state)?;
    let thread_id = args.thread_id.unwrap_or_else(Uuid::new_v4);
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    let now = now_ms();

    // Persist thread (if new) and user message under one DB lock.
    let insert_db = db.clone();
    let content_owned = args.content.clone();
    let session_model_id = session.model_id.clone();
    let is_new_thread = args.thread_id.is_none();
    tauri::async_runtime::spawn_blocking(move || {
        insert_db.with_connection(|conn| {
            if is_new_thread {
                thread_store::insert_thread(
                    conn,
                    &ChatThread {
                        id: thread_id,
                        title: default_thread_title(&content_owned),
                        last_provider_id: None,
                        last_model_id: Some(session_model_id.clone()),
                        created_at: now,
                        updated_at: now,
                    },
                )
                .map_err(haex_crdt::Error::from)?;
            } else {
                thread_store::update_thread(
                    conn,
                    thread_id,
                    &current_title(conn, thread_id).unwrap_or_default(),
                    None,
                    Some(&session_model_id),
                    now,
                )
                .map_err(haex_crdt::Error::from)?;
            }
            let parent_id = last_message_id(conn, thread_id)?;
            let user_msg = ChatMessage {
                id: user_message_id,
                thread_id,
                parent_id,
                role: MessageRole::User,
                content: content_owned.clone(),
                provider_id: None,
                model_id: Some(session_model_id.clone()),
                prompt_tokens: None,
                completion_tokens: None,
                finish_reason: Some(FinishReason::Complete),
                created_at: now,
            };
            msg_store::insert_message(conn, &user_msg).map_err(haex_crdt::Error::from)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("insert user message join: {e}"),
    })?
    .map_err(HolziError::from)?;

    // Build the request from full history + the just-inserted user turn.
    let history_db = db.clone();
    let history = tauri::async_runtime::spawn_blocking(move || {
        history_db.with_connection(|conn| {
            let msgs = msg_store::list_messages(conn, thread_id)
                .map_err(haex_crdt::Error::from)?;
            Ok(msgs)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("history load join: {e}"),
    })?
    .map_err(HolziError::from)?;

    let request = ChatRequest {
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

    // Spawn generation. Its abort handle goes into ChatState so
    // `abort_current_generation` can cancel out-of-band.
    let mut handle = session.model.stream_chat(request);
    let abort = handle.abort_handle();
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

        while let Some(item) = handle.next().await {
            match item {
                Ok(StreamChunk::Delta { content, .. }) => {
                    if !content.is_empty() {
                        assembled.push_str(&content);
                        let _ = app_for_task.emit(
                            EVENT_CHAT_TOKEN,
                            TokenEvent {
                                message_id: assistant_message_id,
                                delta: content,
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
        } else {
            FinishReason::Complete
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
                    provider_id: None,
                    model_id: Some(session_for_task.model_id.clone()),
                    prompt_tokens: prompt_tokens.map(|n| n as i64),
                    completion_tokens: completion_tokens.map(|n| n as i64),
                    finish_reason: Some(finish_reason),
                    created_at: now2,
                };
                msg_store::insert_message(conn, &msg).map_err(haex_crdt::Error::from)?;
                thread_store::update_thread(
                    conn,
                    thread_id,
                    &current_title(conn, thread_id).unwrap_or_default(),
                    None,
                    Some(&session_for_task.model_id),
                    now2,
                )
                .map_err(haex_crdt::Error::from)?;
                Ok(())
            })
        })
        .await;

        if let Err(e) = insert_result {
            let _ = app_for_task.emit(
                EVENT_CHAT_MESSAGE_ERROR,
                MessageErrorEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    reason: format!("assistant persist join: {e}"),
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

fn current_title(
    conn: &haex_crdt::rusqlite::Connection,
    thread_id: Uuid,
) -> Option<String> {
    let mut stmt = conn.prepare("SELECT title FROM chat_threads WHERE id = ?1").ok()?;
    stmt.query_row(haex_crdt::rusqlite::params![thread_id.to_string()], |r| {
        r.get::<_, String>(0)
    })
    .ok()
}

fn last_message_id(
    conn: &haex_crdt::rusqlite::Connection,
    thread_id: Uuid,
) -> haex_crdt::Result<Option<Uuid>> {
    let msgs = msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from)?;
    Ok(msgs.last().map(|m| m.id))
}

fn tokenizer_repo_lookup_from_db(_state: &State<'_, AppState>, _id: &str) -> Option<String> {
    // For MVP the tokenizer_repo is not stored in the `models` table —
    // it comes from the catalog. Arbitrary HF downloads pass it in
    // `DownloadFromHfArgs`, but we do not currently persist it. If it
    // becomes necessary we add a column to `models` and back-fill.
    // Returning `None` here forces the caller to derive from context.
    None
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

