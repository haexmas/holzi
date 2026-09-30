use std::net::Ipv4Addr;

use super::tests::bind_loopback;
use super::*;
use crate::sync::presence::{day_tag_now, load_all, now_ms};
use crate::sync::test_support::Member;

/// The gift-wrapped request `copy` sends for `receiver`'s mailbox of `day`.
fn request_event(receiver: &Member, copy: &Member, day: u32, asked_at: u64) -> Event {
    let (_, mb_pk) = mailbox_of(receiver, day);
    let request = admission::Request::sign(
        &copy.keys,
        copy.device.db().device_id(),
        "Zweitrechner".to_string(),
        asked_at,
    )
    .expect("sign");
    let addr = iroh::EndpointAddr::from_parts(
        iroh::EndpointId::from_bytes(&copy.keys.endpoint_id).expect("endpoint"),
        [iroh::TransportAddr::Ip((Ipv4Addr::LOCALHOST, 4000).into())],
    );
    let json = serde_json::to_string(&admission::Content::new(&request, &addr)).expect("encode");
    wrap_payload(ADMISSION_KIND, &copy.keys.device_secret, &json, &mb_pk).expect("wrap")
}

/// The mailbox of `day` for the content key `member` holds.
fn mailbox_of(member: &Member, day: u32) -> (nostr::key::SecretKey, PublicKey) {
    let content_key = crate::storage::query::read(member.device.db(), |r| {
        crate::sync::content_keys::current_key(r, &[])
    })
    .expect("read")
    .expect("a content key");
    mailbox_keys(&content_key.key, day).expect("mailbox keys")
}

fn open_requests_of(member: &Member) -> Vec<admission::OpenRequest> {
    crate::storage::query::read(member.device.db(), |r| {
        let valid = crate::sync::device_list::valid_lists(
            &crate::sync::device_list::load_all(r)?,
            &member.vault,
        );
        let settled = crate::sync::device_list::effective(&valid)
            .map(admission::settled)
            .unwrap_or_default();
        admission::load_open(r, &settled)
    })
    .expect("read requests")
}

/// A main device, a linked device and a copy of the linked device's file.
fn copy_of_linked() -> (Member, Member, Member) {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    // The linked device holds the content key, as a real one does, so its
    // copy can open the mailbox.
    let list = crate::storage::query::read(main.device.db(), |r| {
        let valid = crate::sync::device_list::valid_lists(
            &crate::sync::device_list::load_all(r)?,
            &main.vault,
        );
        Ok(crate::sync::device_list::effective(&valid)
            .expect("a list")
            .clone())
    })
    .expect("read list");
    main.device
        .db()
        .write(|tx| {
            let key = crate::sync::content_keys::current_key(tx, &[])?.expect("a content key");
            crate::sync::content_keys::issue_generation(tx, &key, &list, &main.keys, 2)
        })
        .expect("wrap the key for the linked device");
    linked.device.pull_from(&main.device);
    linked
        .device
        .db()
        .write(|tx| {
            let valid = crate::sync::device_list::valid_lists(
                &crate::sync::device_list::load_all(tx)?,
                &linked.vault,
            );
            crate::sync::content_keys::unwrap_own_envelopes(tx, &linked.keys, &valid)
        })
        .expect("unwrap");
    let (copy, _) = linked.copy_of();
    (main, linked, copy)
}

/// FR-045: a listed device keeps a valid request of a device the list does
/// not name, for the main devices to decide on, and dials nobody for it.
#[tokio::test]
async fn a_request_from_an_unknown_device_is_kept_for_a_decision() {
    let (main, _linked, copy) = copy_of_linked();
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let event = request_event(&main, &copy, day, now_ms());

    let dial = handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;

    assert!(!dial, "a request is no reason to dial");
    let open = open_requests_of(&main);
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].device, copy.keys.device_pubkey);
    assert_eq!(open[0].name, "Zweitrechner");
    let rows = crate::storage::query::read(main.device.db(), |r| load_all(r)).expect("read");
    assert!(rows.is_empty(), "nothing is recorded as presence for it");
}

#[tokio::test]
async fn a_request_is_kept_once_however_often_it_comes() {
    let (main, _linked, copy) = copy_of_linked();
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let asked_at = now_ms();

    for _ in 0..3 {
        let event = request_event(&main, &copy, day, asked_at);
        handle_incoming(
            &node,
            &main.device.replica,
            &main.keys,
            main.vault,
            day,
            &event,
        )
        .await;
    }

    assert_eq!(open_requests_of(&main).len(), 1);
}

/// A request whose signature does not fit its content is dropped.
#[tokio::test]
async fn a_request_with_a_forged_name_is_dropped() {
    let (main, _linked, copy) = copy_of_linked();
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let (_, mb_pk) = mailbox_of(&main, day);
    let request = admission::Request::sign(
        &copy.keys,
        copy.device.db().device_id(),
        "Zweitrechner".to_string(),
        now_ms(),
    )
    .expect("sign");
    let mut content = admission::Content::new(&request, &node.addr());
    content.name = "Laptop".to_string();
    let json = serde_json::to_string(&content).expect("encode");
    let event =
        wrap_payload(ADMISSION_KIND, &copy.keys.device_secret, &json, &mb_pk).expect("wrap");

    handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;

    assert!(open_requests_of(&main).is_empty());
}

/// A device the list does not name keeps no request: only the devices of
/// the vault do.
#[tokio::test]
async fn a_device_the_list_does_not_name_keeps_no_requests() {
    let (_main, _linked, copy) = copy_of_linked();
    let other = copy.copy_of().0;
    let node = bind_loopback(&copy).await;
    let day = day_tag_now();
    let event = request_event(&copy, &other, day, now_ms());

    handle_incoming(
        &node,
        &copy.device.replica,
        &copy.keys,
        copy.vault,
        day,
        &event,
    )
    .await;

    assert!(open_requests_of(&copy).is_empty());
}

/// A request from a device a main device has admitted since says where to
/// dial it: the copy has not heard of its admission yet.
#[tokio::test]
async fn a_request_from_an_admitted_device_is_presence() {
    let (main, _linked, copy) = copy_of_linked();
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let event = request_event(&main, &copy, day, now_ms());
    handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;
    admission::decide(
        &main.device.replica,
        &main.keys,
        &copy.keys.device_pubkey,
        true,
        now_ms(),
    )
    .expect("admitted");

    let dial = handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;

    assert!(dial, "the admitted copy is dialed");
    let rows = crate::storage::query::read(main.device.db(), |r| load_all(r)).expect("read");
    assert!(rows
        .iter()
        .any(|row| row.device_pubkey == copy.keys.device_pubkey && row.endpoint_addr.is_some()));
    assert!(open_requests_of(&main).is_empty());
}

/// FR-007: a copy of a main device enrolled itself on a newer list; its
/// presence makes the source dial it once, to fetch that list.
#[tokio::test]
async fn a_meeting_that_claims_a_newer_list_is_dialed_once_to_fetch_it() {
    let source = Member::genesis();
    let (copy, state) = source.copy_of();
    assert!(state.enrolled_as_copy);
    let node = bind_loopback(&source).await;
    let day = day_tag_now();
    let content = PresenceContent::own(
        copy.keys.device_pubkey,
        copy.keys.endpoint_id,
        None,
        vec![(Ipv4Addr::LOCALHOST, 4000).into()],
        2,
    );
    let (_, mb_pk) = mailbox_of(&source, day);
    let event = build(&copy.keys, &content, &mb_pk).expect("build");

    let dial = handle_incoming(
        &node,
        &source.device.replica,
        &source.keys,
        source.vault,
        day,
        &event,
    )
    .await;

    assert!(dial, "reconnect is woken to dial the copy");
    assert_eq!(node.take_candidates().len(), 1);
    assert!(
        node.take_candidates().is_empty(),
        "each announcement is dialed once"
    );
}

/// Only a claim of a newer list than the one held is followed: a stranger
/// announcing the same generation is not dialed.
#[tokio::test]
async fn a_meeting_of_an_unknown_device_without_a_newer_list_is_not_dialed() {
    let main = Member::genesis();
    let stranger = Member::genesis();
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let content = PresenceContent::own(
        stranger.keys.device_pubkey,
        stranger.keys.endpoint_id,
        None,
        vec![(Ipv4Addr::LOCALHOST, 4000).into()],
        1,
    );
    let (_, mb_pk) = mailbox_of(&main, day);
    let event = build(&stranger.keys, &content, &mb_pk).expect("build");

    let dial = handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;

    assert!(!dial);
    assert!(node.take_candidates().is_empty());
}

/// FR-007: a device that is the only one of its vault still listens, so the
/// request of a copy of its file reaches it.
#[tokio::test]
async fn a_one_device_vault_still_listens_for_requests() {
    let main = Member::genesis();
    let asking = Member::join(&main);
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let event = request_event(&main, &asking, day, now_ms());

    handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;

    assert_eq!(open_requests_of(&main).len(), 1);
}
