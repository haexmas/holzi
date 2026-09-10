//! Anthropic HTTP adapter.
//!
//! Reference: <https://docs.claude.com/en/api/models-list> and
//! <https://docs.claude.com/en/api/versioning>. Uses the stable
//! `2023-06-01` version header (see [`ANTHROPIC_VERSION`]). Additive
//! output fields under that version are safe — the deserializer
//! ignores unknown keys.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;

use super::types::{AdapterStream, ChatRequest, ChatRole, StreamChunk, StreamError};
use super::{AdapterError, ProviderAdapter, ProviderModel};

/// The pinned Anthropic API version. Per docs, additive optional
/// inputs and outputs are allowed inside a version without breaking
/// callers.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic caps `limit` at 1000. The catalog is small (~a dozen
/// entries) so one page suffices in practice; the loop paginates on
/// `has_more` for correctness.
const PAGE_LIMIT: u16 = 1000;

/// Whole-request budget (connect + TLS + headers + body). Without it a
/// `base_url` that completes the handshake and then stalls would hang
/// `refresh_provider_models` forever, holding the caller's task.
/// Streaming requests bypass this budget — the token stream is
/// intentionally long-lived — via `Client::post` + no per-request
/// timeout override; a hung mid-stream connection is handled by the
/// caller aborting through `AdapterStream::abort_handle`.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Fallback `max_tokens` cap sent when the request does not carry one.
/// Anthropic Messages API requires the field, so we substitute a
/// conservative default rather than refusing the request.
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Adapter for `api_key`-kind providers pointed at the Anthropic
/// Messages API. Constructed per refresh from the provider row plus
/// its stored credentials blob.
pub struct AnthropicAdapter {
    client: Client,
    base_url: String,
    api_key: String,
}

impl AnthropicAdapter {
    /// Builds an adapter. `base_url` should be
    /// `https://api.anthropic.com` for production; tests pass a
    /// wiremock URL. Trailing slashes on `base_url` are trimmed so
    /// callers do not need to normalise.
    pub fn new(base_url: String, api_key: String) -> Result<Self, AdapterError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            // `timeout` covers the pre-stream request budget; per-call
            // overrides in `list_models` and `stream_chat` still apply.
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("holzi/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| AdapterError::Http {
                reason: format!("build reqwest client: {e}"),
            })?;
        Ok(Self {
            client,
            base_url,
            api_key,
        })
    }
}

#[derive(Deserialize)]
struct ListModelsPage {
    data: Vec<ModelInfo>,
    has_more: bool,
    last_id: Option<String>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
    display_name: String,
    /// Nullable integer in the response. A returned `0` is treated as
    /// "not reported" so the picker does not misrender a zero-width
    /// context.
    #[serde(default)]
    max_input_tokens: Option<i64>,
}

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    async fn list_models(&self) -> Result<Vec<ProviderModel>, AdapterError> {
        let endpoint = format!("{}/v1/models", self.base_url.trim_end_matches('/'));
        let limit_str = PAGE_LIMIT.to_string();
        let mut out: Vec<ProviderModel> = Vec::new();
        let mut after_id: Option<String> = None;

        loop {
            let mut query: Vec<(&str, &str)> = vec![("limit", limit_str.as_str())];
            if let Some(after) = after_id.as_deref() {
                query.push(("after_id", after));
            }
            let resp = self
                .client
                .get(&endpoint)
                .query(&query)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .send()
                .await
                .map_err(|e| AdapterError::Http {
                    reason: format!("GET {endpoint}: {e}"),
                })?;

            let status = resp.status();
            if !status.is_success() {
                let body = truncate_body(resp.text().await.unwrap_or_default());
                return Err(match status.as_u16() {
                    401 | 403 => AdapterError::InvalidCredentials,
                    other => AdapterError::Status {
                        status: other,
                        body,
                    },
                });
            }

            let page: ListModelsPage = resp.json().await.map_err(|e| AdapterError::Parse {
                reason: format!("decode /v1/models: {e}"),
            })?;
            for m in page.data {
                out.push(ProviderModel {
                    remote_id: m.id,
                    display_name: m.display_name,
                    context_window: m.max_input_tokens.filter(|n| *n > 0),
                });
            }
            if !page.has_more {
                break;
            }
            // Only follow the cursor when it actually advances. A
            // provider that keeps answering `has_more: true` with a
            // missing or unchanged `last_id` would otherwise spin
            // forever, re-fetching the same page and growing `out`
            // without bound.
            match page.last_id {
                Some(id) if after_id.as_deref() != Some(id.as_str()) => after_id = Some(id),
                Some(_) => {
                    return Err(AdapterError::Parse {
                        reason: "pagination response has_more=true without an advancing last_id"
                            .into(),
                    });
                }
                None => {
                    return Err(AdapterError::Parse {
                        reason: "pagination response has_more=true without last_id".into(),
                    });
                }
            }
        }
        Ok(out)
    }

    async fn stream_chat(&self, req: ChatRequest) -> Result<AdapterStream, AdapterError> {
        let endpoint = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let body = build_messages_body(&req);
        // The pre-stream POST inherits the client-wide `REQUEST_TIMEOUT`
        // (headers + body), then reqwest hands us `bytes_stream()`
        // which is intentionally long-lived — abort via
        // `AdapterStream::abort_handle` if the caller wants to cancel.
        let resp = self
            .client
            .post(&endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("accept", "text/event-stream")
            .json(&body)
            .send()
            .await
            .map_err(|e| AdapterError::Http {
                reason: format!("POST {endpoint}: {e}"),
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = truncate_body(resp.text().await.unwrap_or_default());
            return Err(match status.as_u16() {
                401 | 403 => AdapterError::InvalidCredentials,
                other => AdapterError::Status {
                    status: other,
                    body: body_text,
                },
            });
        }

        let (tx, rx) = mpsc::unbounded_channel();
        let start = Instant::now();
        let byte_stream = resp.bytes_stream();

        let task = tokio::spawn(async move {
            let mut event_stream = byte_stream.eventsource();
            let mut prompt_tokens: Option<usize> = None;
            let mut completion_tokens: Option<usize> = None;
            let mut ttft_ms: Option<u64> = None;
            let mut finish_reason: Option<String> = None;
            let mut done_emitted = false;
            let mut delta_seen = false;

            while let Some(event) = event_stream.next().await {
                let event = match event {
                    Ok(e) => e,
                    Err(e) => {
                        let _ = tx.send(Err(StreamError::Internal(format!("sse decode: {e}"))));
                        return;
                    }
                };
                let payload: Value = match serde_json::from_str(&event.data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match event.event.as_str() {
                    "message_start" => {
                        if let Some(n) = payload
                            .pointer("/message/usage/input_tokens")
                            .and_then(Value::as_u64)
                        {
                            prompt_tokens = Some(n as usize);
                        }
                    }
                    "content_block_delta" => {
                        let Some(delta_type) =
                            payload.pointer("/delta/type").and_then(Value::as_str)
                        else {
                            continue;
                        };
                        match delta_type {
                            "text_delta" => {
                                if let Some(text) =
                                    payload.pointer("/delta/text").and_then(Value::as_str)
                                {
                                    if !text.is_empty() && ttft_ms.is_none() {
                                        ttft_ms = Some(start.elapsed().as_millis() as u64);
                                    }
                                    if !text.is_empty() {
                                        delta_seen = true;
                                    }
                                    if tx
                                        .send(Ok(StreamChunk::Delta {
                                            content: text.to_string(),
                                            reasoning: None,
                                        }))
                                        .is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                            "thinking_delta" => {
                                if let Some(text) =
                                    payload.pointer("/delta/thinking").and_then(Value::as_str)
                                {
                                    if tx
                                        .send(Ok(StreamChunk::Delta {
                                            content: String::new(),
                                            reasoning: Some(text.to_string()),
                                        }))
                                        .is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    "message_delta" => {
                        if let Some(n) = payload
                            .pointer("/usage/output_tokens")
                            .and_then(Value::as_u64)
                        {
                            completion_tokens = Some(n as usize);
                        }
                        if let Some(reason) = payload
                            .pointer("/delta/stop_reason")
                            .and_then(Value::as_str)
                        {
                            finish_reason = Some(reason.to_string());
                        }
                    }
                    "message_stop" => {
                        let done = StreamChunk::Done {
                            finish_reason: finish_reason.clone(),
                            prompt_tokens,
                            completion_tokens,
                            ttft_ms,
                            total_ms: start.elapsed().as_millis() as u64,
                        };
                        let _ = tx.send(Ok(done));
                        done_emitted = true;
                        break;
                    }
                    "error" => {
                        let message = payload
                            .pointer("/error/message")
                            .and_then(Value::as_str)
                            .unwrap_or("anthropic sse error")
                            .to_string();
                        let _ = tx.send(Err(StreamError::Model(message)));
                        done_emitted = true;
                        break;
                    }
                    _ => {}
                }
            }

            if !done_emitted {
                if delta_seen {
                    let _ = tx.send(Ok(StreamChunk::Done {
                        finish_reason,
                        prompt_tokens,
                        completion_tokens,
                        ttft_ms,
                        total_ms: start.elapsed().as_millis() as u64,
                    }));
                } else {
                    let _ = tx.send(Err(StreamError::UnexpectedEnd));
                }
            }
        });

        Ok(AdapterStream::new(rx, task.abort_handle()))
    }
}

/// Serializes a provider-neutral chat request for Anthropic's Messages API.
fn build_messages_body(req: &ChatRequest) -> Value {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": match m.role {
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                },
                "content": m.content,
            })
        })
        .collect();
    let max_tokens = req
        .max_new_tokens
        .map(|n| n as u32)
        .unwrap_or(DEFAULT_MAX_TOKENS);
    let mut body = serde_json::json!({
        "model": req.model_id,
        "max_tokens": max_tokens,
        "messages": messages,
        "stream": true,
    });
    if let Some(sys) = req.system_prompt.as_ref() {
        body["system"] = Value::String(sys.clone());
    }
    body
}

/// Caps a provider-supplied error body before it travels into a
/// `HolziError` and across the Tauri boundary. A misconfigured
/// `base_url` can answer with a multi-megabyte HTML page, and that
/// belongs in neither an error string nor the UI.
fn truncate_body(mut body: String) -> String {
    const MAX_BODY: usize = 512;
    if body.len() <= MAX_BODY {
        return body;
    }
    let mut end = MAX_BODY;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    body.truncate(end);
    body.push_str("… (truncated)");
    body
}
