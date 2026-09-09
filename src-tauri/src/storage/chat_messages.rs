//! Typed writes and reads for the `chat_messages` table.
//!
//! Only finalised messages land here (plan §"Datenmodell": in-flight
//! generations are staged locally until the model finishes or is
//! cancelled). Every insert therefore already knows its
//! `finish_reason` and token counts.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub role: MessageRole,
    pub content: String,
    pub provider_id: Option<Uuid>,
    pub model_id: Option<String>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub finish_reason: Option<FinishReason>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl MessageRole {
    fn as_str(self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(MessageRole::User),
            "assistant" => Some(MessageRole::Assistant),
            "system" => Some(MessageRole::System),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Complete,
    Cancelled,
    Error,
}

impl FinishReason {
    fn as_str(self) -> &'static str {
        match self {
            FinishReason::Complete => "complete",
            FinishReason::Cancelled => "cancelled",
            FinishReason::Error => "error",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "complete" => Some(FinishReason::Complete),
            "cancelled" => Some(FinishReason::Cancelled),
            "error" => Some(FinishReason::Error),
            _ => None,
        }
    }
}

/// Inserts a finalised message. Always sets `haex_hlc_no_sync = current_hlc()`.
pub fn insert_message(conn: &Connection, m: &ChatMessage) -> Result<usize> {
    let sql = format!(
        "INSERT INTO chat_messages \
           (id, thread_id, parent_id, role, content, \
            provider_id, model_id, prompt_tokens, completion_tokens, \
            finish_reason, created_at, {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, current_hlc())"
    );
    conn.execute(
        &sql,
        params![
            m.id.to_string(),
            m.thread_id.to_string(),
            m.parent_id.map(|u| u.to_string()),
            m.role.as_str(),
            m.content,
            m.provider_id.map(|u| u.to_string()),
            m.model_id,
            m.prompt_tokens,
            m.completion_tokens,
            m.finish_reason.map(|r| r.as_str()),
            m.created_at,
        ],
    )
}

/// Lists all messages in a thread, oldest first. Deterministic order
/// matters for the "conversation history" render — plan §"Datenmodell"
/// demands per-thread ordering by logical time + id; this helper uses
/// `created_at` then `id` as a stable proxy sufficient for the MVP
/// (single-writer path). Full CRDT ordering lands with sync.
pub fn list_messages(conn: &Connection, thread_id: Uuid) -> Result<Vec<ChatMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, thread_id, parent_id, role, content, \
                provider_id, model_id, prompt_tokens, completion_tokens, \
                finish_reason, created_at \
         FROM chat_messages WHERE thread_id = ?1 \
         ORDER BY created_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![thread_id.to_string()], row_to_message)?;
    rows.collect()
}

fn row_to_message(row: &haex_crdt::rusqlite::Row<'_>) -> Result<ChatMessage> {
    let id_str: String = row.get(0)?;
    let thread_id_str: String = row.get(1)?;
    let parent_id_str: Option<String> = row.get(2)?;
    let provider_id_str: Option<String> = row.get(5)?;
    let role_str: String = row.get(3)?;
    let finish_str: Option<String> = row.get(9)?;

    Ok(ChatMessage {
        id: parse_uuid(&id_str, 0)?,
        thread_id: parse_uuid(&thread_id_str, 1)?,
        parent_id: parent_id_str.map(|s| parse_uuid(&s, 2)).transpose()?,
        role: MessageRole::parse(&role_str).ok_or_else(|| bad_enum(3, &role_str))?,
        content: row.get(4)?,
        provider_id: provider_id_str.map(|s| parse_uuid(&s, 5)).transpose()?,
        model_id: row.get(6)?,
        prompt_tokens: row.get(7)?,
        completion_tokens: row.get(8)?,
        finish_reason: match finish_str.as_deref() {
            None => None,
            Some(s) => Some(FinishReason::parse(s).ok_or_else(|| bad_enum(9, s))?),
        },
        created_at: row.get(10)?,
    })
}

fn parse_uuid(s: &str, col: usize) -> Result<Uuid> {
    Uuid::parse_str(s).map_err(|e| {
        haex_crdt::rusqlite::Error::FromSqlConversionFailure(
            col,
            haex_crdt::rusqlite::types::Type::Text,
            Box::new(e),
        )
    })
}

fn bad_enum(col: usize, value: &str) -> haex_crdt::rusqlite::Error {
    haex_crdt::rusqlite::Error::FromSqlConversionFailure(
        col,
        haex_crdt::rusqlite::types::Type::Text,
        format!("unexpected enum value: {value}").into(),
    )
}
