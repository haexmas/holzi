//! Cancellable, token-by-token generation on top of
//! [`mistralrs::Model::stream_chat_request`].
//!
//! Etappe 0 finding #5: TTFT is only observable via the streaming API;
//! the non-streaming `send_chat_request` blocks until completion. This
//! module always uses streaming.
//!
//! The public request / chunk / error types live in
//! [`crate::adapters::types`] so both the local and remote paths speak
//! the same shape; `LocalModel::stream_chat` returns an
//! [`AdapterStream`] directly.

use std::time::Instant;

use mistralrs::{RequestBuilder, Response, TextMessageRole, TextMessages};
use tokio::sync::mpsc;

use super::LocalModel;
use crate::adapters::types::{AdapterStream, ChatRequest, ChatRole, StreamChunk, StreamError};

fn role_to_mistralrs(role: ChatRole) -> TextMessageRole {
    match role {
        ChatRole::User => TextMessageRole::User,
        ChatRole::Assistant => TextMessageRole::Assistant,
    }
}

impl LocalModel {
    /// Starts a streaming chat generation. Returns immediately; the
    /// generation runs on a background task and pushes into the
    /// returned [`AdapterStream`].
    pub fn stream_chat(&self, req: ChatRequest) -> AdapterStream {
        let (tx, rx) = mpsc::unbounded_channel();
        let model = self.inner();
        let start = Instant::now();

        let task = tokio::spawn(async move {
            let mut messages = TextMessages::new();
            if let Some(sys) = req.system_prompt.as_ref() {
                messages = messages.add_message(TextMessageRole::System, sys);
            }
            for m in &req.messages {
                messages = messages.add_message(role_to_mistralrs(m.role), &m.content);
            }
            // `ChatRequest::model_id` is ignored here — for local runs
            // the model is bound at `LocalAdapter::new` time.

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
            let mut delta_emitted = false;
            let mut last_finish_reason: Option<String> = None;

            while let Some(response) = stream.next().await {
                match response {
                    Response::Chunk(chunk) => {
                        let mut content = String::new();
                        let mut reasoning: Option<String> = None;
                        for choice in &chunk.choices {
                            if let Some(c) = &choice.delta.content {
                                content.push_str(c);
                            }
                            if let Some(r) = &choice.delta.reasoning_content {
                                reasoning.get_or_insert_with(String::new).push_str(r);
                            }
                            if let Some(fr) = &choice.finish_reason {
                                last_finish_reason = Some(fr.clone());
                            }
                        }
                        if ttft_ms.is_none() && !content.is_empty() {
                            ttft_ms = Some(start.elapsed().as_millis() as u64);
                        }
                        if !content.is_empty() {
                            delta_emitted = true;
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
                        let finish_reason =
                            final_resp.choices.first().map(|c| c.finish_reason.clone());
                        let done = StreamChunk::Done {
                            finish_reason,
                            prompt_tokens: Some(final_resp.usage.prompt_tokens),
                            completion_tokens: Some(final_resp.usage.completion_tokens),
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
                if delta_emitted {
                    // mistralrs 0.8.1 has been observed to close the stream
                    // on CUDA without emitting a final `Response::Done`
                    // when the model hits its max_new_tokens cap. Treat a
                    // clean stream close after we saw content as a
                    // synthetic completion — token counts are unknown
                    // because they only reach us in the Done frame.
                    let synth = StreamChunk::Done {
                        finish_reason: last_finish_reason,
                        prompt_tokens: None,
                        completion_tokens: None,
                        ttft_ms,
                        total_ms: start.elapsed().as_millis() as u64,
                    };
                    let _ = tx.send(Ok(synth));
                } else {
                    let _ = tx.send(Err(StreamError::UnexpectedEnd));
                }
            }
        });

        let abort = task.abort_handle();
        // Only the abort handle is retained; `tokio::spawn` keeps the
        // task running regardless of the JoinHandle being dropped.
        AdapterStream::new(rx, abort)
    }
}
