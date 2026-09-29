use tokio::io::{duplex, split};

use super::*;
use crate::sync::device_list::RemovedDevice;
use crate::sync::test_support::Member;

/// Runs `accept` on `a` and `dial` on `d` against each other, each seeing
/// the other's endpoint unless `d_claims` overrides what `a` sees.
async fn run(
    a: &Member,
    d: &Member,
    a_local: Local<'_>,
    d_local: Local<'_>,
    endpoint_a_sees: [u8; 32],
) -> (Result<Peer, HandshakeError>, Result<Peer, HandshakeError>) {
    let (a_side, d_side) = duplex(1 << 20);
    let (mut a_recv, mut a_send) = split(a_side);
    let (mut d_recv, mut d_send) = split(d_side);
    tokio::join!(
        accept(
            &mut a_send,
            &mut a_recv,
            &a.device.replica,
            &a_local,
            endpoint_a_sees
        ),
        dial(
            &mut d_send,
            &mut d_recv,
            &d.device.replica,
            &d_local,
            a.keys.endpoint_id
        ),
    )
}

async fn handshake(
    a: &Member,
    d: &Member,
) -> (Result<Peer, HandshakeError>, Result<Peer, HandshakeError>) {
    run(a, d, a.local(), d.local(), d.keys.endpoint_id).await
}

#[tokio::test]
async fn two_listed_devices_accept_each_other() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    linked.device.pull_from(&main.device);

    let (at_main, at_linked) = handshake(&main, &linked).await;

    let peer = at_main.expect("main accepts");
    assert_eq!(peer.device_pubkey, linked.keys.device_pubkey);
    assert_eq!(peer.role, Role::Linked);
    assert_eq!(peer.vault_device_uuid, linked.device.db().device_id());
    let peer = at_linked.expect("linked accepts");
    assert_eq!(peer.device_pubkey, main.keys.device_pubkey);
    assert_eq!(peer.role, Role::Main);
}

#[tokio::test]
async fn the_newer_list_travels_in_the_handshake_both_ways() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    // The linked device still holds generation 1, which does not name it.

    let (at_main, at_linked) = handshake(&main, &linked).await;
    assert!(
        at_main.is_ok() && at_linked.is_ok(),
        "{at_main:?} {at_linked:?}"
    );
    assert_eq!(at_linked.expect("peer").list.generation, 2);

    // And from the dialing side: the main device dials, the linked device
    // accepts while it holds the older list.
    let other = Member::join(&main);
    main.add(&other);
    let (at_other, at_main) = handshake(&other, &main).await;
    assert!(
        at_other.is_ok() && at_main.is_ok(),
        "{at_other:?} {at_main:?}"
    );
    assert_eq!(at_other.expect("peer").list.generation, 3);
}

#[tokio::test]
async fn a_device_on_no_list_is_refused() {
    let main = Member::genesis();
    let stranger = Member::join(&main);

    let (at_main, at_stranger) = handshake(&main, &stranger).await;

    assert!(matches!(
        at_main,
        Err(HandshakeError::Refused(RejectCode::NotOnList))
    ));
    assert!(matches!(
        at_stranger,
        Err(HandshakeError::RefusedByPeer(RejectCode::NotOnList))
    ));
}

#[tokio::test]
async fn a_removed_device_stays_refused() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    let removed = RemovedDevice {
        device_pubkey: linked.keys.device_pubkey,
        vault_device_uuid: linked.device.db().device_id(),
        limit_hlc: "0/0".to_string(),
        removed_at: 3,
    };
    main.issue_list(|mut list| {
        list.devices
            .retain(|d| d.device_pubkey != removed.device_pubkey);
        list.removed.push(removed);
        list
    });

    let (at_main, _) = handshake(&main, &linked).await;

    assert!(matches!(
        at_main,
        Err(HandshakeError::Refused(RejectCode::Removed))
    ));
}

#[tokio::test]
async fn a_signature_for_another_endpoint_is_refused() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);

    // The connection claims to come from an endpoint the dialer did not sign for.
    let (at_main, _) = run(&main, &linked, main.local(), linked.local(), [9; 32]).await;

    assert!(matches!(
        at_main,
        Err(HandshakeError::Refused(RejectCode::BadSignature))
    ));
}

#[tokio::test]
async fn a_device_of_another_vault_is_refused() {
    let main = Member::genesis();
    let foreign = Member::genesis();

    let (at_main, at_foreign) = handshake(&main, &foreign).await;

    assert!(matches!(
        at_main,
        Err(HandshakeError::Refused(RejectCode::ForeignVault))
    ));
    assert!(matches!(
        at_foreign,
        Err(HandshakeError::RefusedByPeer(RejectCode::ForeignVault))
    ));
}

#[tokio::test]
async fn another_schema_version_is_refused() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    let mut newer = linked.local();
    newer.schema.holzi_migration += 1;

    let (at_main, at_linked) =
        run(&main, &linked, main.local(), newer, linked.keys.endpoint_id).await;

    assert!(matches!(
        at_main,
        Err(HandshakeError::Refused(RejectCode::Incompatible))
    ));
    assert!(matches!(
        at_linked,
        Err(HandshakeError::RefusedByPeer(RejectCode::Incompatible))
    ));
}

#[test]
fn the_schema_names_this_builds_migrations_and_triggers() {
    let schema = local_schema();
    assert_eq!(schema.protocol, 1);
    assert!(schema.holzi_migration >= 21);
    assert_eq!(schema.crdt_trigger, HOLZI_TRIGGER_VERSION as u32);
}
