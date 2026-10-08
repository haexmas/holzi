use std::sync::Arc;

use uuid::Uuid;

use super::*;
use crate::passwords::test_support::open_test_vault;
use crate::storage::known_devices;
use crate::vault_gate::VaultGate;

fn vault() -> (tempfile::TempDir, VaultDb, Uuid) {
    let (dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db)).unwrap();
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .unwrap()[0]
        .vault_device_uuid;
    (dir, vault, device)
}

fn read(vault: &VaultDb, device: Uuid) -> bool {
    vault
        .read_blocking(move |q| protection(q, device).map_err(Into::into))
        .unwrap()
}

fn set(vault: &VaultDb, device: Uuid, enabled: bool) {
    vault
        .write_blocking(move |tx| set_protection(tx, device, enabled).map_err(Into::into))
        .unwrap();
}

#[test]
fn without_a_choice_the_protection_is_on() {
    let (_dir, vault, device) = vault();
    assert!(read(&vault, device));
}

#[test]
fn the_choice_is_kept_and_counts_for_this_device_only() {
    let (_dir, vault, device) = vault();
    set(&vault, device, false);
    assert!(!read(&vault, device));
    assert!(
        read(&vault, Uuid::new_v4()),
        "another device keeps the default"
    );
    set(&vault, device, true);
    assert!(read(&vault, device));
}
