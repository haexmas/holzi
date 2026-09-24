use std::sync::LazyLock;
use std::time::Duration;

use tauri::test::{mock_builder, mock_context, noop_assets};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::acquire_model_publication_lock;

/// `XDG_DATA_HOME` is process-wide, so these two tests (the only ones in this crate that redirect
/// it from within `src/`'s unit-test binary) take turns, the same way
/// `tests/vault_single_session.rs`'s own `TURN` does for the separate integration-test binary.
static TURN: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn mock_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
    mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app")
        .handle()
        .clone()
}

/// T064: a second acquisition of the same slug waits while the first lock is held, succeeds after
/// it drops, and returns `VaultClosed` when the cancellation token fires while waiting.
#[tokio::test]
async fn a_second_acquisition_waits_then_succeeds_once_the_first_drops() {
    let _turn = TURN.lock().await;
    let data_home = tempfile::tempdir().expect("data home");
    std::env::set_var("XDG_DATA_HOME", data_home.path());
    let app = mock_app();
    let token = CancellationToken::new();

    let first = acquire_model_publication_lock(&app, "slug-a", &token)
        .await
        .expect("first acquisition succeeds at once");

    let waiting_app = app.clone();
    let waiting_token = token.clone();
    let waiting = tokio::spawn(async move {
        acquire_model_publication_lock(&waiting_app, "slug-a", &waiting_token).await
    });

    // Give the waiting task a real chance to actually block on the file lock before releasing the
    // first — otherwise a lucky scheduling order could let it acquire before ever contending.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !waiting.is_finished(),
        "the second acquisition must still be waiting for the first"
    );

    drop(first);

    let second = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .expect("the wait ends soon after the first lock drops")
        .expect("task did not panic")
        .expect("second acquisition succeeds once the first releases");
    drop(second);

    std::env::remove_var("XDG_DATA_HOME");
}

#[tokio::test]
async fn a_waiting_acquisition_returns_vault_closed_once_the_token_fires() {
    let _turn = TURN.lock().await;
    let data_home = tempfile::tempdir().expect("data home");
    std::env::set_var("XDG_DATA_HOME", data_home.path());
    let app = mock_app();
    let token = CancellationToken::new();

    let first = acquire_model_publication_lock(&app, "slug-b", &token)
        .await
        .expect("first acquisition succeeds at once");

    let waiting_app = app.clone();
    let waiting_token = token.clone();
    let waiting = tokio::spawn(async move {
        acquire_model_publication_lock(&waiting_app, "slug-b", &waiting_token).await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    token.cancel();

    let result = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .expect("the wait ends soon after the token fires")
        .expect("task did not panic");
    assert!(matches!(result, Err(crate::error::HolziError::VaultClosed)));

    drop(first);
    std::env::remove_var("XDG_DATA_HOME");
}
