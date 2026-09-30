use haex_crdt::rusqlite::params;

use super::*;
use crate::storage::query::{self, Query};
use crate::sync::device_list::{self, Role};
use crate::sync::test_support::Member;

pub(super) const NOW: u64 = 1_000_000;

/// A vault of a main device and a linked one that holds its data.
pub(super) fn main_and_linked() -> (Member, Member) {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    linked.device.pull_from(&main.device);
    (main, linked)
}

pub(super) fn effective_of(member: &Member) -> SignedList {
    query::read(member.device.db(), |r| {
        let valid = device_list::valid_lists(&device_list::load_all(r)?, &member.vault);
        Ok(device_list::effective(&valid).expect("a list").clone())
    })
    .expect("read list")
}

pub(super) fn request_of(member: &Member, name: &str, requested_at: u64) -> Request {
    Request::sign(
        &member.keys,
        member.device.db().device_id(),
        name.to_string(),
        requested_at,
    )
    .expect("sign request")
}

pub(super) fn keep(member: &Member, request: &Request) -> bool {
    member
        .device
        .db()
        .write(|tx| store(tx, request))
        .expect("store request")
}

pub(super) fn sweep_now(member: &Member, now: u64) -> bool {
    let list = effective_of(member);
    member
        .device
        .db()
        .write(|tx| sweep(tx, now, &settled(&list)))
        .expect("sweep")
}

pub(super) fn state_of(member: &Member, device: &[u8; 32]) -> Option<String> {
    query::read(member.device.db(), |r| {
        r.query_row(
            "SELECT state FROM admission_requests WHERE device_pubkey = ?1",
            params![device.as_slice()],
            |row| row.get(0),
        )
    })
    .expect("read state")
}

pub(super) fn count_requests(member: &Member) -> i64 {
    query::read(member.device.db(), |r| {
        Ok(
            r.query_row("SELECT COUNT(*) FROM admission_requests", &[], |row| {
                row.get::<_, i64>(0)
            })?
            .unwrap_or(0),
        )
    })
    .expect("count")
}

pub(super) fn listed_roles(member: &Member) -> Vec<([u8; 32], Role)> {
    effective_of(member)
        .list
        .devices
        .iter()
        .map(|d| (d.device_pubkey, d.role))
        .collect()
}

#[test]
fn a_copy_of_a_main_device_enrolls_itself_and_leaves_the_source_alone() {
    let main = Member::genesis();
    let source_rows_before = device_key_rows(&main);

    let (copy, state) = main.copy_of();

    assert!(state.is_main && state.enrolled_as_copy);
    assert_ne!(copy.keys.device_pubkey, main.keys.device_pubkey);
    assert_ne!(copy.keys.endpoint_id, main.keys.endpoint_id);
    let enrolled = effective_of(&copy);
    assert_eq!(enrolled.list.generation, 2);
    assert_eq!(enrolled.list.base_list_hash, Some(effective_of(&main).hash));
    assert_eq!(enrolled.list.issued_by, copy.keys.device_pubkey);
    assert_eq!(
        listed_roles(&copy),
        vec![
            (main.keys.device_pubkey, Role::Main),
            (copy.keys.device_pubkey, Role::Main)
        ]
    );
    assert_eq!(
        enrolled
            .list
            .device(&copy.keys.device_pubkey)
            .expect("listed")
            .vault_device_uuid,
        copy.device.db().device_id(),
        "the list names the copy by its own node id"
    );
    let rows = device_key_rows(&copy);
    assert_eq!(rows.len(), 2, "the source's row stays, the copy adds one");
    assert!(
        source_rows_before.iter().all(|row| rows.contains(row)),
        "the source's keys are untouched in the copy (FR-006)"
    );
    assert_eq!(
        effective_of(&main).list.generation,
        1,
        "the source has not heard of the copy yet"
    );
    let notice = query::read(copy.device.db(), |r| {
        crate::storage::preferences::get(
            r,
            crate::storage::preferences::PrefScope::Device(copy.device.db().device_id()),
            PREF_ENROLLED_AS_MAIN,
        )
    })
    .expect("read notice");
    assert_eq!(notice.as_deref(), Some("1"));
}

#[test]
fn enrolling_happens_once() {
    let main = Member::genesis();
    let (copy, _) = main.copy_of();
    let installation = crate::identity::read_or_mint_installation_uuid(
        &crate::identity::installation_id_path(copy.device.dir()),
    )
    .expect("installation");

    let again = crate::sync::genesis::ensure_sync_state(copy.device.db(), installation, false)
        .expect("genesis again");

    assert!(!again.enrolled_as_copy);
    assert_eq!(effective_of(&copy).list.generation, 2);
}

#[test]
fn the_envelope_for_the_copy_opens_with_its_own_key() {
    let main = Member::genesis();
    let (copy, _) = main.copy_of();

    let own = query::read(copy.device.db(), |r| {
        Ok(r.query_row(
            "SELECT COUNT(*) FROM vault_key_envelopes WHERE recipient = ?1",
            params![copy.keys.device_pubkey.as_slice()],
            |row| row.get::<_, i64>(0),
        )?
        .unwrap_or(0))
    })
    .expect("count envelopes");

    assert_eq!(own, 1);
}

#[test]
fn a_copy_of_a_linked_device_waits_for_admission() {
    let (_main, linked) = main_and_linked();

    let (copy, state) = linked.copy_of();

    assert!(!state.is_main && !state.enrolled_as_copy);
    let list = effective_of(&copy);
    assert_eq!(list.list.generation, 2, "the list is the source's");
    assert!(needs_request(&list, &copy.keys.device_pubkey));
    assert!(!needs_request(&list, &linked.keys.device_pubkey));
}

#[test]
fn a_device_that_was_removed_needs_no_request() {
    let (main, linked) = main_and_linked();
    crate::sync::removal::remove_device(
        &main.device.replica,
        &main.keys,
        &linked.keys.device_pubkey,
        5_000,
    )
    .expect("removed");

    let list = effective_of(&main);

    assert!(!needs_request(&list, &linked.keys.device_pubkey));
    assert!(settled(&list).contains(&linked.keys.device_pubkey));
}

fn device_key_rows(member: &Member) -> Vec<(String, Vec<u8>, Vec<u8>)> {
    query::read(member.device.db(), |r| {
        r.query_map(
            "SELECT installation_uuid, device_secret, endpoint_secret FROM device_keys_no_sync \
             ORDER BY installation_uuid",
            &[],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
    })
    .expect("read key rows")
}

fn alias_of(member: &Member) -> Option<String> {
    query::read(member.device.db(), |r| {
        r.query_row(
            "SELECT alias FROM known_devices WHERE vault_device_uuid = ?1",
            params![member.device.db().device_id().to_string()],
            |row| row.get(0),
        )
    })
    .expect("read alias")
}

#[test]
fn a_copy_takes_the_computers_name_so_it_and_its_source_tell_apart() {
    let Some(computer) = crate::hardware::hostname::suggested_alias() else {
        return;
    };
    let main = Member::genesis();
    let (main_copy, _) = main.copy_of();
    let (_main, linked) = main_and_linked();
    let (linked_copy, _) = linked.copy_of();

    assert_eq!(alias_of(&main_copy), Some(computer.clone()));
    assert_eq!(alias_of(&linked_copy), Some(computer));
}

#[test]
fn a_name_the_user_chose_stays() {
    let main = Member::genesis();
    let (copy, _) = main.copy_of();
    let uuid = copy.device.db().device_id();
    copy.device
        .db()
        .write(|tx| {
            tx.execute(
                "UPDATE known_devices SET alias = 'Büro' WHERE vault_device_uuid = ?1",
                params![uuid.to_string()],
            )?;
            adopt_computer_name(tx, uuid)
        })
        .expect("adopt");

    assert_eq!(alias_of(&copy).as_deref(), Some("Büro"));
}
