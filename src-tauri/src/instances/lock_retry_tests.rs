use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::retry_while_locked;
use crate::error::HolziError;

const WINDOW: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(100);

#[tokio::test]
async fn returns_the_value_once_the_error_clears() {
    tokio::time::pause();
    let attempts = Arc::new(AtomicUsize::new(0));
    let token = CancellationToken::new();
    let counted = attempts.clone();
    let result = retry_while_locked(&token, WINDOW, POLL, move || {
        let attempts = counted.clone();
        async move {
            let n = attempts.fetch_add(1, Ordering::SeqCst);
            if n < 3 {
                Err(HolziError::VaultAlreadyOpenElsewhere)
            } else {
                Ok("opened")
            }
        }
    })
    .await;

    assert_eq!(result.expect("clears within the window"), "opened");
    assert_eq!(attempts.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn returns_the_error_after_the_window() {
    tokio::time::pause();
    let attempts = Arc::new(AtomicUsize::new(0));
    let token = CancellationToken::new();
    let counted = attempts.clone();
    let result = retry_while_locked(&token, WINDOW, POLL, move || {
        let attempts = counted.clone();
        async move {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(HolziError::VaultAlreadyOpenElsewhere)
        }
    })
    .await;

    assert!(matches!(result, Err(HolziError::VaultAlreadyOpenElsewhere)));
    // The window is 3s at a 100ms cadence: the first attempt plus the retries fit in the window,
    // never more than one interval past the deadline (ponytail's own stated ceiling).
    let n = attempts.load(Ordering::SeqCst);
    assert!((30..=31).contains(&n), "attempts = {n}");
}

#[tokio::test]
async fn returns_vault_closed_if_the_gate_starts_closing_during_the_wait() {
    tokio::time::pause();
    let token = CancellationToken::new();
    let cancel_token = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        cancel_token.cancel();
    });

    let result = retry_while_locked(&token, WINDOW, POLL, || async {
        Err::<(), _>(HolziError::VaultAlreadyOpenElsewhere)
    })
    .await;

    assert!(matches!(result, Err(HolziError::VaultClosed)));
}

#[tokio::test]
async fn does_not_retry_any_other_error() {
    tokio::time::pause();
    let attempts = Arc::new(AtomicUsize::new(0));
    let token = CancellationToken::new();
    let counted = attempts.clone();
    let result = retry_while_locked(&token, WINDOW, POLL, move || {
        let attempts = counted.clone();
        async move {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(HolziError::WrongPassphrase)
        }
    })
    .await;

    assert!(matches!(result, Err(HolziError::WrongPassphrase)));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}
