//! Integration test for spec 024 User Story 1 (T029): two real app
//! instances — the actual `SyncService`/`SyncNode`/presence/reconnect
//! wiring, not the lower-level fixtures the `sync::*` unit tests use —
//! connect on their own and exchange a change, over a real in-process
//! Nostr relay for presence discovery.
//!
//! Linux only: storage is redirected through `XDG_DATA_HOME` (see
//! `common/sync_fixture.rs`'s module docs on why this is safe to do
//! sequentially in one process).
#![cfg(target_os = "linux")]

use std::sync::LazyLock;
use std::time::Duration;

use tokio::sync::Mutex;

#[path = "common/sync_fixture.rs"]
mod sync_fixture;

/// `XDG_DATA_HOME` is process-wide; tests that set it take turns (same
/// reasoning as `tests/vault_single_session.rs`).
static TURN: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Waits until `condition` holds, polling within a bound; no fixed sleep
/// decides the outcome. Rewrites `id`/`title` on `from` on every poll: a
/// single write can race the very first connection settling (iroh's own
/// address probing plus the Nostr relay handshake take a moment), which a
/// real, longer-lived session absorbs for free from later unrelated
/// commits — a test making only this one change has to redo it itself
/// instead of relying on one exactly-timed shot landing.
async fn eventually(
    from: &sync_fixture::Instance,
    id: &str,
    title: &str,
    condition: impl Fn() -> bool,
) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while !condition() {
            from.write_thread(id, title).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    })
    .await
    .expect("the condition holds within the bound");
}

/// US1 scenarios 1 and 2 (spec.md): two devices of the same vault find and
/// connect to each other without any manual action, and a change made on
/// one appears on the other.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_devices_connect_on_their_own_and_exchange_a_change() {
    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();

    let main = sync_fixture::create_main("main", &relay_url).await;
    let linked = sync_fixture::join(&main, &relay_url).await;

    eventually(&main, "t1", "from main", || {
        linked.thread_title("t1").as_deref() == Some("from main")
    })
    .await;

    eventually(&linked, "t2", "from linked", || {
        main.thread_title("t2").as_deref() == Some("from linked")
    })
    .await;
}

/// US2 scenario 1 and SC-005 (spec.md): a device of another vault, running
/// the same sync service against the same Nostr relay, neither finds nor
/// receives anything of this vault, and this vault receives nothing of it.
/// (The refusals a stranger that does dial gets — foreign vault, not on the
/// list, bad signature — are `sync::handshake` unit tests, where the
/// handshake is exercised message by message.)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_device_of_another_vault_neither_gets_nor_gives_anything() {
    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();

    let main = sync_fixture::create_main("mine", &relay_url).await;
    let linked = sync_fixture::join(&main, &relay_url).await;
    let foreign = sync_fixture::create_main("theirs", &relay_url).await;
    foreign.write_thread("theirs", "not for you").await;

    // The two own devices sync (the positive control that the sync had
    // time to happen), and by then the foreign device has nothing.
    eventually(&main, "secret", "mine only", || {
        linked.thread_title("secret").as_deref() == Some("mine only")
    })
    .await;
    assert_eq!(foreign.thread_title("secret"), None);
    assert_eq!(main.thread_title("theirs"), None);
    assert_eq!(linked.thread_title("theirs"), None);
}

/// US3 scenarios 1 to 3 (spec.md): three devices end with the same data, no
/// change is lost or doubled, and each change still names the device it
/// came from, not the one it came through.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_devices_converge_and_each_change_keeps_its_author() {
    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();

    let a = sync_fixture::create_main("main", &relay_url).await;
    let b = sync_fixture::join(&a, &relay_url).await;
    let c = sync_fixture::join(&a, &relay_url).await;
    let devices = [&a, &b, &c];
    let threads = ["from-a", "from-b", "from-c"];

    // Each device rewrites its own thread until all three have all threads
    // (the same settle race `eventually` documents).
    tokio::time::timeout(Duration::from_secs(40), async {
        loop {
            for (device, id) in devices.iter().zip(threads) {
                device.write_thread(id, id).await;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
            let converged = devices
                .iter()
                .all(|d| threads.iter().all(|id| d.thread_title(id).is_some()));
            if converged {
                break;
            }
        }
    })
    .await
    .expect("all three devices hold all three threads");

    for device in devices {
        for (writer, id) in devices.iter().zip(threads) {
            assert_eq!(
                device.thread_origin(id),
                Some(writer.device_uuid()),
                "{id} names the device that wrote it"
            );
        }
    }
}
