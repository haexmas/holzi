//! Tests for `paths` — name-validation regex and pending-marker path
//! construction. No `AppHandle` needed.

use std::path::PathBuf;

use crate::instances::paths::{get_pending_marker_path, validate_instance_name};

#[test]
fn accepts_valid_names() {
    for name in ["a", "test", "Test-01", "vault_2026", "AAAAAAAAAA"] {
        assert!(
            validate_instance_name(name).is_ok(),
            "should accept `{name}`"
        );
    }
}

#[test]
fn rejects_invalid_names() {
    // Empty, leading non-alnum, invalid chars, too long.
    for name in [
        "",
        "-leading-dash",
        "_leading_underscore",
        "has space",
        "has/slash",
        "has.dot",
    ] {
        assert!(
            validate_instance_name(name).is_err(),
            "should reject `{name}`"
        );
    }
    let too_long = "a".repeat(65);
    assert!(validate_instance_name(&too_long).is_err());
}

#[test]
fn pending_marker_replaces_final_extension() {
    let db = PathBuf::from("/tmp/foo.db");
    let marker = get_pending_marker_path(&db);
    assert_eq!(marker, PathBuf::from("/tmp/foo.db.pending"));
}
