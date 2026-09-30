use std::collections::BTreeSet;

use haex_crdt::rusqlite::params;

use super::*;
use crate::storage::query::{self, Query};
use crate::sync::test_support::{Device, Member};

/// A vault of three devices: a main device and two linked ones, all on the
/// list and all holding its data.
fn three() -> (Member, Member, Member) {
    let main = Member::genesis();
    let linked = Member::join(&main);
    let third = Member::join(&main);
    main.add(&linked);
    main.add(&third);
    linked.device.pull_from(&main.device);
    third.device.pull_from(&main.device);
    (main, linked, third)
}

fn write_thread(device: &Device, id: &str) {
    device
        .db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO chat_threads (id, title, created_at, updated_at) VALUES (?1, ?1, 1, 1)",
                params![id],
            )?;
            Ok(())
        })
        .expect("write thread");
}

fn has_thread(device: &Device, id: &str) -> bool {
    query::read(device.db(), |r| {
        let found: Option<String> = r.query_row(
            "SELECT id FROM chat_threads WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(found.is_some())
    })
    .expect("read thread")
}

fn listed(member: &Member) -> BTreeSet<[u8; 32]> {
    query::read(member.device.db(), |r| {
        let valid = device_list::valid_lists(&device_list::load_all(r)?, &member.vault);
        Ok(device_list::effective(&valid)
            .map(|s| s.list.devices.iter().map(|d| d.device_pubkey).collect())
            .unwrap_or_default())
    })
    .expect("read list")
}

fn recipients_of_generation(member: &Member, generation: u64) -> BTreeSet<Vec<u8>> {
    query::read(member.device.db(), |r| {
        let key_id: Option<Vec<u8>> = r.query_row(
            "SELECT key_id FROM vault_key_generations WHERE generation = ?1",
            params![i64::try_from(generation).expect("fits")],
            |row| row.get(0),
        )?;
        let key_id = key_id.expect("the generation exists");
        Ok(r.query_map(
            "SELECT recipient FROM vault_key_envelopes WHERE key_id = ?1",
            params![key_id],
            |row| row.get::<_, Vec<u8>>(0),
        )?
        .into_iter()
        .collect())
    })
    .expect("read envelopes")
}

fn remove(main: &Member, target: &Member) -> Result<Removal, RemovalError> {
    remove_device(
        &main.device.replica,
        &main.keys,
        &target.keys.device_pubkey,
        5_000,
    )
}

#[test]
fn the_new_list_omits_the_device_carries_its_removal_and_a_key_only_the_others_hold() {
    let (main, linked, third) = three();

    let removal = remove(&main, &linked).expect("removed");

    assert_eq!(removal.list_generation, 4, "two devices were added before");
    assert_eq!(removal.key_generation, 2);
    assert_eq!(
        listed(&main),
        BTreeSet::from([main.keys.device_pubkey, third.keys.device_pubkey])
    );
    let removed: Vec<_> = query::read(main.device.db(), |r| {
        let valid = device_list::valid_lists(&device_list::load_all(r)?, &main.vault);
        Ok(device_list::effective(&valid)
            .map(|s| s.list.removed.clone())
            .unwrap_or_default())
    })
    .expect("read");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].device_pubkey, linked.keys.device_pubkey);
    assert_eq!(removed[0].vault_device_uuid, linked.device.db().device_id());
    assert_eq!(removed[0].limit_hlc, removal.limit_hlc);
    assert_eq!(
        recipients_of_generation(&main, 2),
        BTreeSet::from([
            main.keys.device_pubkey.to_vec(),
            third.keys.device_pubkey.to_vec()
        ]),
        "FR-026: the new generation is wrapped for the remaining devices only"
    );
}

#[test]
fn the_limit_is_what_this_device_holds_of_the_removed_one() {
    let (main, linked, _third) = three();
    write_thread(&linked.device, "before");
    main.device.pull_from(&linked.device);
    write_thread(&linked.device, "after");
    let held = main
        .device
        .replica
        .progress()
        .expect("progress")
        .get(&linked.device.db().device_id())
        .cloned()
        .expect("main holds changes of linked");

    let removal = remove(&main, &linked).expect("removed");

    assert_eq!(removal.limit_hlc, held, "FR-028");
}

#[test]
fn a_removed_device_with_nothing_held_gets_a_limit_that_refuses_everything() {
    let (main, linked, _third) = three();
    write_thread(&linked.device, "never pulled");

    let removal = remove(&main, &linked).expect("removed");
    main.device.pull_from(&linked.device);

    assert_eq!(removal.limit_hlc, "0/0");
    assert!(!has_thread(&main.device, "never pulled"));
}

#[test]
fn changes_within_the_limit_stay_and_later_ones_are_refused_also_when_forwarded() {
    let (main, linked, third) = three();
    write_thread(&linked.device, "before");
    main.device.pull_from(&linked.device);
    third.device.pull_from(&linked.device);
    write_thread(&linked.device, "after");
    third.device.pull_from(&linked.device);
    assert!(
        has_thread(&third.device, "after"),
        "the third device holds both"
    );

    remove(&main, &linked).expect("removed");
    // The third device forwards everything it has of the removed device.
    main.device.pull_from(&third.device);

    assert!(
        has_thread(&main.device, "before"),
        "FR-028: within the limit, it stays"
    );
    assert!(
        !has_thread(&main.device, "after"),
        "beyond the limit it is refused, although a device forwards it"
    );
}

#[test]
fn the_remaining_devices_take_over_list_and_key_without_being_asked() {
    let (main, linked, third) = three();
    remove(&main, &linked).expect("removed");

    third.device.pull_from(&main.device);
    linked.device.pull_from(&main.device);

    let newest = |member: &Member| {
        member
            .device
            .db()
            .write(|tx| {
                let valid = device_list::valid_lists(&device_list::load_all(tx)?, &member.vault);
                content_keys::unwrap_own_envelopes(tx, &member.keys, &valid)?;
                Ok(())
            })
            .expect("unwrap");
        query::read(member.device.db(), |r| content_keys::current_key(r, &[]))
            .expect("read")
            .map(|key| key.generation)
    };
    assert_eq!(
        listed(&third),
        BTreeSet::from([main.keys.device_pubkey, third.keys.device_pubkey])
    );
    assert_eq!(
        newest(&third),
        Some(2),
        "FR-027: the third device opens the new key"
    );
    assert_ne!(
        newest(&linked),
        Some(2),
        "the removed device cannot open what only the others hold"
    );
}

#[test]
fn secrets_and_identity_stay_as_they_were() {
    let (main, linked, _third) = three();
    let before = query::read(main.device.db(), |r| {
        Ok((keys::vault_pubkey(r)?, keys::vault_secret(r)?.map(|s| *s)))
    })
    .expect("read");

    remove(&main, &linked).expect("removed");

    let after = query::read(main.device.db(), |r| {
        Ok((keys::vault_pubkey(r)?, keys::vault_secret(r)?.map(|s| *s)))
    })
    .expect("read");
    assert_eq!(before, after);
}

#[test]
fn the_presence_row_of_the_removed_device_goes() {
    let (main, linked, _third) = three();
    main.device
        .db()
        .write(|tx| crate::sync::presence::record_seen(tx, &linked.keys.device_pubkey, 9, None))
        .expect("record");

    remove(&main, &linked).expect("removed");

    let rows = query::read(main.device.db(), |r| crate::sync::presence::load_all(r)).expect("read");
    assert!(rows
        .iter()
        .all(|row| row.device_pubkey != linked.keys.device_pubkey));
}

#[test]
fn a_device_cannot_remove_itself() {
    let (main, _linked, _third) = three();

    let refused = remove(&main, &main);

    assert!(matches!(refused, Err(RemovalError::SelfRemoval)));
    assert_eq!(listed(&main).len(), 3);
}

#[test]
fn a_device_not_on_the_list_cannot_be_removed() {
    let (main, _linked, _third) = three();
    let stranger = Member::join(&main);

    let refused = remove(&main, &stranger);

    assert!(matches!(refused, Err(RemovalError::UnknownDevice)));
}

#[test]
fn only_a_main_device_removes() {
    let (_main, linked, third) = three();

    let refused = remove_device(
        &linked.device.replica,
        &linked.keys,
        &third.keys.device_pubkey,
        5_000,
    );

    assert!(matches!(refused, Err(RemovalError::NotMainDevice)));
    assert_eq!(listed(&linked).len(), 3, "nothing was published");
}

#[test]
fn removing_a_device_twice_is_refused_the_second_time() {
    let (main, linked, _third) = three();
    remove(&main, &linked).expect("removed");

    let again = remove(&main, &linked);

    assert!(matches!(again, Err(RemovalError::UnknownDevice)));
}

#[test]
fn a_second_removal_carries_the_first_one_forward() {
    let (main, linked, third) = three();
    remove(&main, &linked).expect("first");

    remove(&main, &third).expect("second");

    let removed: BTreeSet<[u8; 32]> = query::read(main.device.db(), |r| {
        let valid = device_list::valid_lists(&device_list::load_all(r)?, &main.vault);
        Ok(device_list::effective(&valid)
            .map(|s| s.list.removed.iter().map(|d| d.device_pubkey).collect())
            .unwrap_or_default())
    })
    .expect("read");
    assert_eq!(
        removed,
        BTreeSet::from([linked.keys.device_pubkey, third.keys.device_pubkey]),
        "a list never forgets a removal (R8)"
    );
    assert_eq!(listed(&main), BTreeSet::from([main.keys.device_pubkey]));
}
