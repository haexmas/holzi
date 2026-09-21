//! Integration coverage for the public, app-independent model-load lifecycle.
//!
//! The actual model resolver needs a live Tauri `AppHandle` and filesystem
//! model catalog, so the desktop scenarios remain in the quickstart. These
//! tests exercise the cancellation boundary and generation hand-off that the
//! resolver relies on.

use holzi_lib::chat::session::{ChatState, ModelLoadStatus};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn cancelling_a_preload_waits_for_its_task_to_terminate() {
    let chat = ChatState::new();
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let join = tokio::spawn(async move {
        task_cancel.cancelled().await;
    });
    chat.install_preload_handle(cancel.clone(), join);

    chat.cancel_preload_and_wait().await;

    assert!(cancel.is_cancelled());
    assert!(matches!(
        chat.model_load_status(),
        ModelLoadStatus::Idle {
            vault_generation: 0
        }
    ));
}

#[test]
fn a_vault_transition_resets_status_and_rejects_the_previous_load() {
    let chat = ChatState::new();
    let (old_generation, old_load) = chat.begin_model_load();
    assert!(chat.set_model_loading(
        old_generation,
        old_load,
        "old-model".into(),
        "Old model".into(),
        "loading".into(),
        None,
    ));

    let new_generation = chat.bump_vault_generation();

    assert!(matches!(
        chat.model_load_status(),
        ModelLoadStatus::Idle { vault_generation } if vault_generation == new_generation
    ));
    assert!(!chat.set_model_ready(
        old_generation,
        old_load,
        "old-model".into(),
        "Old model".into(),
    ));
}
