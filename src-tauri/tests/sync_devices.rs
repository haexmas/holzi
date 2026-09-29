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
