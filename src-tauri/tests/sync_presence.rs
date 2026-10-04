//! Integration test for spec 024 presence (T043): the Nostr traffic of real
//! app instances.
//!
//! Linux only, like `sync_devices.rs` (storage is redirected through
//! `XDG_DATA_HOME`).
#![cfg(target_os = "linux")]

use std::time::Duration;

use futures::StreamExt;
use nostr::event::Kind;
use nostr::filter::Filter;
use nostr_sdk::client::{Client, ClientNotification};

#[path = "common/sync_fixture.rs"]
mod sync_fixture;

/// The event kind of a presence meeting (contracts/nostr-events.md).
const PRESENCE_KIND: u16 = 21059;

/// SC-007 and FR-007: a vault with one device publishes nothing for as long
/// as it stays alone, and starts to publish once a second device is listed.
///
/// An observer subscribes to every presence-kind event on the relay before
/// the first device starts. The quiet window is time-bound (a negative can
/// only be shown that way); the second half is the positive control that
/// the observer would have seen a meeting, and it waits on the event, not a
/// clock.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_one_device_vault_publishes_no_presence_until_it_has_a_second_device() {
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await;

    let observer = Client::new();
    observer
        .add_relay(relay_url.as_str())
        .await
        .expect("add relay");
    observer
        .try_connect_relay(relay_url.clone(), Duration::from_secs(3))
        .await
        .expect("connect");
    let mut notifications = observer.notifications();
    observer
        .subscribe(Filter::new().kind(Kind::Custom(PRESENCE_KIND)))
        .await
        .expect("subscribe");

    let main = sync_fixture::create_main("alone", relay_url.as_str()).await;

    // Presence publishes on its first tick, right after the service starts;
    // a few seconds cover that and a couple of commit wake-ups.
    let quiet = tokio::time::timeout(Duration::from_secs(4), async {
        while let Some(notification) = notifications.next().await {
            if matches!(notification, ClientNotification::Event { .. }) {
                return true;
            }
        }
        false
    })
    .await;
    assert!(quiet.is_err(), "a vault with one device published presence");

    let linked = sync_fixture::join(&main, relay_url.as_str()).await;
    let seen = tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(notification) = notifications.next().await {
            if matches!(notification, ClientNotification::Event { .. }) {
                return true;
            }
        }
        false
    })
    .await
    .expect("presence appears once the vault has a second device");
    assert!(seen);

    tokio::join!(main.shutdown(), linked.shutdown());
}
