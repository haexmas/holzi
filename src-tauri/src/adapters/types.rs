//! Provider-agnostic chat types shared by every adapter.
//!
//! The local `mistralrs` path and the remote `api_key` path both funnel
//! their generated tokens through the same [`AdapterStream`] so the chat
//! dispatch in `chat/commands.rs` does not special-case where the tokens
//! come from.

use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

/// Which speaker a message belongs to. `System` is passed separately in
/// [`ChatRequest::system_prompt`] because both mistralrs and Anthropic
/// treat it as an out-of-band field rather than a message role.
///
/// `ToolCall`/`ToolResult` carry their own payload directly (data-model.md)
/// rather than reusing [`ChatMessage::content`] — each is its own flat
/// entry in [`ChatRequest::messages`]; an adapter groups consecutive
/// entries of the same kind into one wire-level message (research.md §1/§2,
/// data-model.md's two-call example).
#[derive(Debug, Clone, PartialEq)]
pub enum ChatRole {
    User,
    Assistant,
    ToolCall {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        call_id: String,
        content: String,
        is_error: bool,
    },
}

/// `content` is only meaningful for `role: ChatRole::User | Assistant` —
/// `ToolCall`/`ToolResult` carry their payload on the role variant itself
/// and leave this empty.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

/// A tool an adapter may offer the model this step. Reused as-is for
/// Anthropic's `input_schema` and mistralrs' `Function.parameters` — both
/// are JSON-Schema-shaped (research.md §1/§2).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// One tool call the model produced. Reconstructed by an adapter from its
/// own streamed wire format (buffered `input_json_delta`s for Anthropic,
/// `ToolCallResponse` for mistralrs — research.md §1/§2) into this
/// provider-neutral shape.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

/// Complete chat request handed to an adapter. `model_id` is the raw
/// remote id (e.g. `"claude-opus-5"`); the composite `<provider>:<remote>`
/// split happens above the adapter so each adapter only sees the id its
/// provider expects. Local adapters ignore this field — their model is
/// bound at construction.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model_id: String,
    pub system_prompt: Option<String>,
    pub messages: Vec<ChatMessage>,
    /// Cap on completion tokens. `None` uses each adapter's default;
    /// Anthropic Messages API requires the field, so the Anthropic
    /// adapter substitutes a conservative default in that case.
    pub max_new_tokens: Option<usize>,
    /// Tools offered this step. Empty when no tools are registered, or
    /// when the loaded local model's chat template does not support tool
    /// calling (research.md §2 caveat) — an adapter never errors on an
    /// empty list, it just never emits `StreamChunk::ToolCalls`.
    pub tools: Vec<ToolSpec>,
}

/// One event on an [`AdapterStream`]. `Delta` carries either content,
/// reasoning, or both; `ToolCalls` precedes `Done` when the model stopped
/// specifically to call one or more tools; `Done` closes the stream with
/// token counts and timing.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    Delta {
        content: String,
        /// Chain-of-thought text from Harmony-format local models or
        /// Anthropic `thinking_delta` events. `None` for adapters that
        /// do not surface reasoning.
        reasoning: Option<String>,
    },
    ToolCalls(Vec<ToolCall>),
    Done {
        finish_reason: Option<String>,
        prompt_tokens: Option<usize>,
        completion_tokens: Option<usize>,
        /// Time to first non-empty content chunk. `None` when no
        /// content arrived (empty completion or immediate error).
        ttft_ms: Option<u64>,
        total_ms: u64,
    },
}

/// Errors delivered on the stream's `Err` branch. Not all are terminal —
/// see [`StreamError::is_transient`] (spec.md FR-012, tasks.md T034).
#[derive(Debug, thiserror::Error, Clone)]
pub enum StreamError {
    #[error("validation error: {0}")]
    Validation(String),
    #[error("model error: {0}")]
    Model(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("stream ended without a Done frame")]
    UnexpectedEnd,
    #[error("failed to start generation: {0}")]
    StartFailed(String),
    /// A provider-reported condition classified transient at the source
    /// (e.g. Anthropic's `overloaded_error`/`rate_limit_error`/`api_error`
    /// SSE `error` events) — distinct from `Model`, which is a
    /// deterministic-for-this-request provider error.
    #[error("transient provider error: {0}")]
    Transient(String),
}

impl StreamError {
    /// Eligible for the bounded automatic retry (spec.md FR-012): a
    /// transport-level hiccup (`Internal` — e.g. an SSE decode failure from
    /// a dropped connection; `UnexpectedEnd` — the stream closed before a
    /// `Done` frame) or a provider-reported transient condition
    /// (`Transient`). `Model`/`Validation`/`StartFailed` reproduce
    /// deterministically for the same request, so retrying would not help.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            StreamError::Transient(_) | StreamError::Internal(_) | StreamError::UnexpectedEnd
        )
    }
}

/// A live generation. Consume via [`next`](Self::next); cancel by
/// dropping the value or invoking the [`abort_handle`](Self::abort_handle)
/// out of band. Dropping aborts the underlying task, so callers who need
/// to hand the abort out to shared state should clone the handle first.
pub struct AdapterStream {
    rx: mpsc::UnboundedReceiver<Result<StreamChunk, StreamError>>,
    abort: AbortHandle,
}

impl AdapterStream {
    /// Constructs from a sender/receiver pair. Adapters spawn a task
    /// that writes to the sender and pass its abort handle here.
    pub fn new(
        rx: mpsc::UnboundedReceiver<Result<StreamChunk, StreamError>>,
        abort: AbortHandle,
    ) -> Self {
        Self { rx, abort }
    }

    /// Await the next chunk. Returns `None` after a `Done` frame or an
    /// error has been delivered.
    pub async fn next(&mut self) -> Option<Result<StreamChunk, StreamError>> {
        self.rx.recv().await
    }

    /// Cloneable handle for out-of-band cancellation. Chat state stashes
    /// this so `abort_current_generation` can cancel without owning the
    /// full `AdapterStream`.
    pub fn abort_handle(&self) -> AbortHandle {
        self.abort.clone()
    }
}

impl Drop for AdapterStream {
    fn drop(&mut self) {
        // Dropping the receiver also closes the channel, which lets the
        // spawned task exit on its next `send` — but on CUDA a single
        // decode step can take tens of ms, so aborting is quicker.
        self.abort.abort();
    }
}
