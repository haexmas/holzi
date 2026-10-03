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
    reconnect_missing(&linked_node, &linked.device.replica);

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
    reconnect_missing(&main_node, &main.device.replica);
    assert!(main_node.connected().is_empty());
}

/// A listed device that is gone keeps its dial waiting for the connect
/// timeout. Dials run apart, so the device that is there is reached at once
/// instead of after that wait.
#[tokio::test]
async fn a_device_that_is_gone_does_not_hold_up_the_others() {
    let main = Member::genesis();
    let (first, second) = (Member::join(&main), Member::join(&main));
    // The one that is gone comes first in any order the rows are read in.
    let (gone, there) = if first.keys.device_pubkey < second.keys.device_pubkey {
        (first, second)
    } else {
        (second, first)
    };
    main.add(&gone);
    main.add(&there);
    there.device.pull_from(&main.device);

    let main_node = bind_loopback(&main).await;
    let there_node = bind_loopback(&there).await;
    // A socket that takes the dial's packets and never answers.
    let silent = std::net::UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind");
    let gone_addr = iroh::EndpointAddr::new(
        iroh::EndpointId::from_bytes(&gone.keys.endpoint_id).expect("endpoint id"),
    )
    .with_ip_addr(silent.local_addr().expect("addr"));
    main.device
        .db()
        .write(|tx| {
            presence::record_seen(tx, &gone.keys.device_pubkey, 1, Some(&gone_addr))?;
            presence::record_seen(tx, &there.keys.device_pubkey, 1, Some(&there_node.addr()))
        })
        .expect("record presence");

    reconnect_missing(&main_node, &main.device.replica);

    tokio::time::timeout(Duration::from_secs(5), async {
        while main_node.connected().is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the device that is there is reached without waiting for the one that is gone");
    assert_eq!(main_node.connected(), vec![there.keys.device_pubkey]);
}
