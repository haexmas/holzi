//! Tests for `DelegateVendor` parsing and `build_adapter`'s `CliDelegate`
//! construction path (tasks.md T007).

use uuid::Uuid;

use super::DelegateVendor;
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
async fn delegate_exposes_claude_model_aliases() {
    let provider = sample_provider(Some("claude"));
    let adapter = build_adapter(&provider, None).expect("delegate adapter should build");
    let models = adapter
        .list_models()
        .await
        .expect("model listing should work");
    let remote_ids: Vec<&str> = models.iter().map(|m| m.remote_id.as_str()).collect();
    assert_eq!(remote_ids, vec!["sonnet", "opus", "haiku", "fable"]);
}
