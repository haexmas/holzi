//! Unit tests for the preference-scope wire conversion + key
//! validation. DB-behaviour of get/set/clear is covered by the
//! integration suite (`tests/preferences_roundtrip.rs`); these tests
//! ensure the wire-shape rejections keep the sync semantics safe.

use uuid::Uuid;

use super::preferences_commands::PrefScopeWire;
use crate::identity::VAULT_SCOPE_UUID;
use crate::storage::preferences::PrefScope;

#[test]
fn vault_wire_maps_to_vault_scope() {
    let scope = PrefScope::try_from(PrefScopeWire::Vault).expect("vault scope");
    assert_eq!(scope, PrefScope::Vault);
}

#[test]
fn device_wire_maps_to_device_scope() {
    let uuid = Uuid::new_v4();
    let scope = PrefScope::try_from(PrefScopeWire::Device { uuid }).expect("device scope");
    assert_eq!(scope, PrefScope::Device(uuid));
}

#[test]
fn device_wire_rejects_the_nil_uuid() {
    let err = PrefScope::try_from(PrefScopeWire::Device {
        uuid: VAULT_SCOPE_UUID,
    })
    .expect_err("nil UUID must be rejected in device scope");
    assert!(format!("{err:?}").contains("nil UUID"));
}
