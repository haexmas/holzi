//! Tests for the Anthropic adapter using `wiremock` as a local HTTP
//! double. Each test spins its own server on a random port so tests
//! can run in parallel without cross-talk.

use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::anthropic::AnthropicAdapter;
use super::types::{ChatMessage, ChatRequest, ChatRole, StreamChunk, StreamError};
use super::{AdapterError, ProviderAdapter};

const AUTH_HEADER: &str = "x-api-key";
const VERSION_HEADER: &str = "anthropic-version";
const VERSION_VALUE: &str = "2023-06-01";

#[tokio::test]
async fn list_models_returns_single_page_entries_with_context_window() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header(AUTH_HEADER, "sk-test-key"))
        .and(header(VERSION_HEADER, VERSION_VALUE))
        .and(query_param("limit", "1000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {
                    "id": "claude-opus-5",
                    "display_name": "Claude Opus 5",
                    "type": "model",
                    "created_at": "2026-07-24T00:00:00Z",
                    "max_input_tokens": 200000,
                },
                {
                    "id": "claude-haiku-4-5-20251001",
                    "display_name": "Claude Haiku 4.5",
                    "type": "model",
                    "created_at": "2025-10-01T00:00:00Z",
                    "max_input_tokens": null,
                }
            ],
            "has_more": false,
            "first_id": "claude-opus-5",
            "last_id": "claude-haiku-4-5-20251001"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-test-key".to_string()).unwrap();
    let models = adapter.list_models().await.expect("list_models succeeds");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0].remote_id, "claude-opus-5");
    assert_eq!(models[0].display_name, "Claude Opus 5");
    assert_eq!(models[0].context_window, Some(200000));
    assert_eq!(models[1].remote_id, "claude-haiku-4-5-20251001");
    assert!(models[1].context_window.is_none());
}

#[tokio::test]
async fn list_models_paginates_on_has_more() {
    let server = MockServer::start().await;

    // First page: has_more=true, last_id set → adapter must issue a
    // second GET with after_id.
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(query_param("limit", "1000"))
        .and(query_param_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {"id": "model-a", "display_name": "A", "type": "model",
                 "created_at": "2026-01-01T00:00:00Z", "max_input_tokens": 100}
            ],
            "has_more": true,
            "first_id": "model-a",
            "last_id": "model-a"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Second page: uses after_id=model-a, closes with has_more=false.
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(query_param("limit", "1000"))
        .and(query_param("after_id", "model-a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {"id": "model-b", "display_name": "B", "type": "model",
                 "created_at": "2026-01-02T00:00:00Z", "max_input_tokens": 200}
            ],
            "has_more": false,
            "first_id": "model-b",
            "last_id": "model-b"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-test".to_string()).unwrap();
    let models = adapter.list_models().await.unwrap();

    let ids: Vec<_> = models.iter().map(|m| m.remote_id.as_str()).collect();
    assert_eq!(ids, ["model-a", "model-b"]);
}

#[tokio::test]
/// A paginated response without a cursor must not produce an incomplete
/// model cache.
async fn list_models_rejects_has_more_without_last_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "id": "model-a",
                "display_name": "Model A",
                "max_input_tokens": 4096
            }],
            "has_more": true
        })))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "test-key".to_string()).unwrap();
    let error = adapter
        .list_models()
        .await
        .expect_err("missing cursor must fail");
    assert!(matches!(error, AdapterError::Parse { reason } if reason.contains("without last_id")));
}

#[tokio::test]
async fn list_models_maps_401_to_invalid_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "type": "error",
            "error": {"type": "authentication_error", "message": "invalid x-api-key"}
        })))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-wrong".to_string()).unwrap();
    let err = adapter.list_models().await.expect_err("401 must error");
    assert!(
        matches!(err, AdapterError::InvalidCredentials),
        "got {err:?}"
    );
}

#[tokio::test]
async fn list_models_maps_403_to_invalid_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-forbidden".to_string()).unwrap();
    let err = adapter.list_models().await.expect_err("403 must error");
    assert!(
        matches!(err, AdapterError::InvalidCredentials),
        "got {err:?}"
    );
}

#[tokio::test]
async fn list_models_maps_5xx_to_status_error_preserving_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream unavailable"))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-any".to_string()).unwrap();
    let err = adapter.list_models().await.expect_err("5xx must error");
    match err {
        AdapterError::Status { status, body } => {
            assert_eq!(status, 503);
            assert!(body.contains("upstream unavailable"));
        }
        other => panic!("expected Status, got {other:?}"),
    }
}

#[tokio::test]
async fn list_models_treats_zero_max_input_tokens_as_absent() {
    // The docs example ships `max_input_tokens: 0` on newer models —
    // treat that as "not reported" so the picker does not show a
    // useless zero-width context.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {"id": "m", "display_name": "M", "type": "model",
                 "created_at": "2026-01-01T00:00:00Z", "max_input_tokens": 0}
            ],
            "has_more": false,
            "first_id": "m",
            "last_id": "m"
        })))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-any".to_string()).unwrap();
    let models = adapter.list_models().await.unwrap();
    assert_eq!(models.len(), 1);
    assert!(models[0].context_window.is_none());
}

// ---------- stream_chat ----------

fn sample_request(model: &str) -> ChatRequest {
    ChatRequest {
        model_id: model.to_string(),
        system_prompt: None,
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: "hi".to_string(),
        }],
        max_new_tokens: Some(128),
        tools: Vec::new(),
    }
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
async fn stream_chat_emits_deltas_and_done_with_token_counts() {
    let server = MockServer::start().await;
    let body = sse_body(&[
        (
            "message_start",
            serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": "msg_1",
                    "usage": {"input_tokens": 12, "output_tokens": 0}
                }
            }),
        ),
        (
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {"type": "text_delta", "text": "Hello"}
            }),
        ),
        (
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {"type": "text_delta", "text": " world"}
            }),
        ),
        (
            "message_delta",
            serde_json::json!({
                "type": "message_delta",
                "delta": {"stop_reason": "end_turn"},
                "usage": {"output_tokens": 3}
            }),
        ),
        ("message_stop", serde_json::json!({"type": "message_stop"})),
    ]);
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "sk-test"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body)
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-test".to_string()).unwrap();
    let mut stream = adapter
        .stream_chat(sample_request("claude-opus-5"))
        .await
        .expect("stream_chat starts");

    let mut deltas: Vec<String> = Vec::new();
    let mut finished: Option<(Option<usize>, Option<usize>, Option<String>)> = None;
    while let Some(chunk) = stream.next().await {
        match chunk.expect("no error frame") {
            StreamChunk::Delta { content, reasoning } => {
                assert!(reasoning.is_none(), "no thinking in this fixture");
                if !content.is_empty() {
                    deltas.push(content);
                }
            }
            StreamChunk::Done {
                prompt_tokens,
                completion_tokens,
                finish_reason,
                ..
            } => {
                finished = Some((prompt_tokens, completion_tokens, finish_reason));
                break;
            }
            StreamChunk::ToolCalls(_) => panic!("this fixture emits no tool_use blocks"),
        }
    }

    assert_eq!(deltas.join(""), "Hello world");
    let (pt, ct, fr) = finished.expect("Done frame arrived");
    assert_eq!(pt, Some(12));
    assert_eq!(ct, Some(3));
    assert_eq!(fr.as_deref(), Some("end_turn"));
}

#[tokio::test]
/// A stream that ends after a delta without `message_stop` is truncated.
async fn stream_chat_reports_unexpected_end_after_delta() {
    let server = MockServer::start().await;
    let body = sse_body(&[(
        "content_block_delta",
        serde_json::json!({
            "type": "content_block_delta",
            "delta": {"type": "text_delta", "text": "partial"}
        }),
    )]);
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body)
                .insert_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-any".to_string()).unwrap();
    let mut stream = adapter
        .stream_chat(sample_request("claude-opus-5"))
        .await
        .unwrap();
    let mut saw_unexpected_end = false;
    while let Some(chunk) = stream.next().await {
        if matches!(chunk, Err(StreamError::UnexpectedEnd)) {
            saw_unexpected_end = true;
            break;
        }
    }
    assert!(saw_unexpected_end, "truncated stream must not complete");
}

#[tokio::test]
async fn stream_chat_surfaces_thinking_delta_as_reasoning() {
    let server = MockServer::start().await;
    let body = sse_body(&[
        (
            "message_start",
            serde_json::json!({
                "type": "message_start",
                "message": {"id": "m", "usage": {"input_tokens": 4, "output_tokens": 0}}
            }),
        ),
        (
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {"type": "thinking_delta", "thinking": "let me think"}
            }),
        ),
        (
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {"type": "text_delta", "text": "42"}
            }),
        ),
        ("message_stop", serde_json::json!({"type": "message_stop"})),
    ]);
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body)
                .insert_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-any".to_string()).unwrap();
    let mut stream = adapter
        .stream_chat(sample_request("claude-opus-5"))
        .await
        .unwrap();

    let mut reasoning: Vec<String> = Vec::new();
    let mut content: Vec<String> = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk.unwrap() {
            StreamChunk::Delta {
                content: c,
                reasoning: r,
            } => {
                if let Some(r) = r {
                    reasoning.push(r);
                }
                if !c.is_empty() {
                    content.push(c);
                }
            }
            StreamChunk::Done { .. } => break,
            StreamChunk::ToolCalls(_) => panic!("this fixture emits no tool_use blocks"),
        }
    }
    assert_eq!(reasoning, vec!["let me think"]);
    assert_eq!(content.join(""), "42");
}

#[tokio::test]
async fn stream_chat_maps_401_to_invalid_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "type": "error",
            "error": {"type": "authentication_error", "message": "bad key"}
        })))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-bad".to_string()).unwrap();
    let result = adapter.stream_chat(sample_request("claude-opus-5")).await;
    match result {
        Err(AdapterError::InvalidCredentials) => {}
        Err(other) => panic!("expected InvalidCredentials, got {other:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[tokio::test]
async fn stream_chat_delivers_sse_error_event_as_stream_error() {
    let server = MockServer::start().await;
    let body = sse_body(&[
        (
            "message_start",
            serde_json::json!({
                "type": "message_start",
                "message": {"id": "m", "usage": {"input_tokens": 1, "output_tokens": 0}}
            }),
        ),
        (
            "error",
            serde_json::json!({
                "type": "error",
                "error": {"type": "overloaded_error", "message": "overloaded"}
            }),
        ),
    ]);
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body)
                .insert_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-any".to_string()).unwrap();
    let mut stream = adapter
        .stream_chat(sample_request("claude-opus-5"))
        .await
        .unwrap();
    let mut saw_error: Option<StreamError> = None;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(_) => continue,
            Err(e) => {
                saw_error = Some(e);
                break;
            }
        }
    }
    match saw_error {
        Some(StreamError::Model(msg)) => assert!(msg.contains("overloaded"), "{msg}"),
        other => panic!("expected Model error, got {other:?}"),
    }
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
        max_new_tokens: Some(128),
        tools: Vec::new(),
    };

    let mut stream = adapter.stream_chat(request).await.unwrap();
    while stream.next().await.is_some() {}
}

// ---------- helpers ----------

fn query_param_missing(name: &'static str) -> impl wiremock::Match {
    // wiremock has no built-in "must not be present" matcher, so build
    // one from the incoming request: the whole query string must lack
    // the exact `name=` prefix. Good enough for the two-page test —
    // real requests only include the params the adapter sets.
    struct Missing(&'static str);
    impl wiremock::Match for Missing {
        fn matches(&self, req: &wiremock::Request) -> bool {
            let query = req.url.query().unwrap_or("");
            let needle = format!("{}=", self.0);
            !query.split('&').any(|part| part.starts_with(&needle))
        }
    }
    Missing(name)
}
