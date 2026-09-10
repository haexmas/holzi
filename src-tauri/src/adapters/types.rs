//! Provider-agnostic chat types shared by every adapter.
//!
//! The local `mistralrs` path and the remote `api_key` path both funnel
//! their generated tokens through the same [`AdapterStream`] so the chat
//! dispatch in `chat/commands.rs` does not special-case where the tokens
//! come from.

use tokio::sync::mpsc;
use tokio::task::AbortHandle;

/// Which speaker a message belongs to. `System` is passed separately in
/// [`ChatRequest::system_prompt`] because both mistralrs and Anthropic
/// treat it as an out-of-band field rather than a message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
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
}

/// One event on an [`AdapterStream`]. `Delta` carries either content,
/// reasoning, or both; `Done` closes the stream with token counts and
/// timing.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    Delta {
        content: String,
        /// Chain-of-thought text from Harmony-format local models or
        /// Anthropic `thinking_delta` events. `None` for adapters that
        /// do not surface reasoning.
        reasoning: Option<String>,
    },
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

/// Terminal errors delivered on the stream's `Err` branch.
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
