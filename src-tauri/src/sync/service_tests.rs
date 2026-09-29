use std::time::Duration;

use super::*;

/// Waits until `condition` holds, polling within a bound; no fixed sleep
/// decides the outcome.
async fn eventually(condition: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the condition holds within the bound");
}

#[tokio::test]
async fn a_commit_wakes_the_service_and_the_close_ends_it() {
    let gate = VaultGate::with_runtime(tokio::runtime::Handle::current());
    gate.begin_session().expect("session");
    let service = SyncService::start(&gate).expect("start");

    gate.sync_notify().notify_one();
    eventually(|| service.rounds() >= 1).await;

    gate.request_close();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !gate.is_idle() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the service ends once the close starts");
}

#[tokio::test]
async fn a_closing_gate_starts_no_service() {
    let gate = VaultGate::with_runtime(tokio::runtime::Handle::current());
    gate.request_close();
    assert!(SyncService::start(&gate).is_err());
}
