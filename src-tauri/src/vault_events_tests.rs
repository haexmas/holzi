use std::sync::{Arc, Mutex};

use super::*;
use crate::identity::VAULT_SCOPE_UUID;
use crate::storage::preferences::{self, PrefScope};
use crate::sync::test_support::Device;

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| (*n).to_string()).collect()
}

type Emitted = Arc<Mutex<Vec<Vec<String>>>>;

fn spawn_feed(
    token: CancellationToken,
) -> (
    tokio::sync::mpsc::UnboundedSender<BTreeSet<String>>,
    Emitted,
    tokio::task::JoinHandle<()>,
) {
    let (tx, rx) = unbounded_channel();
    let emitted: Emitted = Arc::default();
    let sink = Arc::clone(&emitted);
    let feed = tokio::spawn(run(rx, token, move |tables| {
        sink.lock().unwrap().push(tables)
    }));
    (tx, emitted, feed)
}

#[tokio::test(start_paused = true)]
async fn reports_inside_one_window_become_one_event_with_their_union() {
    let (tx, emitted, _feed) = spawn_feed(CancellationToken::new());

    tx.send(set(&["preferences"])).unwrap();
    tokio::time::sleep(WINDOW / 2).await;
    tx.send(set(&["chat_threads", "preferences"])).unwrap();
    tokio::time::sleep(WINDOW).await;

    assert_eq!(
        *emitted.lock().unwrap(),
        vec![vec!["chat_threads".to_string(), "preferences".to_string()]]
    );
}

#[tokio::test(start_paused = true)]
async fn nothing_is_sent_without_a_report() {
    let (_tx, emitted, _feed) = spawn_feed(CancellationToken::new());

    tokio::time::sleep(WINDOW * 10).await;

    assert!(emitted.lock().unwrap().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_steady_stream_still_yields_an_event_every_window() {
    let (tx, emitted, _feed) = spawn_feed(CancellationToken::new());

    for _ in 0..8 {
        tx.send(set(&["chat_messages"])).unwrap();
        tokio::time::sleep(WINDOW / 4).await;
    }
    tokio::time::sleep(WINDOW).await;

    assert!(
        emitted.lock().unwrap().len() >= 2,
        "the window must not restart on every report"
    );
}

#[tokio::test(start_paused = true)]
async fn the_feed_ends_when_the_session_is_cancelled() {
    let token = CancellationToken::new();
    let (tx, emitted, feed) = spawn_feed(token.clone());

    token.cancel();
    feed.await.unwrap();
    let _ = tx.send(set(&["preferences"]));

    assert!(emitted.lock().unwrap().is_empty());
}

#[tokio::test(start_paused = true)]
async fn the_last_window_is_sent_when_the_database_closes() {
    let (tx, emitted, feed) = spawn_feed(CancellationToken::new());

    tx.send(set(&["preferences"])).unwrap();
    drop(tx);
    feed.await.unwrap();

    assert_eq!(
        *emitted.lock().unwrap(),
        vec![vec!["preferences".to_string()]]
    );
}

const COLOR_SCHEME_KEY: &str = "appearance.color_scheme";

fn drain(reports: &mut UnboundedReceiver<BTreeSet<String>>) -> BTreeSet<String> {
    let mut all = BTreeSet::new();
    while let Ok(tables) = reports.try_recv() {
        all.extend(tables);
    }
    all
}

#[test]
fn a_local_write_is_reported() {
    let device = Device::new();
    let mut reports = observe(device.db());

    device
        .db()
        .write(|tx| preferences::insert_or_update(tx, PrefScope::Vault, COLOR_SCHEME_KEY, "dark"))
        .expect("write");

    assert!(drain(&mut reports).contains("preferences"));
}

#[test]
fn a_change_received_from_another_device_is_reported() {
    let sender = Device::new();
    let receiver = Device::new();
    sender
        .db()
        .write(|tx| preferences::insert_or_update(tx, PrefScope::Vault, COLOR_SCHEME_KEY, "dark"))
        .expect("write on the sender");
    let mut reports = observe(receiver.db());

    receiver.pull_from(&sender);

    assert!(
        drain(&mut reports).contains("preferences"),
        "the receiving device must learn that preferences changed"
    );
    let stored = receiver
        .db()
        .read(|conn| {
            conn.query_row(
                "SELECT value FROM preferences WHERE vault_device_uuid = ?1 AND key = ?2",
                [VAULT_SCOPE_UUID.to_string(), COLOR_SCHEME_KEY.to_string()],
                |row| row.get::<_, String>(0),
            )
            .map_err(haex_crdt::db::error::DatabaseError::from)
            .map_err(haex_crdt::Error::from)
        })
        .expect("read");
    assert_eq!(stored, "dark");
}

#[test]
fn a_failed_write_is_not_reported() {
    let device = Device::new();
    let mut reports = observe(device.db());

    let failed: haex_crdt::Result<()> = device.db().write(|tx| {
        preferences::insert_or_update(tx, PrefScope::Vault, COLOR_SCHEME_KEY, "dark")?;
        Err(haex_crdt::Error::consumer("abort"))
    });

    assert!(failed.is_err());
    assert!(drain(&mut reports).is_empty());
}

/// Spec 017 (T023): a Rust subscriber of the change broadcast learns of local writes and of changes
/// received from another device, through the same window as the frontend event.
#[tokio::test]
async fn a_subscriber_of_the_broadcast_receives_local_and_remote_commits() {
    let device = Device::new();
    let sender = Device::new();
    let changes = changes_channel();
    let subscriber = changes.subscribe();
    let token = CancellationToken::new();
    let publisher = changes.clone();
    let feed = tokio::spawn(run(observe(device.db()), token.clone(), move |tables| {
        publish(&publisher, &tables)
    }));
    let next = |subscriber: &tokio::sync::broadcast::Receiver<Arc<Vec<String>>>| {
        let mut subscriber = subscriber.resubscribe();
        async move {
            tokio::time::timeout(std::time::Duration::from_secs(5), subscriber.recv())
                .await
                .expect("an announcement within the window")
                .expect("the broadcast is open")
        }
    };

    let local = next(&subscriber);
    device
        .db()
        .write(|tx| preferences::insert_or_update(tx, PrefScope::Vault, COLOR_SCHEME_KEY, "dark"))
        .expect("local write");
    assert!(local.await.contains(&"preferences".to_string()));

    let remote = next(&subscriber);
    sender
        .db()
        .write(|tx| preferences::insert_or_update(tx, PrefScope::Vault, COLOR_SCHEME_KEY, "light"))
        .expect("write on the other device");
    device.pull_from(&sender);
    assert!(remote.await.contains(&"preferences".to_string()));

    token.cancel();
    feed.await.expect("feed ends");
}
