//! Anthropic Messages-API request construction.
//!
//! Turns a provider-neutral [`ChatRequest`] into the vendor's wire body:
//! the `max_tokens`/`thinking` interplay, the adaptive-vs-manual
//! thinking decision per model generation, and the regrouping of
//! holzi's flat per-row history into Anthropic's block shape. Kept apart
//! from `anthropic.rs` so the transport and SSE-decoding side of the
//! adapter stays readable on its own.

use serde_json::Value;

use super::types::{ChatRequest, ChatRole};

/// Fallback `max_tokens` cap sent when the request does not carry one.
/// Anthropic Messages API requires the field, so we substitute a
/// conservative default rather than refusing the request.
const DEFAULT_MAX_TOKENS: u32 = 4096;
const MIN_MANUAL_THINKING_MAX_TOKENS: u32 = 1025;
const MIN_MANUAL_THINKING_BUDGET: u32 = 1024;
const MAX_MANUAL_THINKING_BUDGET: u32 = 16_384;

/// Serializes a provider-neutral chat request for Anthropic's Messages API.
pub(super) fn build_messages_body(req: &ChatRequest) -> Value {
    let requested_max_tokens = req
        .max_new_tokens
        .map(|n| n as u32)
        .unwrap_or(DEFAULT_MAX_TOKENS);
    let adaptive_thinking = req.reasoning_requested && supports_adaptive_thinking(&req.model_id);
    let manual_thinking = req.reasoning_requested && !adaptive_thinking;
    // Anthropic requires a manual thinking budget to be >= 1024 and strictly
    // lower than max_tokens. Preserve the caller's cap whenever possible,
    // reserving one output token for small reasoning requests.
    let max_tokens = if manual_thinking {
        requested_max_tokens.max(MIN_MANUAL_THINKING_MAX_TOKENS)
    } else {
        requested_max_tokens
    };
    let mut body = serde_json::json!({
        "model": req.model_id,
        "max_tokens": max_tokens,
        "messages": build_messages(&req.messages),
        "stream": true,
    });
    if let Some(sys) = req.system_prompt.as_ref() {
        body["system"] = Value::String(sys.clone());
    }
    if req.reasoning_requested {
        body["thinking"] = if adaptive_thinking {
            serde_json::json!({"type": "adaptive"})
        } else {
            serde_json::json!({
                "type": "enabled",
                "budget_tokens": (max_tokens - 1)
                    .clamp(MIN_MANUAL_THINKING_BUDGET, MAX_MANUAL_THINKING_BUDGET),
            })
        };
    }
    if !req.tools.is_empty() {
        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect();
        body["tools"] = Value::Array(tools);
    }
    body
}

/// Claude 4.7 and newer use Anthropic's adaptive thinking API. Older Claude
/// models that expose extended thinking still use the manual budget form.
fn supports_adaptive_thinking(model_id: &str) -> bool {
    let lower_model_id = model_id.to_ascii_lowercase();
    let parts: Vec<_> = lower_model_id.split('-').collect();
    let Some((major_index, major)) = parts
        .iter()
        .enumerate()
        .find_map(|(index, part)| part.parse::<u32>().ok().map(|major| (index, major)))
    else {
        return false;
    };
    if parts.first() != Some(&"claude") || major_index < 2 {
        return false;
    }
    if major > 4 {
        return true;
    }
    major == 4
        && parts
            .get(major_index + 1)
            .and_then(|minor| minor.parse::<u32>().ok())
            // Date-stamped ids such as `claude-sonnet-4-20250514` are
            // legacy manual-thinking models, not Claude 4.7 variants.
            .is_some_and(|minor| (7..100).contains(&minor))
}

/// Groups the flat, per-row `messages` history into Anthropic's wire
/// shape: consecutive `ToolCall` rows become one `assistant` message with
/// one `tool_use` block per call, and consecutive `ToolResult` rows become
/// one `user` message with one `tool_result` block per call, in the same
/// order (data-model.md's two-call example).
fn build_messages(messages: &[super::types::ChatMessage]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        match &messages[i].role {
            ChatRole::User => {
                out.push(serde_json::json!({"role": "user", "content": messages[i].content}));
                i += 1;
            }
            ChatRole::Assistant => {
                let text = messages[i].content.clone();
                i += 1;

                let mut blocks = Vec::new();
                if !text.is_empty() {
                    blocks.push(serde_json::json!({
                        "type": "text",
                        "text": text,
                    }));
                }
                while let Some(ChatRole::ToolCall { id, name, input }) =
                    messages.get(i).map(|m| &m.role)
                {
                    blocks.push(serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": input,
                    }));
                    i += 1;
                }
                if blocks.is_empty() {
                    continue;
                }
                if blocks.len() == 1 && !text.is_empty() {
                    out.push(serde_json::json!({"role": "assistant", "content": text}));
                } else {
                    out.push(serde_json::json!({"role": "assistant", "content": blocks}));
                }
            }
            ChatRole::ToolCall { .. } => {
                let mut blocks = Vec::new();
                while let Some(ChatRole::ToolCall { id, name, input }) =
                    messages.get(i).map(|m| &m.role)
                {
                    blocks.push(serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": input,
                    }));
                    i += 1;
                }
                out.push(serde_json::json!({"role": "assistant", "content": blocks}));
            }
            ChatRole::ToolResult { .. } => {
                let mut blocks = Vec::new();
                while let Some(ChatRole::ToolResult {
                    call_id,
                    content,
                    is_error,
                }) = messages.get(i).map(|m| &m.role)
                {
                    let mut block = serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": call_id,
                        "content": content,
                    });
                    if *is_error {
                        block["is_error"] = Value::Bool(true);
                    }
                    blocks.push(block);
                    i += 1;
                }
                out.push(serde_json::json!({"role": "user", "content": blocks}));
            }
        }
    }
    out
}
