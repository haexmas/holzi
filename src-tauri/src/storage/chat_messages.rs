//! Typed writes and reads for the `chat_messages` table.
//!
//! Only finalised messages land here (plan §"Datenmodell": in-flight
//! generations are staged locally until the model finishes or is
//! cancelled). Every insert therefore already knows its
//! `finish_reason` and token counts.

use haex_crdt::crdt::columns::HLC_TIMESTAMP_COLUMN;
use haex_crdt::rusqlite::{params, Connection, OptionalExtension, Result};
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
    /// Set only on the user-message row of a `send_message` call
    /// (contract §send_message). `None` for every assistant/system row
    /// and for messages inserted before migration 0013.
    pub idempotency_key: Option<String>,
    /// Tool name called. Set only on `role = ToolCall` (data-model.md).
    pub tool_name: Option<String>,
    /// Correlates a `ToolCall` row with its `ToolResult` row. Set on both
    /// roles; no hard FK, same rationale as `parent_id`.
    pub tool_call_id: Option<String>,
    /// JSON text of the tool input. Set only on `role = ToolCall`.
    pub tool_input: Option<String>,
    /// `None`/`Some(false)` both mean "no error" (pre-migration rows have
    /// `None`). Set only on `role = ToolResult`.
    pub tool_is_error: Option<bool>,
    /// `mcp` or `cli`. Set only on `role = ToolCall`.
    pub tool_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    ToolCall,
    ToolResult,
}

impl MessageRole {
    fn as_str(self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::ToolCall => "tool_call",
            MessageRole::ToolResult => "tool_result",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(MessageRole::User),
            "assistant" => Some(MessageRole::Assistant),
            "system" => Some(MessageRole::System),
            "tool_call" => Some(MessageRole::ToolCall),
            "tool_result" => Some(MessageRole::ToolResult),
            _ => None,
        }
    }
}

/// A `chat_messages` row's tool columns are inconsistent with its `role`
/// (data-model.md validation rules). Surfaced from [`insert_message`] as a
/// synthetic [`haex_crdt::rusqlite::Error::ToSqlConversionFailure`] — this
/// module already does the equivalent for malformed reads (see
/// [`bad_enum`]), so a write-side rejection follows the same pattern rather
/// than widening `insert_message`'s return type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChatMessageValidationError {
    #[error("tool_call row requires tool_name, tool_call_id, tool_input, and tool_source")]
    ToolCallMissingFields,
    #[error("tool_call row must not set tool_is_error")]
    ToolCallHasToolIsError,
    #[error("tool_result row requires tool_call_id and tool_is_error")]
    ToolResultMissingFields,
    #[error("tool_result row must not set tool_name, tool_input, or tool_source")]
    ToolResultHasToolCallFields,
    #[error("role {role:?} must not set any tool_* column")]
    ToolColumnsOnNonToolRole { role: MessageRole },
}

/// Checks a message's tool columns against its `role` (data-model.md).
/// Pure — no I/O, so it is covered directly by the colocated
/// `chat_messages_tests` rather than the DB-backed integration suite.
pub fn validate(m: &ChatMessage) -> std::result::Result<(), ChatMessageValidationError> {
    match m.role {
        MessageRole::ToolCall => {
            if m.tool_name.is_none()
                || m.tool_call_id.is_none()
                || m.tool_input.is_none()
                || m.tool_source.is_none()
            {
                return Err(ChatMessageValidationError::ToolCallMissingFields);
            }
            if m.tool_is_error.is_some() {
                return Err(ChatMessageValidationError::ToolCallHasToolIsError);
            }
        }
        MessageRole::ToolResult => {
            if m.tool_call_id.is_none() || m.tool_is_error.is_none() {
                return Err(ChatMessageValidationError::ToolResultMissingFields);
            }
            if m.tool_name.is_some() || m.tool_input.is_some() || m.tool_source.is_some() {
                return Err(ChatMessageValidationError::ToolResultHasToolCallFields);
            }
        }
        MessageRole::User | MessageRole::Assistant | MessageRole::System => {
            if m.tool_name.is_some()
                || m.tool_call_id.is_some()
                || m.tool_input.is_some()
                || m.tool_is_error.is_some()
                || m.tool_source.is_some()
            {
                return Err(ChatMessageValidationError::ToolColumnsOnNonToolRole { role: m.role });
            }
        }
    }
    Ok(())
}

fn validation_error(e: ChatMessageValidationError) -> haex_crdt::rusqlite::Error {
    haex_crdt::rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Complete,
    Cancelled,
    Error,
    /// The turn hit its fixed round cap without a final answer (spec.md
    /// FR-016). Distinguishable from `Error` and `Cancelled` (contracts.md).
    ToolLimitReached,
}

impl FinishReason {
    fn as_str(self) -> &'static str {
        match self {
            FinishReason::Complete => "complete",
            FinishReason::Cancelled => "cancelled",
            FinishReason::Error => "error",
            FinishReason::ToolLimitReached => "tool_limit_reached",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "complete" => Some(FinishReason::Complete),
            "cancelled" => Some(FinishReason::Cancelled),
            "error" => Some(FinishReason::Error),
            "tool_limit_reached" => Some(FinishReason::ToolLimitReached),
            _ => None,
        }
    }
}

/// Inserts a finalised message. Always sets `haex_hlc_no_sync = current_hlc()`.
/// A child must sort after its persisted parent, even when the wall clock
/// moves backwards or an earlier tool round advanced logical timestamps.
pub fn insert_message(conn: &Connection, m: &ChatMessage) -> Result<usize> {
    validate(m).map_err(validation_error)?;
    let sql = format!(
        "INSERT INTO chat_messages \
           (id, thread_id, parent_id, role, content, \
            provider_id, model_id, prompt_tokens, completion_tokens, \
            finish_reason, created_at, idempotency_key, \
            tool_name, tool_call_id, tool_input, tool_is_error, tool_source, \
            {HLC_TIMESTAMP_COLUMN}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, \
                 MAX(?11, COALESCE((SELECT created_at + 1 FROM chat_messages \
                                   WHERE id = ?3 AND thread_id = ?2), ?11)), ?12, \
                 ?13, ?14, ?15, ?16, ?17, current_hlc())"
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
            m.idempotency_key,
            m.tool_name,
            m.tool_call_id,
            m.tool_input,
            m.tool_is_error,
            m.tool_source,
        ],
    )
}

/// Looks up the user-message row carrying the given `idempotency_key`,
/// if any. Used by `send_message` to dedup retries before minting a new
/// insert (contract §send_message).
pub fn find_by_idempotency_key(conn: &Connection, key: &str) -> Result<Option<ChatMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, thread_id, parent_id, role, content, \
                provider_id, model_id, prompt_tokens, completion_tokens, \
                finish_reason, created_at, idempotency_key, \
                tool_name, tool_call_id, tool_input, tool_is_error, tool_source \
         FROM chat_messages WHERE idempotency_key = ?1",
    )?;
    stmt.query_row(params![key], row_to_message).optional()
}

/// Removes a message that was staged before adapter startup completed.
pub fn delete_message(conn: &Connection, id: Uuid) -> Result<usize> {
    conn.execute(
        "DELETE FROM chat_messages WHERE id = ?1",
        params![id.to_string()],
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
                finish_reason, created_at, idempotency_key, \
                tool_name, tool_call_id, tool_input, tool_is_error, tool_source \
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
        idempotency_key: row.get(11)?,
        tool_name: row.get(12)?,
        tool_call_id: row.get(13)?,
        tool_input: row.get(14)?,
        tool_is_error: row.get(15)?,
        tool_source: row.get(16)?,
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
