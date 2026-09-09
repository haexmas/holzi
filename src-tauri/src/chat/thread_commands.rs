//! Thread + message CRUD, distinct from the streaming `send_message`
//! path in `commands.rs`.

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::chat_messages::{self as msg_store, ChatMessage};
use crate::storage::chat_threads::{self as thread_store, ChatThread};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateThreadArgs {
    pub title: Option<String>,
}

/// Creates an empty thread. Callers typically let `send_message`
/// create the thread implicitly; this command exists for a "New chat"
/// button that wants a stable id before the first message.
#[tauri::command]
pub async fn create_thread(
    state: State<'_, AppState>,
    args: CreateThreadArgs,
) -> Result<ThreadPayload> {
    let db = active_database(&state)?;
    let now = now_ms();
    let thread = ChatThread {
        id: Uuid::new_v4(),
        title: args.title.unwrap_or_else(|| "New chat".to_string()),
        last_provider_id: None,
        last_model_id: None,
        created_at: now,
        updated_at: now,
    };
    let inserted = thread.clone();
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            thread_store::insert_thread(conn, &inserted).map_err(haex_crdt::Error::from)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("create_thread join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(thread.into())
}

#[tauri::command]
pub async fn list_threads(state: State<'_, AppState>) -> Result<Vec<ThreadPayload>> {
    let db = active_database(&state)?;
    let rows = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| thread_store::list_threads(conn).map_err(haex_crdt::Error::from))
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("list_threads join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn list_messages(
    state: State<'_, AppState>,
    thread_id: Uuid,
) -> Result<Vec<MessagePayload>> {
    let db = active_database(&state)?;
    let rows = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("list_messages join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPayload {
    pub id: Uuid,
    pub title: String,
    pub last_model_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<ChatThread> for ThreadPayload {
    fn from(t: ChatThread) -> Self {
        ThreadPayload {
            id: t.id,
            title: t.title,
            last_model_id: t.last_model_id,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePayload {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub role: String,
    pub content: String,
    pub model_id: Option<String>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub finish_reason: Option<String>,
    pub created_at: i64,
}

impl From<ChatMessage> for MessagePayload {
    fn from(m: ChatMessage) -> Self {
        MessagePayload {
            id: m.id,
            thread_id: m.thread_id,
            parent_id: m.parent_id,
            role: match m.role {
                crate::storage::chat_messages::MessageRole::User => "user".into(),
                crate::storage::chat_messages::MessageRole::Assistant => "assistant".into(),
                crate::storage::chat_messages::MessageRole::System => "system".into(),
            },
            content: m.content,
            model_id: m.model_id,
            prompt_tokens: m.prompt_tokens,
            completion_tokens: m.completion_tokens,
            finish_reason: m.finish_reason.map(|r| match r {
                crate::storage::chat_messages::FinishReason::Complete => "complete".into(),
                crate::storage::chat_messages::FinishReason::Cancelled => "cancelled".into(),
                crate::storage::chat_messages::FinishReason::Error => "error".into(),
            }),
            created_at: m.created_at,
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
