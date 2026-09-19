//! Tests for `DelegateVendor` parsing and `build_adapter`'s `CliDelegate`
//! construction path (tasks.md T007).

use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{build_transcript_prompt, DelegateVendor};
use crate::adapters::types::{Attachment, AttachmentKind, ChatMessage, ChatRequest, ChatRole};
use crate::providers::build_adapter;
use crate::storage::providers::{Provider, ProviderCapability, ProviderKind};

#[test]
fn delegate_vendor_round_trips() {
    for vendor in [DelegateVendor::Claude, DelegateVendor::Codex] {
        let parsed = DelegateVendor::parse(vendor.as_str());
        assert_eq!(parsed, Some(vendor));
    }
}

#[test]
fn delegate_vendor_rejects_unknown_strings() {
    assert_eq!(DelegateVendor::parse("anthropic"), None);
    assert_eq!(DelegateVendor::parse(""), None);
    assert_eq!(DelegateVendor::parse("Claude"), None);
}

#[test]
fn transcript_gives_duplicate_attachment_names_unique_sandbox_names() {
    let request = ChatRequest {
        model_id: "claude-opus-5".to_string(),
        thread_id: None,
        system_prompt: None,
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: "read both files".to_string(),
            attachments: vec![
                Attachment {
                    name: "report.txt".to_string(),
                    kind: AttachmentKind::Text,
                    media_type: "text/plain".to_string(),
                    bytes: b"first".to_vec(),
                },
                Attachment {
                    name: "report.txt".to_string(),
                    kind: AttachmentKind::Text,
                    media_type: "text/plain".to_string(),
                    bytes: b"second".to_vec(),
                },
            ],
        }],
        reasoning_requested: false,
        max_new_tokens: None,
        tools: Vec::new(),
        autonomy_mode: Default::default(),
        effort_level: Default::default(),
    };

    let prompt = build_transcript_prompt(&request);
    assert!(prompt.contains("attachment-0-report.txt"));
    assert!(prompt.contains("attachment-1-report.txt"));
}

fn sample_provider(adapter: Option<&str>) -> Provider {
    Provider {
        id: Uuid::new_v4(),
        kind: ProviderKind::CliDelegate,
        adapter: adapter.map(str::to_string),
        name: "Claude Code".to_string(),
        base_url: Some("claude".to_string()),
        credentials: Some(b"fake-oauth-token".to_vec()),
        created_at: 0,
        capability: ProviderCapability::Chat,
    }
}

#[test]
fn build_adapter_constructs_cli_delegate_for_well_formed_row() {
    let provider = sample_provider(Some("claude"));
    let adapter = build_adapter(&provider, None);
    assert!(adapter.is_ok(), "expected Ok, got an error instead");
}

#[test]
fn build_adapter_errors_on_malformed_adapter_value() {
    let provider = sample_provider(Some("not-a-real-vendor"));
    let err = build_adapter(&provider, None)
        .err()
        .expect("malformed adapter must be rejected");
    let message = format!("{err}");
    assert!(
        message.contains("invalid or missing adapter"),
        "unexpected error message: {message}"
    );
}

#[test]
fn build_adapter_errors_on_missing_adapter_value() {
    let provider = sample_provider(None);
    assert!(build_adapter(&provider, None).is_err());
}

#[test]
fn build_adapter_errors_on_missing_credentials() {
    let mut provider = sample_provider(Some("codex"));
    provider.credentials = None;
    let err = build_adapter(&provider, None)
        .err()
        .expect("missing credentials must be rejected");
    let message = format!("{err}");
    assert!(
        message.contains("missing credentials"),
        "unexpected error message: {message}"
    );
}

#[tokio::test]
async fn delegate_exposes_one_synthetic_vendor_model() {
    let provider = sample_provider(Some("codex"));
    let adapter = build_adapter(&provider, None).expect("delegate adapter should build");
    let models = adapter
        .list_models()
        .await
        .expect("model listing should work");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].remote_id, "codex");
    assert_eq!(models[0].display_name, "codex (CLI delegate)");
}

#[tokio::test]
async fn claude_model_list_comes_from_the_models_api_via_oauth_bearer() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header("authorization", "Bearer test-oauth-token"))
        .and(header("anthropic-beta", "oauth-2025-04-20"))
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
                    "id": "claude-sonnet-5",
                    "display_name": "Claude Sonnet 5",
                    "type": "model",
                    "created_at": "2026-07-24T00:00:00Z",
                    "max_input_tokens": 200000,
                }
            ],
            "has_more": false,
            "first_id": "claude-opus-5",
            "last_id": "claude-sonnet-5"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Not routed through `CliDelegateAdapter`/`build_adapter`: the provider
    // row's `base_url` holds the `claude` binary path, not an API endpoint,
    // so `fetch_claude_models` takes the base URL directly — that's what
    // makes it testable against a wiremock server in the first place.
    let models = super::fetch_claude_models(&server.uri(), "test-oauth-token")
        .await
        .expect("fetch_claude_models should succeed");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0].remote_id, "claude-opus-5");
    assert_eq!(models[0].display_name, "Claude Opus 5");
    assert_eq!(models[1].remote_id, "claude-sonnet-5");
}

#[tokio::test]
async fn claude_model_list_rejects_empty_credentials() {
    let mut provider = sample_provider(Some("claude"));
    provider.credentials = Some(b"   ".to_vec());
    let adapter = build_adapter(&provider, None).expect("delegate adapter should build");
    let err = adapter
        .list_models()
        .await
        .expect_err("blank credentials must be rejected before any network call");
    assert!(matches!(
        err,
        crate::adapters::AdapterError::InvalidCredentials
    ));
}
