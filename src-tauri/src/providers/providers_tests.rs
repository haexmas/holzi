//! Tests for the provider-command boundary's adapter validation and
//! legacy-row repair.

use super::{legacy_adapter_for_url, validate_adapter};
use crate::error::HolziError;

#[test]
/// Only adapters with an implemented protocol may be persisted.
fn validate_adapter_rejects_missing_and_unknown_values() {
    assert!(validate_adapter(Some("anthropic")).is_ok());
    assert!(matches!(
        validate_adapter(None),
        Err(HolziError::InvalidInput { reason }) if reason.contains("requires adapter")
    ));
    assert!(matches!(
        validate_adapter(Some("openai")),
        Err(HolziError::InvalidInput { reason }) if reason.contains("unsupported")
    ));
}

#[test]
/// Legacy rows are repaired only when their endpoint identifies Anthropic.
fn legacy_adapter_repair_does_not_guess_from_kind() {
    assert_eq!(
        legacy_adapter_for_url(Some("https://api.anthropic.com/")),
        Some("anthropic")
    );
    assert_eq!(legacy_adapter_for_url(Some("https://api.openai.com")), None);
    assert_eq!(legacy_adapter_for_url(None), None);
}
