use uuid::Uuid;

use super::*;
use crate::storage::query;
use crate::sync::test_support::open_vault;

#[test]
fn two_copies_of_a_seed_derive_the_same_valid_identity() {
    let seed = [42u8; 32];
    let first = derive_vault_identity(&seed);
    let second = derive_vault_identity(&seed);
    assert_eq!(first.as_slice(), second.as_slice());
    assert!(SecretKey::from_byte_array(&first).is_ok());
    assert_ne!(
        derive_vault_identity(&[43u8; 32]).as_slice(),
        first.as_slice()
    );
}

#[test]
fn the_derivation_is_hkdf_sha256_with_counter_zero_first() {
    let seed = [1u8; 32];
    let hkdf = Hkdf::<Sha256>::new(Some(b"holzi"), &seed);
    let mut expected = [0u8; 32];
    hkdf.expand(b"holzi/vault-identity/v1\0\0\0\0", &mut expected)
        .expect("expand");
    assert_eq!(derive_vault_identity(&seed).as_slice(), expected);
}

#[test]
fn device_keys_are_created_once_per_installation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());
    let own = Uuid::new_v4();
    let source = Uuid::new_v4();

    let source_keys = db
        .write(|tx| ensure_device_keys(tx, source, 1))
        .expect("source keys");
    let first = db
        .write(|tx| ensure_device_keys(tx, own, 2))
        .expect("own keys");
    let again = db
        .write(|tx| ensure_device_keys(tx, own, 3))
        .expect("own keys again");

    assert_eq!(first.device_pubkey, again.device_pubkey);
    assert_eq!(first.endpoint_id, again.endpoint_id);
    assert_ne!(first.device_pubkey, source_keys.device_pubkey);
    let stored_source = query::read(&db, |r| load_device_keys(r, source))
        .expect("read")
        .expect("the source row stays");
    assert_eq!(stored_source.device_pubkey, source_keys.device_pubkey);
    assert_eq!(
        stored_source.device_secret.as_slice(),
        source_keys.device_secret.as_slice()
    );
}

#[test]
fn the_endpoint_id_is_the_iroh_public_key_of_the_endpoint_secret() {
    let keys = DeviceKeys::generate();
    let expected = *iroh::SecretKey::from_bytes(&keys.endpoint_secret)
        .public()
        .as_bytes();
    assert_eq!(keys.endpoint_id, expected);
    assert_eq!(
        keys.device_pubkey,
        xonly_public_key(&keys.device_secret).expect("public key")
    );
}

#[test]
fn debug_output_shows_no_secret() {
    let keys = DeviceKeys::generate();
    let debug = format!("{keys:?}");
    assert!(!debug.contains(&hex(keys.device_secret.as_slice())));
    assert!(!debug.contains(&hex(keys.endpoint_secret.as_slice())));
    assert!(debug.contains(&hex(&keys.device_pubkey)));
}

#[test]
fn a_new_vault_publishes_an_identity_only_with_genesis() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());

    assert_eq!(
        db.write(|tx| ensure_vault_identity(tx, false))
            .expect("no genesis"),
        None
    );
    let pubkey = db
        .write(|tx| ensure_vault_identity(tx, true))
        .expect("genesis")
        .expect("published");
    let secret = query::read(&db, |r| vault_secret(r))
        .expect("read")
        .expect("a new vault's first device is a main device");
    assert_eq!(xonly_public_key(&secret).expect("public key"), pubkey);
    assert_eq!(
        db.write(|tx| ensure_vault_identity(tx, true))
            .expect("again"),
        Some(pubkey),
        "publishing is idempotent"
    );
}

#[test]
fn a_placeholder_seed_becomes_the_derived_identity() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());
    let seed = [9u8; 32];
    db.write(|tx| {
        tx.execute(
            "INSERT INTO vault_identity_secret_no_sync (id, privkey) VALUES (1, ?1)",
            params![seed.as_slice()],
        )
    })
    .expect("seed");

    let pubkey = db
        .write(|tx| ensure_vault_identity(tx, false))
        .expect("derive")
        .expect("published");

    let derived = derive_vault_identity(&seed);
    assert_eq!(pubkey, xonly_public_key(&derived).expect("public key"));
    let stored = query::read(&db, |r| vault_secret(r))
        .expect("read")
        .expect("secret");
    assert_eq!(
        stored.as_slice(),
        derived.as_slice(),
        "the seed is replaced"
    );
}
