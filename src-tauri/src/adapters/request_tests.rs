//! Tests for Anthropic request-body construction. The pure cases call
//! [`build_messages_body`] directly; the two `wiremock` cases assert the
//! body as it actually leaves the adapter, which is the only way to prove
//! the streaming path serializes what these helpers build.

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::anthropic::AnthropicAdapter;
use super::effort::EffortLevel;
use super::request::build_messages_body;
use super::types::{Attachment, AttachmentKind, ChatMessage, ChatRequest, ChatRole};
use super::ProviderAdapter;

fn sample_request(model: &str) -> ChatRequest {
    ChatRequest {
        model_id: model.to_string(),
        thread_id: None,
        system_prompt: None,
        messages: vec![ChatMessage {
            role: ChatRole::User,
            attachments: Vec::new(),
            content: "hi".to_string(),
        }],
        reasoning_requested: false,
        max_new_tokens: Some(128),
        tools: Vec::new(),
        autonomy_mode: Default::default(),
        effort_level: Default::default(),
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

#[test]
fn effort_level_is_omitted_when_not_requested() {
    let request = sample_request("claude-sonnet-5");
    let body = build_messages_body(&request);
    assert!(body.get("output_config").is_none());
}

#[test]
fn effort_level_is_sent_when_the_model_supports_it() {
    let mut request = sample_request("claude-sonnet-5");
    request.effort_level = Some(EffortLevel::XHigh);
    let body = build_messages_body(&request);
    assert_eq!(body["output_config"]["effort"], "xhigh");
}

#[test]
fn effort_level_clamps_down_for_a_model_without_that_level() {
    let mut request = sample_request("claude-opus-4-6");
    request.effort_level = Some(EffortLevel::XHigh);
    let body = build_messages_body(&request);
    // claude-opus-4-6 supports `max` but not `xhigh` (research.md §1) — the
    // request must clamp down to the nearest supported level, never send
    // an unsupported one.
    assert_eq!(body["output_config"]["effort"], "high");
}

#[test]
fn effort_level_is_never_sent_for_a_model_outside_the_support_table() {
    let mut request = sample_request("claude-3-5-sonnet-20241022");
    request.effort_level = Some(EffortLevel::Low);
    let body = build_messages_body(&request);
    assert!(body.get("output_config").is_none());
}

#[test]
fn a_user_message_with_no_attachments_stays_a_plain_string() {
    let request = sample_request("claude-sonnet-5");
    let body = build_messages_body(&request);
    assert_eq!(body["messages"][0]["content"], "hi");
}

#[test]
fn an_image_attachment_becomes_a_base64_image_block_alongside_the_text() {
    let mut request = sample_request("claude-sonnet-5");
    request.messages[0].attachments.push(Attachment {
        name: "photo.png".to_string(),
        kind: AttachmentKind::Image,
        media_type: "image/png".to_string(),
        bytes: vec![1, 2, 3],
    });
    let body = build_messages_body(&request);
    let content = &body["messages"][0]["content"];
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "hi");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["media_type"], "image/png");
    assert!(!content[1]["source"]["data"].as_str().unwrap().is_empty());
}

#[test]
fn a_text_attachment_is_inlined_as_an_extra_text_block() {
    let mut request = sample_request("claude-sonnet-5");
    request.messages[0].attachments.push(Attachment {
        name: "notes.txt".to_string(),
        kind: AttachmentKind::Text,
        media_type: "text/plain".to_string(),
        bytes: b"hello from a file".to_vec(),
    });
    let body = build_messages_body(&request);
    let content = &body["messages"][0]["content"];
    assert_eq!(content[1]["type"], "text");
    assert!(content[1]["text"].as_str().unwrap().contains("notes.txt"));
    assert!(content[1]["text"]
        .as_str()
        .unwrap()
        .contains("hello from a file"));
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
                attachments: Vec::new(),
                content: "hi".to_string(),
            },
            ChatMessage {
                role: ChatRole::Assistant,
                attachments: Vec::new(),
                content: "I will check that.".to_string(),
            },
            ChatMessage {
                role: ChatRole::ToolCall {
                    id: "call-a".to_string(),
                    name: "first".to_string(),
                    input: serde_json::json!({"x": 1}),
                },
                attachments: Vec::new(),
                content: String::new(),
            },
            ChatMessage {
                role: ChatRole::ToolCall {
                    id: "call-b".to_string(),
                    name: "second".to_string(),
                    input: serde_json::json!({"y": 2}),
                },
                attachments: Vec::new(),
                content: String::new(),
            },
            ChatMessage {
                role: ChatRole::ToolResult {
                    call_id: "call-a".to_string(),
                    content: "result-a".to_string(),
                    is_error: false,
                },
                attachments: Vec::new(),
                content: String::new(),
            },
            ChatMessage {
                role: ChatRole::ToolResult {
                    call_id: "call-b".to_string(),
                    content: "result-b".to_string(),
                    is_error: true,
                },
                attachments: Vec::new(),
                content: String::new(),
            },
        ],
        reasoning_requested: false,
        max_new_tokens: Some(128),
        tools: Vec::new(),
        autonomy_mode: Default::default(),
        effort_level: Default::default(),
    };

    let mut stream = adapter.stream_chat(request).await.unwrap();
    while stream.next().await.is_some() {}
}
