// The tests add a second device row directly.
#![allow(clippy::disallowed_methods)]

use std::path::Path;
use std::sync::Arc;

use uuid::Uuid;

use super::*;
use crate::extensions::bridge::dispatch::{call, Emit};
use crate::extensions::host::ExtensionHost;
use crate::extensions::registry::effective::effective_bundle;
use crate::extensions::registry::install::install;
use crate::passwords::test_support::open_test_vault;
use crate::storage::known_devices;
use crate::vault_gate::{VaultDb, VaultGate};

struct Silent;

impl Emit for Silent {
    fn emit(&self, _event: &str, _payload: Value) {}
}

fn install_fixture(vault: &VaultDb, name: &str, device: Uuid) -> (Uuid, Uuid) {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension_bundles")
            .join(name),
    )
    .unwrap();
    let extension = install(vault, &bytes, vec![], false, device, 1)
        .unwrap()
        .ids
        .extension_id;
    let bundle = vault
        .read_blocking(move |q| effective_bundle(q, extension).map_err(Into::into))
        .unwrap()
        .unwrap()
        .bundle_id;
    (extension, bundle)
}

/// Two extensions on this device and the first one on a second device of the vault.
struct Setup {
    _dir: tempfile::TempDir,
    notes: CallContext,
    minimal: CallContext,
    notes_elsewhere: CallContext,
}

fn setup() -> Setup {
    let (dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db)).unwrap();
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .unwrap()[0]
        .vault_device_uuid;
    let other_device = Uuid::new_v4();
    vault
        .write_blocking(move |tx| {
            tx.execute(
                "INSERT INTO known_devices (installation_uuid, vault_device_uuid, alias, first_seen) \
                 VALUES (?1, ?2, 'phone', 1)",
                params![Uuid::new_v4().to_string(), other_device.to_string()],
            )
            .map(drop)
        })
        .unwrap();
    let (notes, notes_bundle) = install_fixture(&vault, "good-notes-like.xt", device);
    let (minimal, minimal_bundle) = install_fixture(&vault, "good-minimal.xt", device);
    let host = Arc::new(ExtensionHost::default());
    let context = |extension, bundle, device| CallContext {
        db: vault.clone(),
        host: Arc::clone(&host),
        session: host.frames.open(extension, bundle, "tab"),
        device,
        emitter: Arc::new(Silent),
    };
    Setup {
        notes: context(notes, notes_bundle, device),
        minimal: context(minimal, minimal_bundle, device),
        notes_elsewhere: context(notes, notes_bundle, other_device),
        _dir: dir,
    }
}

fn get(ctx: &CallContext, key: &str) -> Value {
    call(
        ctx,
        "extension_web_storage_get_item",
        &json!({ "key": key }),
    )
    .unwrap()
}

fn set(ctx: &CallContext, key: &str, value: &str) -> Result<Value, BridgeError> {
    call(
        ctx,
        "extension_web_storage_set_item",
        &json!({ "key": key, "value": value }),
    )
}

fn code(outcome: Result<Value, BridgeError>) -> u16 {
    outcome.unwrap_err().code.as_u16()
}

#[test]
fn a_value_belongs_to_its_extension_and_device_only() {
    let s = setup();
    set(&s.notes, "theme", "dark").unwrap();
    set(&s.notes, "zoom", "2").unwrap();
    set(&s.notes, "theme", "light").unwrap();

    assert_eq!(get(&s.notes, "theme"), json!("light"));
    assert_eq!(get(&s.minimal, "theme"), Value::Null, "another extension");
    assert_eq!(
        get(&s.notes_elsewhere, "theme"),
        Value::Null,
        "another device"
    );
    assert_eq!(
        call(&s.notes, "extension_web_storage_keys", &Value::Null).unwrap(),
        json!(["theme", "zoom"])
    );

    set(&s.minimal, "theme", "own").unwrap();
    call(
        &s.notes,
        "extension_web_storage_remove_item",
        &json!({ "key": "zoom" }),
    )
    .unwrap();
    call(&s.notes, "extension_web_storage_clear", &Value::Null).unwrap();
    assert_eq!(
        call(&s.notes, "extension_web_storage_keys", &Value::Null).unwrap(),
        json!([])
    );
    assert_eq!(
        get(&s.minimal, "theme"),
        json!("own"),
        "clear stays in its store"
    );
}

#[test]
fn keys_values_and_the_whole_store_have_limits() {
    let s = setup();
    assert_eq!(
        code(set(&s.notes, &"k".repeat(MAX_KEY_BYTES + 1), "v")),
        7000
    );
    assert_eq!(
        code(set(&s.notes, "big", &"v".repeat(MAX_VALUE_BYTES + 1))),
        7000
    );

    let value = "v".repeat(MAX_VALUE_BYTES - 16);
    for i in 0..10 {
        set(&s.notes, &format!("k{i}"), &value).unwrap();
    }
    assert_eq!(
        code(set(&s.notes, "k10", &value)),
        7000,
        "the store is full"
    );
    set(&s.notes, "k0", "small").unwrap();
    set(&s.notes, "k10", &value).unwrap();
    assert_eq!(
        get(&s.notes, "k10").as_str().map(str::len),
        Some(value.len())
    );
    set(&s.minimal, "k0", &value).unwrap();
}

#[test]
fn a_key_and_a_value_must_be_text() {
    let s = setup();
    assert_eq!(
        code(call(
            &s.notes,
            "extension_web_storage_set_item",
            &json!({ "key": "n", "value": 1 })
        )),
        3001
    );
    assert_eq!(
        code(call(
            &s.notes,
            "extension_web_storage_get_item",
            &Value::Null
        )),
        3001
    );
}
