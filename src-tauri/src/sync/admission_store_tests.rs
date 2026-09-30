use std::collections::HashSet;

use iroh::{EndpointAddr, TransportAddr};

use super::tests::{
    count_requests, keep, listed_roles, main_and_linked, request_of, state_of, sweep_now, NOW,
};
use super::*;
use crate::storage::query::{self, Query};
use crate::sync::test_support::Member;

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
