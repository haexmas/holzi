use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use iroh::RelayMode;

use super::*;
use crate::sync::endpoint::{NodeConfig, SyncNode};
use crate::sync::keys::DeviceKeys;
use crate::sync::presence::{day_tag_now, load_all};
use crate::sync::test_support::Member;

#[test]
fn a_one_device_vault_has_no_peers_to_publish_to() {
    let solo = Member::genesis();
    let roster = read_roster(&solo.device.replica, solo.vault, &solo.keys.device_pubkey)
        .expect("read roster")
        .expect("a content key exists right after genesis");
    assert!(
        !roster.has_peers(),
        "FR-007: a device list naming only itself must not publish presence"
    );
}

/// Two lists of the same generation: the effective one removes a device that
/// holds no key, the one that lost the tie-break removes the linked device the
/// newest key is wrapped for (as when two main devices removed each other).
/// Only the effective list's removals count (FR-043), so presence keeps the
/// newest key instead of an older one or none.
#[test]
fn a_removal_in_a_list_that_lost_the_tie_break_keeps_the_newest_key() {
    use crate::sync::content_keys::{issue_generation, ContentKey};
    use crate::sync::device_list::{self, RemovedDevice};
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    let newest = ContentKey::generate(2);
    main.device
        .db()
        .write(|tx| {
            let secret = crate::sync::keys::vault_secret(tx)?.expect("a main device");
            let valid = device_list::valid_lists(&device_list::load_all(tx)?, &main.vault);
            let base = device_list::effective(&valid).expect("a list").clone();
            issue_generation(tx, &newest, &base, &main.keys, 2)?;
            let removing = |device_pubkey, issued_at| {
                let mut list = device_list::DeviceList {
                    generation: base.list.generation + 1,
                    base_list_hash: Some(base.hash),
                    removed: vec![RemovedDevice {
                        device_pubkey,
                        vault_device_uuid: uuid::Uuid::new_v4(),
                        limit_hlc: String::new(),
                        removed_at: 1,
                    }],
                    issued_at,
                    ..base.list.clone()
                };
                list.devices.retain(|d| d.device_pubkey != device_pubkey);
                device_list::sign_list(list, &secret).map_err(haex_crdt::Error::consumer)
            };
            let winner = removing([7; 32], 1)?;
            let loser = (2..)
                .map(|issued_at| removing(linked.keys.device_pubkey, issued_at))
                .find(|signed| signed.as_ref().map_or(true, |s| s.hash > winner.hash))
                .expect("a losing list")?;
            device_list::insert(tx, &winner)?;
            device_list::insert(tx, &loser)
        })
        .expect("forked lists");

    let roster = read_roster(&main.device.replica, main.vault, &main.keys.device_pubkey)
        .expect("read roster")
        .expect("a content key");
    assert_eq!(roster.content_key, *newest.key);
    assert!(
        roster.has_peers(),
        "the effective list still names the linked device"
    );
}

/// Binds `member`'s endpoint on loopback with no iroh-Relay.
pub(super) async fn bind_loopback(member: &Member) -> SyncNode {
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
    crate::sync::reconnect_missing(&linked_node, &linked.device.replica);
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

/// A meeting `signer` published for `endpoint`, sealed for `day`'s mailbox
/// of `receiver`'s content key.
fn meeting(receiver: &Member, signer: &DeviceKeys, endpoint: [u8; 32], day: u32) -> Event {
    let content_key = crate::storage::query::read(receiver.device.db(), |r| {
        crate::sync::content_keys::current_key(r, &[])
    })
    .expect("read")
    .expect("a content key");
    let (_, mb_pk) = mailbox_keys(&content_key.key, day).expect("mailbox keys");
    let content = PresenceContent::own(
        signer.device_pubkey,
        endpoint,
        None,
        vec![(Ipv4Addr::LOCALHOST, 4000).into()],
        2,
    );
    build(signer, &content, &mb_pk).expect("build")
}

fn problem_of(member: &Member, device: &[u8; 32]) -> Option<crate::sync::problems::Problem> {
    crate::storage::query::read(member.device.db(), |r| crate::sync::problems::of(r, device))
        .expect("read problem")
}

/// R14, FR-030: a listed device key speaking from an endpoint the list does
/// not name means two installations act as one device.
#[tokio::test]
async fn a_listed_key_meeting_from_another_endpoint_halts_that_device() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let event = meeting(&main, &linked.keys, [5; 32], day);

    let recorded = handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;

    assert!(!recorded, "a duplicate is never dialed");
    assert_eq!(
        problem_of(&main, &linked.keys.device_pubkey),
        Some(crate::sync::problems::Problem::Duplicate)
    );
    let rows = crate::storage::query::read(main.device.db(), |r| load_all(r)).expect("read");
    assert!(
        rows.iter().all(|row| row.endpoint_addr.is_none()),
        "no address of a duplicate is recorded"
    );
}

/// A copy of this very installation on another endpoint shows up in this
/// device's own mailbox: this device notes that it is duplicated.
#[tokio::test]
async fn a_meeting_with_this_devices_key_from_another_endpoint_marks_it_duplicated() {
    let main = Member::genesis();
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let event = meeting(&main, &main.keys, [6; 32], day);

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
    assert_eq!(
        problem_of(&main, &main.keys.device_pubkey),
        Some(crate::sync::problems::Problem::Duplicate)
    );
}

/// A fresh meeting from the endpoint the list names is no duplicate.
#[tokio::test]
async fn a_meeting_from_the_listed_endpoint_records_no_problem() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let event = meeting(&main, &linked.keys, linked.keys.endpoint_id, day);

    let recorded = handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;

    assert!(recorded);
    assert_eq!(problem_of(&main, &linked.keys.device_pubkey), None);
}

/// A new content key generation for `main`, issued for the list it has now (a device was removed or
/// added since the last one).
fn rotate(main: &Member) -> crate::sync::content_keys::ContentKey {
    let old_key = crate::storage::query::read(main.device.db(), |r| {
        crate::sync::content_keys::current_key(r, &[])
    })
    .expect("read")
    .expect("a content key");
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
            let generation = crate::sync::envelopes::held_keys(tx)?.len() as u64 + 1;
            let key = crate::sync::content_keys::ContentKey::generate(generation);
            crate::sync::content_keys::issue_generation(tx, &key, &effective, &main.keys, 10)
        })
        .expect("issue the next generation");
    old_key
}

/// A device that was away while the content key was replaced still speaks into the mailbox of the old
/// key, the only one it knows. A device that holds both keys hears it there and can dial it; only
/// then can the device sync and learn the new key.
#[tokio::test]
async fn a_device_on_an_older_content_key_is_heard_and_dialed() {
    let main = Member::genesis();
    let stale = Member::join(&main);
    main.add(&stale);
    let old_key = rotate(&main);

    let roster = read_roster(&main.device.replica, main.vault, &main.keys.device_pubkey)
        .expect("read roster")
        .expect("a roster");
    assert_eq!(
        roster.listening_keys().len(),
        2,
        "both generations are held"
    );
    assert_ne!(
        roster.listening_keys()[0],
        *old_key.key,
        "the current key comes first"
    );
    assert!(roster.listening_keys().contains(&*old_key.key));

    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let (_, old_mailbox) = mailbox_keys(&old_key.key, day).expect("mailbox keys");
    let content = PresenceContent::own(
        stale.keys.device_pubkey,
        stale.keys.endpoint_id,
        None,
        vec![(Ipv4Addr::LOCALHOST, 4000).into()],
        2,
    );
    let event = build(&stale.keys, &content, &old_mailbox).expect("build");

    let recorded = handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;

    assert!(
        recorded,
        "a meeting at the mailbox of an older held key is dialed"
    );
}

/// Only the current key and the configured number of neighboring generations are listened to.
#[test]
fn only_the_nearest_held_keys_are_listened_to() {
    let main = Member::genesis();
    for _ in 0..(OTHER_MAILBOXES + 2) {
        rotate(&main);
    }
    let roster = read_roster(&main.device.replica, main.vault, &main.keys.device_pubkey)
        .expect("read roster")
        .expect("a roster");
    assert_eq!(roster.listening_keys().len(), OTHER_MAILBOXES + 1);
    assert_eq!(roster.listening_keys()[0], roster.content_key);
}

/// A removed or unknown device cannot use a retained older mailbox to make a
/// fresh presence claim that would be dialed.
#[tokio::test]
async fn a_stranger_in_an_older_mailbox_is_not_dialed() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    let old_key = rotate(&main);
    let stranger = Member::genesis();
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let (_, old_mailbox) = mailbox_keys(&old_key.key, day).expect("mailbox keys");
    let content = PresenceContent::own(
        stranger.keys.device_pubkey,
        stranger.keys.endpoint_id,
        None,
        vec![(Ipv4Addr::LOCALHOST, 4000).into()],
        99,
    );
    let event = build(&stranger.keys, &content, &old_mailbox).expect("build");

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

/// A request from an unknown device in an older mailbox is not admitted into
/// the synchronized request list.
#[tokio::test]
async fn a_request_in_an_older_mailbox_is_not_kept() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    let old_key = rotate(&main);
    let asking = Member::genesis();
    let node = bind_loopback(&main).await;
    let day = day_tag_now();
    let (_, old_mailbox) = mailbox_keys(&old_key.key, day).expect("mailbox keys");
    let request = admission::Request::sign(
        &asking.keys,
        asking.device.db().device_id(),
        "Zweitrechner".to_string(),
        now_ms(),
    )
    .expect("sign");
    let addr = iroh::EndpointAddr::from_parts(
        iroh::EndpointId::from_bytes(&asking.keys.endpoint_id).expect("endpoint"),
        [iroh::TransportAddr::Ip((Ipv4Addr::LOCALHOST, 4000).into())],
    );
    let json = serde_json::to_string(&admission::Content::new(&request, &addr)).expect("encode");
    let event = wrap_payload(
        ADMISSION_KIND,
        &asking.keys.device_secret,
        &json,
        &old_mailbox,
    )
    .expect("wrap");

    handle_incoming(
        &node,
        &main.device.replica,
        &main.keys,
        main.vault,
        day,
        &event,
    )
    .await;

    let open = crate::storage::query::read(main.device.db(), |r| {
        admission::load_open(r, &HashSet::new())
    })
    .expect("read requests");
    assert!(open.is_empty());
}
