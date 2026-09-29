use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use iroh::RelayMode;

use super::*;
use crate::sync::endpoint::{NodeConfig, SyncNode};
use crate::sync::keys::DeviceKeys;
use crate::sync::test_support::Member;

#[test]
fn mailbox_keys_are_deterministic_from_the_same_content_key_and_day() {
    let content_key = [7u8; 32];
    let (secret_a, public_a) = mailbox_keys(&content_key, 42).expect("keys");
    let (secret_b, public_b) = mailbox_keys(&content_key, 42).expect("keys");
    assert_eq!(secret_a.secret_bytes(), secret_b.secret_bytes());
    assert_eq!(public_a, public_b);
}

#[test]
fn mailbox_keys_differ_by_day() {
    let content_key = [7u8; 32];
    let (_, today) = mailbox_keys(&content_key, 42).expect("keys");
    let (_, yesterday) = mailbox_keys(&content_key, 41).expect("keys");
    assert_ne!(today, yesterday);
}

#[test]
fn a_receiver_with_the_same_content_key_opens_the_meeting_and_learns_the_real_sender() {
    let content_key = [3u8; 32];
    let day = day_tag_now();
    let (mb_sk, mb_pk) = mailbox_keys(&content_key, day).expect("mailbox keys");

    let sender = DeviceKeys::generate();
    let content = PresenceContent::own(
        sender.device_pubkey,
        sender.endpoint_id,
        Some("https://relay.example.org".to_string()),
        vec!["203.0.113.5:4433".parse().expect("addr")],
        3,
    );

    let gift_wrap = build(&sender, &content, &mb_pk).expect("build");
    let (opened_sender, opened_content) = open(&gift_wrap, &mb_sk).expect("open");

    assert_eq!(opened_sender, sender.device_pubkey);
    assert_eq!(opened_content, content);
    assert!(opened_content.is_fresh(opened_content.ts));
}

#[test]
fn a_different_day_mailbox_cannot_open_the_meeting() {
    let content_key = [3u8; 32];
    let (_, mb_pk) = mailbox_keys(&content_key, 100).expect("mailbox keys");
    let (wrong_sk, _) = mailbox_keys(&content_key, 101).expect("mailbox keys");

    let sender = DeviceKeys::generate();
    let content = PresenceContent::own(sender.device_pubkey, sender.endpoint_id, None, vec![], 1);
    let gift_wrap = build(&sender, &content, &mb_pk).expect("build");

    assert!(open(&gift_wrap, &wrong_sk).is_err());
}

#[test]
fn a_meeting_is_only_fresh_within_the_bounds() {
    let content = PresenceContent {
        v: 1,
        device: [0; 32],
        endpoint: [0; 32],
        iroh_relay: None,
        addrs: Vec::new(),
        list_generation: 1,
        ts: 1_000_000,
        nonce: [0; 16],
    };
    assert!(content.is_fresh(1_000_000));
    assert!(content.is_fresh(1_000_000 + MAX_AGE_MS));
    assert!(!content.is_fresh(1_000_000 + MAX_AGE_MS + 1));
    assert!(content.is_fresh(1_000_000 - MAX_FUTURE_MS));
    assert!(!content.is_fresh(1_000_000 - MAX_FUTURE_MS - 1));
}

#[test]
fn the_content_survives_the_json_roundtrip_with_hex_fields() {
    let content = PresenceContent::own(
        [9u8; 32],
        [8u8; 32],
        Some("https://relay.example.org".to_string()),
        vec!["203.0.113.5:4433".parse().expect("addr")],
        2,
    );
    let json = serde_json::to_string(&content).expect("encode");
    assert!(json.contains("\"device\":\"09090909"));
    let back: PresenceContent = serde_json::from_str(&json).expect("decode");
    assert_eq!(back, content);
}

#[test]
fn a_one_device_vault_has_no_peers_to_publish_to() {
    let solo = Member::genesis();
    let roster = read_roster(&solo.device.replica, solo.vault)
        .expect("read roster")
        .expect("a content key exists right after genesis");
    assert!(
        !roster.has_peers(),
        "FR-007: a device list naming only itself must not publish presence"
    );
}

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

/// A real meeting published to a real (in-process) relay, received through
/// `presence::run`'s own subscribe-and-handle loop: it lands in
/// `device_presence_no_sync` and the node's address book, both with a
/// device the receiver did not know how to reach a moment ago.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fresh_meeting_from_a_listed_device_is_recorded_and_looked_up() {
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let url = relay.url().await;

    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    // `Member::add` only updates the device list; a real device gains the
    // vault's content key by an actual generation being issued to it, which
    // genesis only did for `main` (the sole device at the time).
    main.device
        .db()
        .write(|tx| {
            let valid = crate::sync::device_list::valid_lists(
                &crate::sync::device_list::load_all(tx)?,
                &main.vault,
            );
            let effective = crate::sync::device_list::effective(&valid)
                .expect("a list")
                .clone();
            let key = crate::sync::content_keys::ContentKey::generate(2);
            crate::sync::content_keys::issue_generation(tx, &key, &effective, &main.keys, 10)
        })
        .expect("issue generation 2");
    linked.device.pull_from(&main.device);
    linked
        .device
        .db()
        .write(|tx| {
            let valid = crate::sync::device_list::valid_lists(
                &crate::sync::device_list::load_all(tx)?,
                &linked.vault,
            );
            crate::sync::content_keys::unwrap_own_envelopes(tx, &linked.keys, &valid)?;
            Ok(())
        })
        .expect("unwrap linked's envelope");

    let main_node = bind_loopback(&main).await;
    let main_addr = main_node.addr();

    // `run` subscribes and only then waits; start it in the background
    // first, so a later publish (the events are ephemeral, kind 21059 —
    // a real relay never redelivers one to a subscription opened after
    // the fact) has a live subscription to actually land on.
    let linked_node = Arc::new(bind_loopback(&linked).await);
    let reconnect = Arc::new(tokio::sync::Notify::new());
    let receiver = {
        let linked_node = Arc::clone(&linked_node);
        let replica = Arc::clone(&linked.device.replica);
        let keys = linked.keys.clone();
        let vault = linked.vault;
        let relay_urls = vec![url.to_string()];
        let reconnect = Arc::clone(&reconnect);
        tokio::spawn(async move {
            // The sender stays alive for the whole run: a dropped one closes
            // the channel, and `run` ends on a closed `changed`.
            let (_changed_tx, changed_rx) = tokio::sync::watch::channel(0u64);
            let _ = tokio::time::timeout(
                Duration::from_secs(10),
                run(
                    &linked_node,
                    &replica,
                    &keys,
                    vault,
                    relay_urls,
                    changed_rx,
                    &reconnect,
                ),
            )
            .await;
        })
    };

    let publisher = nostr_sdk::client::Client::new();
    publisher.add_relay(url.as_str()).await.expect("add relay");
    publisher
        .try_connect_relay(url.clone(), Duration::from_secs(3))
        .await
        .expect("connect");

    let content_key = crate::storage::query::read(main.device.db(), |r| {
        crate::sync::content_keys::current_key(r, &[])
    })
    .expect("read")
    .expect("a content key");

    // The first meeting can still be lost to the subscribe/publish race
    // (there is no explicit "subscribed" signal to wait on), so republish
    // on a short poll until it lands or the receiver's own budget expires.
    let recorded = tokio::time::timeout(Duration::from_secs(9), async {
        loop {
            let day = day_tag_now();
            let (_, mb_pk) = mailbox_keys(&content_key.key, day).expect("mailbox keys");
            let content = PresenceContent::own(
                main.keys.device_pubkey,
                main.keys.endpoint_id,
                main_addr.relay_urls().next().map(RelayUrl::to_string),
                main_addr.ip_addrs().copied().collect(),
                2,
            );
            let event = build(&main.keys, &content, &mb_pk).expect("build");
            let _ = publisher.send_event(&event).await;

            tokio::time::sleep(Duration::from_millis(200)).await;
            let rows = crate::storage::query::read(linked.device.db(), |r| load_all(r))
                .expect("read presence");
            if let Some(row) = rows
                .into_iter()
                .find(|row| row.device_pubkey == main.keys.device_pubkey)
            {
                return row;
            }
        }
    })
    .await
    .expect("main's presence was recorded before the budget ran out");

    assert!(recorded.endpoint_addr.is_some());
    // A fresh, authenticated meeting also nudges reconnect right away
    // (T039): recording presence and never dialing would leave two
    // devices that just found each other waiting out a whole idle tick.
    // Presence only wakes the reconnect loop (`SyncService` runs it); here
    // the test stands in for that loop.
    tokio::time::timeout(Duration::from_secs(5), reconnect.notified())
        .await
        .expect("presence recording a fresh meeting wakes reconnect");
    crate::sync::reconnect_missing(&linked_node, &linked.device.replica).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while linked_node.connected().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("reconnect dials the device presence just recorded");
    assert_eq!(linked_node.connected(), vec![main.keys.device_pubkey]);
    receiver.abort();
}

/// The relay hands a device its own meetings back (it subscribes to the
/// mailbox it publishes to); recording those would list this device as its
/// own peer and have reconnect dial itself.
#[tokio::test]
async fn a_device_ignores_its_own_meeting() {
    let main = Member::genesis();
    let node = bind_loopback(&main).await;
    let content_key = crate::storage::query::read(main.device.db(), |r| {
        crate::sync::content_keys::current_key(r, &[])
    })
    .expect("read")
    .expect("a content key");
    let day = day_tag_now();
    let (_, mb_pk) = mailbox_keys(&content_key.key, day).expect("mailbox keys");
    let addr = node.addr();
    let content = PresenceContent::own(
        main.keys.device_pubkey,
        main.keys.endpoint_id,
        None,
        addr.ip_addrs().copied().collect(),
        1,
    );
    let event = build(&main.keys, &content, &mb_pk).expect("build");

    let recorded = handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;
    assert!(!recorded);
    let rows =
        crate::storage::query::read(main.device.db(), |r| load_all(r)).expect("read presence");
    assert!(rows.is_empty());
}

#[test]
fn a_crypto_provider_is_installed_for_the_relay_websocket() {
    ensure_crypto_provider();
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
}
