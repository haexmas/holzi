//! Tests for Anthropic request-body construction. The pure cases call
//! [`build_messages_body`] directly; the two `wiremock` cases assert the
//! body as it actually leaves the adapter, which is the only way to prove
//! the streaming path serializes what these helpers build.

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::anthropic::AnthropicAdapter;
use super::request::build_messages_body;
use super::types::{ChatMessage, ChatRequest, ChatRole};
use super::ProviderAdapter;

fn sample_request(model: &str) -> ChatRequest {
    ChatRequest {
        model_id: model.to_string(),
        thread_id: None,
        system_prompt: None,
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: "hi".to_string(),
        }],
        reasoning_requested: false,
        max_new_tokens: Some(128),
        tools: Vec::new(),
        autonomy_mode: Default::default(),
}
}

#[test]
fn reasoning_capability_controls_anthropic_thinking_request() {
    let mut request = sample_request("claude-sonnet-4-20250514");
    let without_reasoning = build_messages_body(&request);
    assert!(without_reasoning.get("thinking").is_none());

    request.reasoning_requested = true;
    let with_reasoning = build_messages_body(&request);
    assert_eq!(with_reasoning["thinking"]["type"], "enabled");
    let budget = with_reasoning["thinking"]["budget_tokens"]
        .as_u64()
        .expect("manual thinking has a budget");
    assert!(budget >= 1024);
    assert!(budget < with_reasoning["max_tokens"].as_u64().unwrap());

    request.model_id = "claude-opus-4-7".to_string();
    let adaptive = build_messages_body(&request);
    assert_eq!(adaptive["thinking"]["type"], "adaptive");
    assert!(adaptive["thinking"].get("budget_tokens").is_none());
}

fn sse_body(events: &[(&str, serde_json::Value)]) -> String {
    let mut out = String::new();
    for (event, data) in events {
        out.push_str(&format!("event: {event}\n"));
        out.push_str(&format!("data: {}\n\n", data));
    }
    out
}

#[tokio::test]
async fn stream_chat_sends_request_body_with_model_and_stream_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(body_json(serde_json::json!({
            "model": "claude-opus-5",
            "max_tokens": 128,
            "messages": [{"role": "user", "content": "hi"}],
            "stream": true,
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(sse_body(&[(
                    "message_stop",
                    serde_json::json!({"type": "message_stop"}),
                )]))
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-any".to_string()).unwrap();
    let mut stream = adapter
        .stream_chat(sample_request("claude-opus-5"))
        .await
        .unwrap();
    while stream.next().await.is_some() {}
}

#[tokio::test]
/// Two ordered tool calls reconstruct as exactly one `assistant` message
/// with two `tool_use` blocks followed by one `user` message with the
/// matching `tool_result` blocks, in the same order and with matching ids
/// (data-model.md's two-call example).
async fn stream_chat_groups_ordered_tool_calls_and_results_into_two_messages() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(body_json(serde_json::json!({
            "model": "claude-opus-5",
            "max_tokens": 128,
            "messages": [
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": [
                    {"type": "text", "text": "I will check that."},
                    {"type": "tool_use", "id": "call-a", "name": "first", "input": {"x": 1}},
                    {"type": "tool_use", "id": "call-b", "name": "second", "input": {"y": 2}},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call-a", "content": "result-a"},
                    {"type": "tool_result", "tool_use_id": "call-b", "content": "result-b", "is_error": true},
                ]},
            ],
            "stream": true,
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(sse_body(&[(
                    "message_stop",
                    serde_json::json!({"type": "message_stop"}),
                )]))
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-any".to_string()).unwrap();
    let request = ChatRequest {
        model_id: "claude-opus-5".to_string(),
        thread_id: None,
        system_prompt: None,
        messages: vec![
            ChatMessage {
                role: ChatRole::User,
                content: "hi".to_string(),
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: "I will check that.".to_string(),
            },
            ChatMessage {
                role: ChatRole::ToolCall {
                    id: "call-a".to_string(),
                    name: "first".to_string(),
                    input: serde_json::json!({"x": 1}),
                },
                content: String::new(),
            },
            ChatMessage {
                role: ChatRole::ToolCall {
                    id: "call-b".to_string(),
                    name: "second".to_string(),
                    input: serde_json::json!({"y": 2}),
                },
                content: String::new(),
            },
            ChatMessage {
                role: ChatRole::ToolResult {
                    call_id: "call-a".to_string(),
                    content: "result-a".to_string(),
                    is_error: false,
                },
                content: String::new(),
            },
            ChatMessage {
                role: ChatRole::ToolResult {
                    call_id: "call-b".to_string(),
                    content: "result-b".to_string(),
                    is_error: true,
                },
                content: String::new(),
            },
        ],
        reasoning_requested: false,
        max_new_tokens: Some(128),
        tools: Vec::new(),
        autonomy_mode: Default::default(),
};

    let mut stream = adapter.stream_chat(request).await.unwrap();
    while stream.next().await.is_some() {}
}
