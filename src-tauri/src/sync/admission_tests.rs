use std::collections::HashSet;

use haex_crdt::rusqlite::params;
use iroh::TransportAddr;

use super::*;
use crate::storage::query::{self, Query};
use crate::sync::content_keys::ContentKey;
use crate::sync::test_support::Member;

const NOW: u64 = 1_000_000;

/// A vault of a main device and a linked one that holds its data.
fn main_and_linked() -> (Member, Member) {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    linked.device.pull_from(&main.device);
    (main, linked)
}

fn effective_of(member: &Member) -> SignedList {
    query::read(member.device.db(), |r| {
        let valid = device_list::valid_lists(&device_list::load_all(r)?, &member.vault);
        Ok(device_list::effective(&valid).expect("a list").clone())
    })
    .expect("read list")
}

fn request_of(member: &Member, name: &str, requested_at: u64) -> Request {
    Request::sign(
        &member.keys,
        member.device.db().device_id(),
        name.to_string(),
        requested_at,
    )
    .expect("sign request")
}

fn keep(member: &Member, request: &Request) -> bool {
    member
        .device
        .db()
        .write(|tx| store(tx, request))
        .expect("store request")
}

fn sweep_now(member: &Member, now: u64) -> bool {
    let list = effective_of(member);
    member
        .device
        .db()
        .write(|tx| sweep(tx, now, &settled(&list)))
        .expect("sweep")
}

fn state_of(member: &Member, device: &[u8; 32]) -> Option<String> {
    query::read(member.device.db(), |r| {
        r.query_row(
            "SELECT state FROM admission_requests WHERE device_pubkey = ?1",
            params![device.as_slice()],
            |row| row.get(0),
        )
    })
    .expect("read state")
}

fn count_requests(member: &Member) -> i64 {
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

fn listed_roles(member: &Member) -> Vec<([u8; 32], Role)> {
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
fn a_request_carries_the_name_and_verifies_only_as_signed() {
    let (_main, linked) = main_and_linked();
    let (copy, _) = linked.copy_of();
    let request = request_of(&copy, "Zweitrechner", NOW);

    assert!(request.verify().is_ok());
    let mut tampered = request.clone();
    tampered.name = "Laptop".into();
    assert!(tampered.verify().is_err());
    let mut other_uuid = request.clone();
    other_uuid.vault_device_uuid = Uuid::new_v4();
    assert!(other_uuid.verify().is_err());
    let mut other_time = request.clone();
    other_time.requested_at += 1;
    assert!(other_time.verify().is_err());
}

#[test]
fn content_survives_its_json_and_is_checked_on_receipt() {
    let (_main, linked) = main_and_linked();
    let (copy, _) = linked.copy_of();
    let request = request_of(&copy, "Zweitrechner", NOW);
    let addr = EndpointAddr::from_parts(
        iroh::EndpointId::from_bytes(&copy.keys.endpoint_id).expect("endpoint"),
        [TransportAddr::Ip("192.0.2.7:4433".parse().expect("addr"))],
    );
    let json = serde_json::to_string(&Content::new(&request, &addr)).expect("encode");
    let content: Content = serde_json::from_str(&json).expect("decode");
    let sender = copy.keys.device_pubkey;

    assert_eq!(content.request(&sender, NOW), Some(request.clone()));
    assert_eq!(content.endpoint_addr(), Some(addr));
    assert_eq!(
        content.request(&[7; 32], NOW),
        None,
        "the seal's signer must be the device it names"
    );
    assert_eq!(
        content.request(&sender, NOW + MAX_AGE_MS + 1),
        None,
        "older than 30 days"
    );
    assert_eq!(
        content.request(&sender, NOW - MAX_FUTURE_MS - 1),
        None,
        "from the future"
    );
    let mut forged = content.clone();
    forged.name = "Laptop".into();
    assert_eq!(forged.request(&sender, NOW), None, "the signature must fit");
    let mut long = content;
    long.name = "x".repeat(MAX_NAME_BYTES + 1);
    assert_eq!(long.request(&sender, NOW), None);
}

#[test]
fn a_newer_request_replaces_an_open_one_and_a_repeat_changes_nothing() {
    let (main, linked) = main_and_linked();
    let (copy, _) = linked.copy_of();
    let first = request_of(&copy, "Zweitrechner", 100);

    assert!(keep(&main, &first));
    assert!(!keep(&main, &first), "the same request again");
    assert!(keep(&main, &request_of(&copy, "Neuer Name", 200)));

    let open = query::read(main.device.db(), |r| load_open(r, &HashSet::new())).expect("open");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].name, "Neuer Name");
    assert_eq!(open[0].requested_at, 200);
    assert!(
        !keep(&main, &request_of(&copy, "Älter", 50)),
        "older never wins"
    );
}

#[test]
fn a_refused_request_stays_refused_when_the_copy_asks_again() {
    let (main, linked) = main_and_linked();
    let (copy, _) = linked.copy_of();
    keep(&main, &request_of(&copy, "Zweitrechner", 100));

    let decided = decide(
        &main.device.replica,
        &main.keys,
        &copy.keys.device_pubkey,
        false,
        NOW,
    )
    .expect("refused");

    assert_eq!(decided, Decided::Refused);
    assert!(!keep(&main, &request_of(&copy, "Zweitrechner", 900)));
    assert_eq!(
        state_of(&main, &copy.keys.device_pubkey).as_deref(),
        Some("rejected")
    );
    let open = query::read(main.device.db(), |r| load_open(r, &HashSet::new())).expect("open");
    assert!(open.is_empty(), "a refused request is no longer offered");
    assert_eq!(listed_roles(&main).len(), 2, "nobody was admitted");
}

#[test]
fn only_the_twenty_smallest_open_requests_stay_open() {
    let main = Member::genesis();
    let mut devices: Vec<Request> = Vec::new();
    for at in 0..25u64 {
        let stranger = Member::genesis();
        devices.push(request_of(&stranger, "x", 1_000 + at));
    }
    // Inserted newest first, to show the order comes from the data and not
    // from the order they arrived in.
    for request in devices.iter().rev() {
        keep(&main, request);
    }

    assert!(sweep_now(&main, NOW));

    let open = query::read(main.device.db(), |r| load_open(r, &HashSet::new())).expect("open");
    assert_eq!(open.len(), OPEN_LIMIT);
    let expected: Vec<u64> = (0..OPEN_LIMIT as u64).map(|i| 1_000 + i).collect();
    assert_eq!(
        open.iter().map(|r| r.requested_at).collect::<Vec<_>>(),
        expected
    );
    for request in &devices[OPEN_LIMIT..] {
        assert_eq!(
            state_of(&main, &request.device).as_deref(),
            Some("rejected")
        );
    }
    assert!(!sweep_now(&main, NOW), "a second sweep changes nothing");
}

#[test]
fn the_same_requests_end_in_the_same_state_on_every_device() {
    let (first, second) = (Member::genesis(), Member::genesis());
    let requests: Vec<Request> = (0..23u64)
        .map(|at| request_of(&Member::genesis(), "x", 500 + (at * 7) % 23))
        .collect();
    for request in &requests {
        keep(&first, request);
    }
    for request in requests.iter().rev() {
        keep(&second, request);
    }

    sweep_now(&first, NOW);
    sweep_now(&second, NOW);

    let states = |member: &Member| -> Vec<(Vec<u8>, String)> {
        query::read(member.device.db(), |r| {
            r.query_map(
                "SELECT device_pubkey, state FROM admission_requests ORDER BY device_pubkey",
                &[],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
        })
        .expect("states")
    };
    assert_eq!(states(&first), states(&second));
}

#[test]
fn a_sweep_drops_old_requests_and_those_of_devices_already_on_the_list() {
    let (main, linked) = main_and_linked();
    let stranger = Member::genesis();
    keep(&main, &request_of(&stranger, "alt", 10));
    keep(&main, &request_of(&linked, "schon da", NOW));
    let fresh = Member::genesis();
    keep(&main, &request_of(&fresh, "frisch", NOW));

    assert!(sweep_now(&main, NOW + MAX_AGE_MS));

    assert_eq!(count_requests(&main), 1, "only the fresh one is left");
    assert!(state_of(&main, &fresh.keys.device_pubkey).is_some());
}

#[test]
fn admitting_lists_the_copy_as_a_linked_device_and_wraps_every_generation_for_it() {
    let (main, linked) = main_and_linked();
    let list = effective_of(&main);
    main.device
        .db()
        .write(|tx| {
            content_keys::issue_generation(tx, &ContentKey::generate(2), &list, &main.keys, 5)
        })
        .expect("a second generation");
    let (copy, _) = linked.copy_of();
    keep(&main, &request_of(&copy, "Zweitrechner", 100));

    let decided = decide(
        &main.device.replica,
        &main.keys,
        &copy.keys.device_pubkey,
        true,
        NOW,
    )
    .expect("admitted");

    assert_eq!(
        decided,
        Decided::Admitted {
            list_generation: 3,
            endpoint_id: copy.keys.endpoint_id
        }
    );
    let list = effective_of(&main);
    let entry = list.list.device(&copy.keys.device_pubkey).expect("listed");
    assert_eq!(entry.role, Role::Linked);
    assert_eq!(entry.vault_device_uuid, copy.device.db().device_id());
    assert_eq!(entry.endpoint_id, copy.keys.endpoint_id);
    assert_eq!(count_requests(&main), 0, "the request is gone");

    copy.device.pull_from(&main.device);
    let unwrapped = copy
        .device
        .db()
        .write(|tx| {
            let valid = device_list::valid_lists(&device_list::load_all(tx)?, &copy.vault);
            content_keys::unwrap_own_envelopes(tx, &copy.keys, &valid)
        })
        .expect("unwrap");
    assert_eq!(unwrapped, 2, "both generations open for the copy");
}

#[test]
fn a_linked_device_neither_admits_nor_refuses() {
    let (main, linked) = main_and_linked();
    let (copy, _) = linked.copy_of();
    keep(&linked, &request_of(&copy, "Zweitrechner", 100));

    for admit in [true, false] {
        let refused = decide(
            &linked.device.replica,
            &linked.keys,
            &copy.keys.device_pubkey,
            admit,
            NOW,
        );
        assert!(matches!(refused, Err(AdmissionError::NotMainDevice)));
    }
    assert_eq!(listed_roles(&main).len(), 2);
    assert_eq!(count_requests(&linked), 1);
}

#[test]
fn a_decision_needs_an_open_request_and_is_made_once() {
    let (main, linked) = main_and_linked();
    let (copy, _) = linked.copy_of();

    let none = decide(
        &main.device.replica,
        &main.keys,
        &copy.keys.device_pubkey,
        true,
        NOW,
    );
    assert!(matches!(none, Err(AdmissionError::NoRequest)));

    keep(&main, &request_of(&copy, "Zweitrechner", 100));
    decide(
        &main.device.replica,
        &main.keys,
        &copy.keys.device_pubkey,
        true,
        NOW,
    )
    .expect("admitted");
    let twice = decide(
        &main.device.replica,
        &main.keys,
        &copy.keys.device_pubkey,
        true,
        NOW,
    );
    assert!(matches!(twice, Err(AdmissionError::NoRequest)));
}

#[test]
fn a_stale_request_of_a_listed_device_only_goes_away() {
    let (main, linked) = main_and_linked();
    keep(&main, &request_of(&linked, "schon da", 100));

    let decided = decide(
        &main.device.replica,
        &main.keys,
        &linked.keys.device_pubkey,
        true,
        NOW,
    )
    .expect("decided");

    assert_eq!(decided, Decided::AlreadyListed);
    assert_eq!(listed_roles(&main).len(), 2, "no new list");
    assert_eq!(count_requests(&main), 0);
}

#[test]
fn a_request_whose_signature_does_not_verify_is_dropped_not_admitted() {
    let (main, linked) = main_and_linked();
    let (copy, _) = linked.copy_of();
    keep(&main, &request_of(&copy, "Zweitrechner", 100));
    main.device
        .db()
        .write(|tx| {
            tx.execute(
                "UPDATE admission_requests SET name = 'Laptop' WHERE device_pubkey = ?1",
                params![copy.keys.device_pubkey.as_slice()],
            )
        })
        .expect("tamper");

    let decided = decide(
        &main.device.replica,
        &main.keys,
        &copy.keys.device_pubkey,
        true,
        NOW,
    )
    .expect("decided");

    assert_eq!(decided, Decided::Dropped);
    assert_eq!(listed_roles(&main).len(), 2, "nobody was admitted");
    assert_eq!(count_requests(&main), 0);
}

#[test]
fn a_removed_device_is_not_admitted_again() {
    let (main, linked) = main_and_linked();
    crate::sync::removal::remove_device(
        &main.device.replica,
        &main.keys,
        &linked.keys.device_pubkey,
        5_000,
    )
    .expect("removed");
    keep(&main, &request_of(&linked, "zurück", 100));

    let decided = decide(
        &main.device.replica,
        &main.keys,
        &linked.keys.device_pubkey,
        true,
        NOW,
    )
    .expect("decided");

    assert_eq!(decided, Decided::Dropped);
    assert_eq!(listed_roles(&main).len(), 1);
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
