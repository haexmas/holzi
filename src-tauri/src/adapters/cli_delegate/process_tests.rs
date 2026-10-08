//! Tests for `map_spawn_error`'s not-installed distinction (tasks.md T019).

use super::process::{ensure_available, map_spawn_error};
use super::DelegateVendor;
use crate::adapters::AdapterError;

#[test]
fn missing_binary_maps_to_unavailable() {
    let error = std::io::Error::from(std::io::ErrorKind::NotFound);
    match map_spawn_error("claude", DelegateVendor::Claude, error) {
        AdapterError::Unavailable { reason } => {
            assert!(reason.contains("claude"), "reason: {reason}");
            assert!(reason.contains("not installed"), "reason: {reason}");
            assert!(reason.contains("npm install"), "reason: {reason}");
        }
        other => panic!("expected Unavailable, got {other:?}"),
    }
}

#[test]
fn missing_binary_hint_matches_vendor() {
    let error = std::io::Error::from(std::io::ErrorKind::NotFound);
    match map_spawn_error("codex", DelegateVendor::Codex, error) {
        AdapterError::Unavailable { reason } => {
            assert!(reason.contains("@openai/codex"), "reason: {reason}");
        }
        other => panic!("expected Unavailable, got {other:?}"),
    }
}

#[test]
fn other_spawn_failures_stay_generic() {
    let error = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    match map_spawn_error("codex", DelegateVendor::Codex, error) {
        AdapterError::Http { reason } => {
            assert!(reason.contains("codex"), "reason: {reason}");
        }
        other => panic!("expected Http, got {other:?}"),
    }
}

#[test]
fn without_delegates_on_the_device_a_start_is_refused_for_the_platform() {
    assert!(ensure_available(true).is_ok());
    match ensure_available(false) {
        Err(AdapterError::Unavailable { reason }) => assert!(reason.starts_with("platform")),
        other => panic!("{other:?}"),
    }
}
