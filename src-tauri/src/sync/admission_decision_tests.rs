use haex_crdt::rusqlite::params;

use super::tests::{
    count_requests, effective_of, keep, listed_roles, main_and_linked, request_of, NOW,
};
use super::*;
use crate::sync::content_keys::{self, ContentKey};
use crate::sync::device_list::{self, Role};

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
fn a_device_with_a_vault_secret_but_without_the_main_role_cannot_decide() {
    let (main, linked) = main_and_linked();
    main.issue_list(|mut list| {
        for device in &mut list.devices {
            if device.device_pubkey == main.keys.device_pubkey {
                device.role = Role::Linked;
            } else if device.device_pubkey == linked.keys.device_pubkey {
                device.role = Role::Main;
            }
        }
        list
    });
    let (copy, _) = linked.copy_of();
    keep(&main, &request_of(&copy, "Zweitrechner", 100));

    let refused = decide(
        &main.device.replica,
        &main.keys,
        &copy.keys.device_pubkey,
        true,
        NOW,
    );

    assert!(matches!(refused, Err(AdmissionError::NotMainDevice)));
    assert_eq!(count_requests(&main), 1, "the request remains undecided");
    assert_eq!(listed_roles(&main).len(), 2, "nothing was published");
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
