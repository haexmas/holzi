//! Unit tests for the preference-scope + key-validation surface. Pure
//! functions, no I/O — the DB-behaviour (roundtrip, scope-isolation,
//! NULL-as-absent, sentinel-idempotency, FK-cascade) lives in the
//! integration suite at `tests/preferences_roundtrip.rs` because it
//! needs the `current_hlc()` SQLite function that only exists on an
//! open `haex_crdt::Database`.

use uuid::Uuid;

use super::preferences::{
    parse_bool, validate_key, validate_scope, PrefError, PrefScope, ScopedBool,
};
use crate::identity::VAULT_SCOPE_UUID;

#[test]
fn vault_scope_resolves_to_sentinel_uuid() {
    assert_eq!(PrefScope::Vault.to_uuid(), VAULT_SCOPE_UUID);
}

#[test]
fn device_scope_resolves_to_the_wrapped_uuid() {
    let id = Uuid::new_v4();
    assert_eq!(PrefScope::Device(id).to_uuid(), id);
}

#[test]
fn key_must_be_dotted_namespaced() {
    assert!(validate_key("chat.default_model_id").is_ok());
    assert!(matches!(validate_key(""), Err(PrefError::EmptyKey)));
    assert!(matches!(
        validate_key("no_namespace"),
        Err(PrefError::UnnamespacedKey(_))
    ));
}

#[test]
fn device_scope_refuses_the_nil_uuid() {
    assert!(validate_scope(PrefScope::Vault).is_ok());
    assert!(validate_scope(PrefScope::Device(Uuid::new_v4())).is_ok());
    assert!(matches!(
        validate_scope(PrefScope::Device(VAULT_SCOPE_UUID)),
        Err(PrefError::NilDeviceUuid)
    ));
}

// Boolean preferences with device-over-vault resolution (spec 022-session-restore,
// research R3). The DB read in `get_scoped_bool` is covered through the
// `wm_session_commands` tests, which need an open vault.

#[test]
fn only_true_and_false_parse_as_booleans() {
    assert_eq!(parse_bool(Some("true")), Some(true));
    assert_eq!(parse_bool(Some("false")), Some(false));
    assert_eq!(parse_bool(Some("TRUE")), None);
    assert_eq!(parse_bool(Some("1")), None);
    assert_eq!(parse_bool(Some("")), None);
    assert_eq!(parse_bool(None), None);
}

#[test]
fn the_device_value_wins_over_the_vault_value() {
    let scoped = ScopedBool {
        device: Some(true),
        vault: Some(false),
    };
    assert!(scoped.effective());
    let scoped = ScopedBool {
        device: Some(false),
        vault: Some(true),
    };
    assert!(!scoped.effective());
}

#[test]
fn an_unset_device_value_falls_back_to_the_vault_value_then_to_false() {
    assert!(ScopedBool {
        device: None,
        vault: Some(true),
    }
    .effective());
    assert!(!ScopedBool {
        device: None,
        vault: Some(false),
    }
    .effective());
    assert!(!ScopedBool {
        device: None,
        vault: None,
    }
    .effective());
}
