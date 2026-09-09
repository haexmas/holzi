//! Cancellable, token-by-token generation on top of
//! [`mistralrs::Model::stream_chat_request`].
//!
//! Etappe 0 finding #5: TTFT is only observable via the streaming API;
//! the non-streaming `send_chat_request` blocks until completion. This
//! module always uses streaming.

use std::time::Instant;

use mistralrs::{RequestBuilder, Response, TextMessageRole, TextMessages};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::LocalModel;

/// One event on the streaming channel returned by
/// [`LocalModel::stream_chat`].
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// Incremental content emitted by the model. `content` may be
    /// empty for role-only frames; `reasoning` carries chain-of-thought
    /// deltas from Harmony-format models and is `None` otherwise.
    Delta {
        content: String,
        reasoning: Option<String>,
    },
    /// Final frame with token counts and TTFT. TTFT is measured from
    /// the moment `stream_chat` starts polling until the first chunk
    /// that carries non-empty content is observed.
    Done {
        finish_reason: Option<String>,
        prompt_tokens: usize,
        completion_tokens: usize,
        ttft_ms: Option<u64>,
        total_ms: u64,
    },
}

/// Terminal errors emitted onto the receiver's `Err` branch.
#[derive(Debug, Error, Clone)]
pub enum StreamError {
    #[error("mistralrs validation error: {0}")]
    Validation(String),
    #[error("mistralrs model error: {0}")]
    Model(String),
    #[error("mistralrs internal error: {0}")]
    Internal(String),
    #[error("stream ended without a Done frame")]
    UnexpectedEnd,
    #[error("stream_chat_request failed to start: {0}")]
    StartFailed(String),
}

/// A single generation run. Drop or call [`GenerationHandle::abort`] to
/// cancel; the spawned task terminates and the mistralrs sender closes.
pub struct GenerationHandle {
    rx: mpsc::UnboundedReceiver<Result<StreamChunk, StreamError>>,
    task: JoinHandle<()>,
}

impl GenerationHandle {
    /// Await the next chunk or error. Returns `None` once the stream
    /// is exhausted (a `Done` chunk or an error was already delivered).
    pub async fn next(&mut self) -> Option<Result<StreamChunk, StreamError>> {
        self.rx.recv().await
    }

    /// Cancel the running generation. The spawned task is aborted and
    /// subsequent `next()` calls return `None` once the channel drains.
    /// Safe to call more than once. Takes `&self` so callers can still
    /// drain any in-flight chunks after aborting.
    pub fn abort(&self) {
        self.task.abort();
    }

    /// Returns a cloneable, Send abort handle for the underlying task.
    /// Chat commands stash this in shared state so an out-of-band
    /// `abort_current_generation` call can cancel without owning the
    /// full `GenerationHandle`.
    pub fn abort_handle(&self) -> tokio::task::AbortHandle {
        self.task.abort_handle()
    }
}

impl Drop for GenerationHandle {
    fn drop(&mut self) {
        // Dropping the handle also drops the receiver, which closes the
        // channel and causes the spawned task's `tx.send` to fail — but
        // aborting explicitly is cheaper and shortens the tail on CUDA
        // where a single decode step can take tens of ms.
        self.task.abort();
    }
}

/// Input for [`LocalModel::stream_chat`]. Kept minimal on purpose;
/// sampling knobs land here as concrete needs surface in Slice (c).
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system_prompt: Option<String>,
    pub messages: Vec<ChatMessage>,
    /// Cap on completion tokens. `None` uses the model's default.
    pub max_new_tokens: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

impl ChatRole {
    fn to_mistralrs(self) -> TextMessageRole {
        match self {
            ChatRole::User => TextMessageRole::User,
            ChatRole::Assistant => TextMessageRole::Assistant,
        }
    }
}

impl LocalModel {
    /// Starts a streaming chat generation. Returns immediately; the
    /// generation runs on a background task and pushes into the
    /// returned [`GenerationHandle`].
    pub fn stream_chat(&self, req: ChatRequest) -> GenerationHandle {
        let (tx, rx) = mpsc::unbounded_channel();
        let model = self.inner();
        let start = Instant::now();

        let task = tokio::spawn(async move {
            let mut messages = TextMessages::new();
            if let Some(sys) = req.system_prompt.as_ref() {
                messages = messages.add_message(TextMessageRole::System, sys);
            }
            for m in &req.messages {
                messages = messages.add_message(m.role.to_mistralrs(), &m.content);
            }

            let mut builder = RequestBuilder::from(messages);
            if let Some(cap) = req.max_new_tokens {
                builder = builder.set_sampler_max_len(cap);
            }

            let mut stream = match model.stream_chat_request(builder).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Err(StreamError::StartFailed(e.to_string())));
                    return;
                }
            };

            let mut ttft_ms: Option<u64> = None;
            let mut done_emitted = false;

            while let Some(response) = stream.next().await {
                match response {
                    Response::Chunk(chunk) => {
                        let mut content = String::new();
                        let mut reasoning: Option<String> = None;
                        for choice in chunk.choices {
                            if let Some(c) = choice.delta.content {
                                content.push_str(&c);
                            }
                            if let Some(r) = choice.delta.reasoning_content {
                                reasoning
                                    .get_or_insert_with(String::new)
                                    .push_str(&r);
                            }
                        }
                        if ttft_ms.is_none() && !content.is_empty() {
                            ttft_ms = Some(start.elapsed().as_millis() as u64);
                        }
                        if tx
                            .send(Ok(StreamChunk::Delta { content, reasoning }))
                            .is_err()
                        {
                            // Receiver dropped: caller cancelled.
                            return;
                        }
                    }
                    Response::Done(final_resp) => {
                        let finish_reason = final_resp
                            .choices
                            .first()
                            .map(|c| c.finish_reason.clone());
                        let done = StreamChunk::Done {
                            finish_reason,
                            prompt_tokens: final_resp.usage.prompt_tokens,
                            completion_tokens: final_resp.usage.completion_tokens,
                            ttft_ms,
                            total_ms: start.elapsed().as_millis() as u64,
                        };
                        let _ = tx.send(Ok(done));
                        done_emitted = true;
                        break;
                    }
                    Response::ModelError(msg, _partial) => {
                        let _ = tx.send(Err(StreamError::Model(msg)));
                        done_emitted = true;
                        break;
                    }
                    Response::ValidationError(e) => {
                        let _ = tx.send(Err(StreamError::Validation(e.to_string())));
                        done_emitted = true;
                        break;
                    }
                    Response::InternalError(e) => {
                        let _ = tx.send(Err(StreamError::Internal(e.to_string())));
                        done_emitted = true;
                        break;
                    }
                    // Non-chat variants (completion/image/speech/raw) cannot
                    // arrive here because we only submit chat requests, but
                    // enum exhaustiveness demands a branch. `Response` does
                    // not implement `Debug`, so we cannot format it.
                    _ => {
                        let _ = tx.send(Err(StreamError::Internal(
                            "unexpected non-chat response variant".to_string(),
                        )));
                        done_emitted = true;
                        break;
                    }
                }
            }

            if !done_emitted {
                let _ = tx.send(Err(StreamError::UnexpectedEnd));
            }
        });

        GenerationHandle { rx, task }
    }
}
