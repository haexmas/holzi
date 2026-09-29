use uuid::Uuid;

use super::*;
use crate::sync::keys::random_secret_key;
use crate::sync::signing::xonly_public_key;

struct Vault {
    secret: zeroize::Zeroizing<[u8; 32]>,
    pubkey: [u8; 32],
}

fn vault() -> Vault {
    let secret = random_secret_key();
    let pubkey = xonly_public_key(&secret).expect("public key");
    Vault { secret, pubkey }
}

fn device(tag: u8, role: Role) -> ListedDevice {
    ListedDevice {
        device_pubkey: xonly_public_key(&[tag; 32]).expect("device key"),
        endpoint_id: [tag; 32],
        role,
        vault_device_uuid: Uuid::from_bytes([tag; 16]),
        name_sealed: vec![tag],
        added_at: u64::from(tag),
    }
}

fn removal(device: &ListedDevice) -> RemovedDevice {
    RemovedDevice {
        device_pubkey: device.device_pubkey,
        vault_device_uuid: device.vault_device_uuid,
        limit_hlc: "1/0".to_string(),
        removed_at: 9,
    }
}

fn list(
    vault: &Vault,
    generation: u64,
    base: Option<[u8; 32]>,
    devices: Vec<ListedDevice>,
) -> DeviceList {
    DeviceList {
        vault: vault.pubkey,
        generation,
        devices,
        removed: Vec::new(),
        issued_by: [0; 32],
        issued_at: generation,
        base_list_hash: base,
    }
}

fn stored(signed: &SignedList) -> StoredList {
    StoredList {
        hash: signed.hash.to_vec(),
        generation: signed.list.generation as i64,
        payload: signed.payload.clone(),
        signature: signed.signature.to_vec(),
    }
}

#[test]
fn a_signed_first_list_is_valid_and_effective() {
    let v = vault();
    let signed =
        sign_list(list(&v, 1, None, vec![device(1, Role::Main)]), &v.secret).expect("sign");
    let valid = valid_lists(&[stored(&signed)], &v.pubkey);
    assert_eq!(effective(&valid).map(|s| s.hash), Some(signed.hash));
}

#[test]
fn the_highest_generation_wins() {
    let v = vault();
    let first = sign_list(list(&v, 1, None, vec![device(1, Role::Main)]), &v.secret).expect("sign");
    let second = sign_list(
        list(
            &v,
            2,
            Some(first.hash),
            vec![device(1, Role::Main), device(2, Role::Linked)],
        ),
        &v.secret,
    )
    .expect("sign");
    let valid = valid_lists(&[stored(&first), stored(&second)], &v.pubkey);
    assert_eq!(effective(&valid).map(|s| s.hash), Some(second.hash));
}

#[test]
fn on_a_tie_the_smallest_hash_wins() {
    let v = vault();
    let base = sign_list(list(&v, 1, None, vec![device(1, Role::Main)]), &v.secret).expect("sign");
    let a = sign_list(
        list(
            &v,
            2,
            Some(base.hash),
            vec![device(1, Role::Main), device(2, Role::Linked)],
        ),
        &v.secret,
    )
    .expect("sign");
    let b = sign_list(
        list(
            &v,
            2,
            Some(base.hash),
            vec![device(1, Role::Main), device(3, Role::Linked)],
        ),
        &v.secret,
    )
    .expect("sign");
    let smallest = a.hash.min(b.hash);
    for order in [[&a, &b], [&b, &a]] {
        let rows = [stored(&base), stored(order[0]), stored(order[1])];
        let valid = valid_lists(&rows, &v.pubkey);
        assert_eq!(effective(&valid).map(|s| s.hash), Some(smallest));
    }
}

#[test]
fn two_main_devices_removing_each_other_leave_exactly_one_main_device() {
    let v = vault();
    let a = device(1, Role::Main);
    let b = device(2, Role::Main);
    let base = sign_list(list(&v, 1, None, vec![a.clone(), b.clone()]), &v.secret).expect("sign");
    let mut a_removes_b = list(&v, 2, Some(base.hash), vec![a.clone()]);
    a_removes_b.removed.push(removal(&b));
    let mut b_removes_a = list(&v, 2, Some(base.hash), vec![b.clone()]);
    b_removes_a.removed.push(removal(&a));
    let l1 = sign_list(a_removes_b, &v.secret).expect("sign");
    let l2 = sign_list(b_removes_a, &v.secret).expect("sign");

    let valid = valid_lists(&[stored(&base), stored(&l1), stored(&l2)], &v.pubkey);
    let winner = effective(&valid).expect("effective");
    let mains: Vec<_> = winner
        .list
        .devices
        .iter()
        .filter(|d| d.role == Role::Main)
        .collect();
    assert_eq!(mains.len(), 1, "SC-013: exactly one main device remains");
    let removed = if winner.hash == l1.hash { &b } else { &a };
    assert!(winner.list.removes(&removed.device_pubkey));
}

#[test]
fn a_list_signed_by_another_key_is_rejected() {
    let v = vault();
    let other = vault();
    let mut forged = list(&v, 1, None, vec![device(1, Role::Main)]);
    forged.vault = v.pubkey;
    let signed = sign_list(forged, &other.secret).expect("sign");
    assert_eq!(
        stored(&signed).check(&v.pubkey),
        Err(ListError::BadSignature)
    );
    assert!(valid_lists(&[stored(&signed)], &v.pubkey).is_empty());
}

#[test]
fn a_list_without_main_device_is_invalid() {
    let v = vault();
    let signed =
        sign_list(list(&v, 1, None, vec![device(1, Role::Linked)]), &v.secret).expect("sign");
    assert_eq!(
        stored(&signed).check(&v.pubkey),
        Err(ListError::NoMainDevice)
    );
}

#[test]
fn a_list_that_drops_a_removal_of_its_base_is_invalid() {
    let v = vault();
    let a = device(1, Role::Main);
    let b = device(2, Role::Linked);
    let mut first = list(&v, 1, None, vec![a.clone()]);
    first.removed.push(removal(&b));
    let first = sign_list(first, &v.secret).expect("sign");
    let second = sign_list(list(&v, 2, Some(first.hash), vec![a, b]), &v.secret).expect("sign");

    let valid = valid_lists(&[stored(&first), stored(&second)], &v.pubkey);
    assert!(!valid.contains_key(&second.hash));
    assert_eq!(effective(&valid).map(|s| s.hash), Some(first.hash));
}

#[test]
fn a_list_with_a_missing_base_is_invalid() {
    let v = vault();
    let orphan = sign_list(
        list(&v, 2, Some([7; 32]), vec![device(1, Role::Main)]),
        &v.secret,
    )
    .expect("sign");
    assert!(valid_lists(&[stored(&orphan)], &v.pubkey).is_empty());
}

#[test]
fn a_device_both_listed_and_removed_or_listed_twice_is_invalid() {
    let v = vault();
    let a = device(1, Role::Main);
    let mut both = list(&v, 1, None, vec![a.clone()]);
    both.removed.push(removal(&a));
    assert_eq!(both.check_structure(), Err(ListError::ListedAndRemoved));
    let twice = list(&v, 1, None, vec![a.clone(), a]);
    assert_eq!(twice.check_structure(), Err(ListError::Duplicate));
}

#[test]
fn only_generation_one_has_no_base() {
    let v = vault();
    assert_eq!(
        list(&v, 2, None, vec![device(1, Role::Main)]).check_structure(),
        Err(ListError::BaseRule)
    );
    assert_eq!(
        list(&v, 1, Some([1; 32]), vec![device(1, Role::Main)]).check_structure(),
        Err(ListError::BaseRule)
    );
}

#[test]
fn merging_carries_removals_forward_and_adds_the_losers_new_devices() {
    let v = vault();
    let a = device(1, Role::Main);
    let b = device(2, Role::Linked);
    let c = device(3, Role::Linked);
    let base = sign_list(list(&v, 1, None, vec![a.clone(), b.clone()]), &v.secret).expect("sign");
    let mut winner = list(&v, 2, Some(base.hash), vec![a.clone()]);
    winner.removed.push(removal(&b));
    let winner = sign_list(winner, &v.secret).expect("sign");
    let loser = sign_list(
        list(
            &v,
            2,
            Some(base.hash),
            vec![a.clone(), b.clone(), c.clone()],
        ),
        &v.secret,
    )
    .expect("sign");

    let merged = merge_next(&winner, &[&loser], a.device_pubkey, 10);

    assert_eq!(merged.generation, 3);
    assert_eq!(merged.base_list_hash, Some(winner.hash));
    assert!(merged.removes(&b.device_pubkey), "the removal stays");
    assert!(merged.device(&b.device_pubkey).is_none());
    assert!(
        merged.device(&c.device_pubkey).is_some(),
        "the new device joins"
    );
    let signed = sign_list(merged, &v.secret).expect("sign");
    let valid = valid_lists(
        &[
            stored(&base),
            stored(&winner),
            stored(&loser),
            stored(&signed),
        ],
        &v.pubkey,
    );
    assert_eq!(effective(&valid).map(|s| s.hash), Some(signed.hash));
}

#[test]
fn stored_lists_round_trip_through_the_vault() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = crate::sync::test_support::open_vault(dir.path());
    let v = vault();
    let signed =
        sign_list(list(&v, 1, None, vec![device(1, Role::Main)]), &v.secret).expect("sign");

    db.write(|tx| {
        insert(tx, &signed)?;
        insert(tx, &signed)
    })
    .expect("insert twice");

    let rows = crate::storage::query::read(&db, |r| load_all(r)).expect("load");
    assert_eq!(rows, vec![stored(&signed)]);
}
