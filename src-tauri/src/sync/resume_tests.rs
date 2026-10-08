use std::time::Duration;

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::*;

/// Whether `notify` holds a wake, without waiting for one to come.
async fn woken(notify: &Notify) -> bool {
    tokio::time::timeout(Duration::from_millis(20), notify.notified())
        .await
        .is_ok()
}

#[tokio::test]
async fn resume_wakes_reconnect_and_relays_once() {
    let wakeups = Wakeups::new(CancellationToken::new());

    assert!(wakeups.wake(Wake::Resumed));
    // A second resume before the loops ran collapses into the same pass.
    assert!(wakeups.wake(Wake::Resumed));

    assert!(woken(&wakeups.reconnect).await);
    assert!(
        !woken(&wakeups.reconnect).await,
        "one pass per burst of wakes"
    );
    assert!(woken(&wakeups.relays).await);
    assert!(!woken(&wakeups.relays).await);
    assert!(
        !woken(&wakeups.network).await,
        "coming back to the foreground is no network change"
    );
}

#[tokio::test]
async fn network_change_also_tells_iroh() {
    let wakeups = Wakeups::new(CancellationToken::new());

    assert!(wakeups.wake(Wake::NetworkChanged));

    assert!(woken(&wakeups.network).await);
    assert!(woken(&wakeups.relays).await);
    assert!(woken(&wakeups.reconnect).await);
}

#[tokio::test]
async fn nothing_wakes_while_the_vault_closes() {
    let cancel = CancellationToken::new();
    let wakeups = Wakeups::new(cancel.clone());
    cancel.cancel();

    assert!(!wakeups.wake(Wake::Resumed));
    assert!(!wakeups.wake(Wake::NetworkChanged));

    assert!(!woken(&wakeups.reconnect).await);
    assert!(!woken(&wakeups.relays).await);
    assert!(!woken(&wakeups.network).await);
}

#[test]
fn without_an_open_vault_a_wake_does_nothing() {
    let registry = SyncRegistry::default();

    assert!(!wake_registry(&registry, Wake::Resumed));
    assert!(!wake_registry(&registry, Wake::NetworkChanged));
}
