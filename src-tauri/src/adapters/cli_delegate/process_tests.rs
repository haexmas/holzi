//! Tests for `map_spawn_error`'s not-installed distinction (tasks.md T019).

use super::process::map_spawn_error;
use crate::adapters::AdapterError;

#[test]
fn missing_binary_maps_to_unavailable() {
    let error = std::io::Error::from(std::io::ErrorKind::NotFound);
    match map_spawn_error("claude", error) {
        AdapterError::Unavailable { reason } => {
            assert!(reason.contains("claude"), "reason: {reason}");
            assert!(reason.contains("not installed"), "reason: {reason}");
        }
        other => panic!("expected Unavailable, got {other:?}"),
    }
}

#[test]
fn other_spawn_failures_stay_generic() {
    let error = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    match map_spawn_error("codex", error) {
        AdapterError::Http { reason } => {
            assert!(reason.contains("codex"), "reason: {reason}");
        }
        other => panic!("expected Http, got {other:?}"),
    }
}
