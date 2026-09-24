//! Tests for `VaultDb`, the database handle that carries a tracker token (data-model.md).

use std::sync::Arc;

use haex_crdt::Database;

use crate::identity::installation_id_path;
use crate::instances::vault_config::vault_config;
use crate::vault_gate::VaultGate;

fn open_throwaway() -> (tempfile::TempDir, Arc<Database>) {
    let tmp = tempfile::tempdir().expect("temp dir");
    let db = Database::open(vault_config(
        "vault-db-test-passphrase",
        &tmp.path().join("vault.db"),
        &installation_id_path(tmp.path()),
        true,
    ))
    .expect("open a throwaway database");
    (tmp, Arc::new(db))
}

#[test]
fn a_clone_keeps_the_tracker_busy_until_the_last_clone_drops() {
    let (_tmp, db) = open_throwaway();
    let gate = VaultGate::new();
    assert!(gate.is_idle(), "nothing uses the database yet");

    let first = gate.vault_db(db).expect("open gate");
    let second = first.clone();
    assert!(!gate.is_idle());

    drop(first);
    assert!(!gate.is_idle(), "one clone is still alive");
    drop(second);
    assert!(gate.is_idle(), "the tracker empties after the last drop");
}

#[test]
fn deref_reaches_the_database() {
    let (_tmp, db) = open_throwaway();
    let expected = db.device_id();
    let gate = VaultGate::new();
    let vault_db = gate.vault_db(db).expect("open gate");

    assert_eq!(vault_db.device_id(), expected);
    let database: &Database = &vault_db;
    assert_eq!(database.device_id(), expected);
}

#[test]
fn a_closing_gate_rejects_new_database_handles() {
    let (_tmp, db) = open_throwaway();
    let gate = VaultGate::new();
    gate.request_close();

    assert!(matches!(
        gate.vault_db(db),
        Err(crate::error::HolziError::VaultClosed)
    ));
}
