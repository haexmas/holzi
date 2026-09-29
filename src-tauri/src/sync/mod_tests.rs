use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use iroh::RelayMode;

use super::*;
use crate::sync::endpoint::{NodeConfig, SyncNode};
use crate::sync::test_support::Member;

/// Binds `member`'s endpoint on loopback with no iroh-Relay.
async fn bind_loopback(member: &Member) -> SyncNode {
    SyncNode::bind(
        Arc::clone(&member.device.replica),
        member.keys.clone(),
        member.vault,
        NodeConfig {
            relay_mode: RelayMode::Disabled,
            bind_addr: Some((Ipv4Addr::LOCALHOST, 0).into()),
        },
        Arc::new(|_| {}),
    )
    .await
    .expect("bind")
}

#[tokio::test]
async fn reconnect_dials_a_listed_device_presence_has_an_address_for() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    linked.device.pull_from(&main.device);

    let main_node = bind_loopback(&main).await;
    let linked_node = bind_loopback(&linked).await;
    let main_addr = main_node.addr();

    linked
        .device
        .db()
        .write(|tx| presence::record_seen(tx, &main.keys.device_pubkey, 1, Some(&main_addr)))
        .expect("record presence");

    assert!(linked_node.connected().is_empty());
    reconnect_missing(&linked_node, &linked.device.replica).await;

    tokio::time::timeout(Duration::from_secs(5), async {
        while linked_node.connected().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("reconnect dials the address presence recorded");
    assert_eq!(linked_node.connected(), vec![main.keys.device_pubkey]);
}

#[tokio::test]
async fn reconnect_ignores_a_device_no_longer_on_the_list() {
    let main = Member::genesis();
    let stranger = Member::genesis();
    let main_node = bind_loopback(&main).await;

    main.device
        .db()
        .write(|tx| {
            presence::record_seen(tx, &stranger.keys.device_pubkey, 1, Some(&main_node.addr()))
        })
        .expect("record presence");

    // No panic, no connection attempt worth waiting for: the stranger is
    // not on `main`'s device list.
    reconnect_missing(&main_node, &main.device.replica).await;
    assert!(main_node.connected().is_empty());
}
