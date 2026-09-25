//! Tests for `wm_session_commands`'s core logic (spec 022-session-restore,
//! contracts/wm-session.md §1). Declared as a child module (`#[path]` in that
//! file) so it can call the private `async fn`s directly. A real `VaultDb`
//! comes from `AppState::install`, the same route a live command takes.

use std::sync::Arc;

use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::identity::{holzi_migration_source, HolziBootstrap, HOLZI_TRIGGER_VERSION};
use crate::state::{ActiveInstanceHandle, AppState};

fn open_test_state(passphrase: &str) -> (tempfile::TempDir, AppState, Uuid) {
    let dir = tempfile::tempdir().expect("tempdir");
    let install_path = installation_id_path(dir.path());
    let db = Arc::new(
        Database::open(DatabaseConfig {
            path: dir.path().join("vault.db"),
            key: SqlCipherKey::new(passphrase),
            create_if_missing: true,
            bootstrap: Arc::new(HolziBootstrap::new(install_path.clone())),
            signature_provider: Arc::new(NoopSignatureProvider),
            migration_source: holzi_migration_source(),
            trigger_version: HOLZI_TRIGGER_VERSION,
        })
        .expect("open test vault"),
    );
    let installation_uuid =
        read_or_mint_installation_uuid(&install_path).expect("installation uuid");
    let device = db
        .with_connection(|conn| {
            Ok(known_devices::get_vault_device_uuid(
                conn,
                installation_uuid,
            )?)
        })
        .expect("read vault_device_uuid")
        .expect("bootstrap registered this installation");

    let state = AppState::default();
    state
        .install(
            ActiveInstanceHandle {
                name: "wm-session-commands-test".to_string(),
                database: Arc::clone(&db),
            },
            || Ok(()),
        )
        .expect("install the test vault");
    (dir, state, device)
}

fn db_of(state: &AppState) -> VaultDb {
    state.database().expect("active vault")
}

fn snapshot(marker: &str) -> serde_json::Value {
    json!({ "version": 1, "workspaces": [], "windows": [], "activeWorkspaceId": marker })
}

fn set_args(scope: SessionRestoreScope, enabled: Option<bool>) -> SessionRestoreSetArgs {
    SessionRestoreSetArgs { scope, enabled }
}

async fn stored_session(state: &AppState, device: Uuid) -> Option<serde_json::Value> {
    let db = db_of(state);
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| Ok(wm_session::load(conn, device).expect("load")))
            .expect("load")
    })
    .await
    .expect("join")
}

#[tokio::test]
async fn nothing_is_set_on_a_fresh_vault_and_the_default_is_off() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-default");
    let restore = restore_get(db_of(&state), device).await.expect("get");
    assert_eq!(
        restore,
        SessionRestoreState {
            device: None,
            vault: None,
            effective: false,
        }
    );
}

#[tokio::test]
async fn setting_the_device_value_turns_restore_on_and_null_resets_it() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-device");
    let on = restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, Some(true)),
    )
    .await
    .expect("set on");
    assert_eq!(on.device, Some(true));
    assert!(on.effective);

    let reset = restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, None),
    )
    .await
    .expect("reset");
    assert_eq!(reset.device, None);
    assert!(!reset.effective);
    assert_eq!(
        restore_get(db_of(&state), device).await.expect("get"),
        reset
    );
}

#[tokio::test]
async fn turning_restore_off_deletes_the_saved_session_in_the_same_call() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-off");
    restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, Some(true)),
    )
    .await
    .expect("set on");
    let saved = session_save(db_of(&state), device, snapshot("a"))
        .await
        .expect("save");
    assert!(saved.saved);
    assert!(stored_session(&state, device).await.is_some());

    let off = restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, Some(false)),
    )
    .await
    .expect("set off");
    assert!(!off.effective);
    assert_eq!(stored_session(&state, device).await, None);
}

#[tokio::test]
async fn load_with_restore_off_deletes_a_leftover_session() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-load-off");
    let db = db_of(&state);
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            wm_session::save(conn, device, &snapshot("leftover")).expect("save");
            Ok(())
        })
        .expect("save leftover")
    })
    .await
    .expect("join");

    let loaded = session_load(db_of(&state), device).await.expect("load");
    assert!(!loaded.restore.effective);
    assert_eq!(loaded.session, None);
    assert_eq!(stored_session(&state, device).await, None);
}

#[tokio::test]
async fn load_with_restore_on_returns_the_saved_session() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-load-on");
    restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, Some(true)),
    )
    .await
    .expect("set on");
    session_save(db_of(&state), device, snapshot("kept"))
        .await
        .expect("save");

    let loaded = session_load(db_of(&state), device).await.expect("load");
    assert!(loaded.restore.effective);
    assert_eq!(loaded.session, Some(snapshot("kept")));
}

#[tokio::test]
async fn save_with_restore_off_writes_nothing() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-save-off");
    let saved = session_save(db_of(&state), device, snapshot("late"))
        .await
        .expect("save");
    assert!(!saved.saved);
    assert_eq!(stored_session(&state, device).await, None);
}

#[tokio::test]
async fn an_oversized_session_is_rejected_as_too_large() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-too-large");
    restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, Some(true)),
    )
    .await
    .expect("set on");
    let big = json!({ "version": 1, "padding": "x".repeat(wm_session::MAX_SESSION_BYTES) });
    let result = session_save(db_of(&state), device, big).await;
    assert!(matches!(result, Err(HolziError::SessionTooLarge { .. })));
}

// Vault scope (US3, T028).

#[tokio::test]
async fn the_vault_value_applies_while_the_device_value_is_unset() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-vault-on");
    let restore = restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Vault, Some(true)),
    )
    .await
    .expect("set vault on");
    assert_eq!(restore.vault, Some(true));
    assert_eq!(restore.device, None);
    assert!(restore.effective);
}

#[tokio::test]
async fn a_device_off_overrides_vault_on_and_deletes_the_own_session() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-device-off");
    restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Vault, Some(true)),
    )
    .await
    .expect("set vault on");
    session_save(db_of(&state), device, snapshot("mine"))
        .await
        .expect("save");

    let restore = restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, Some(false)),
    )
    .await
    .expect("set device off");
    assert!(!restore.effective);
    assert_eq!(stored_session(&state, device).await, None);
}

#[tokio::test]
async fn a_device_on_applies_even_with_vault_off() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-device-on");
    restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Vault, Some(false)),
    )
    .await
    .expect("set vault off");
    let restore = restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, Some(true)),
    )
    .await
    .expect("set device on");
    assert!(restore.effective);
}

#[tokio::test]
async fn resetting_the_device_value_falls_back_to_the_vault_value() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-reset-fallback");
    restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Vault, Some(true)),
    )
    .await
    .expect("set vault on");
    restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, Some(false)),
    )
    .await
    .expect("set device off");
    let restore = restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Device, None),
    )
    .await
    .expect("reset device");
    assert_eq!(restore.device, None);
    assert!(restore.effective, "the vault value applies again");
}

#[tokio::test]
async fn two_devices_with_restore_on_keep_separate_sessions() {
    let (_dir, state, device) = open_test_state("wm-session-cmd-two-devices");
    // The second device has no device value, so the vault value applies to it too.
    let other = Uuid::new_v4();
    restore_set(
        db_of(&state),
        device,
        set_args(SessionRestoreScope::Vault, Some(true)),
    )
    .await
    .expect("set vault on");
    session_save(db_of(&state), device, snapshot("first"))
        .await
        .expect("save first");
    session_save(db_of(&state), other, snapshot("second"))
        .await
        .expect("save second");

    let first = session_load(db_of(&state), device)
        .await
        .expect("load first");
    let second = session_load(db_of(&state), other)
        .await
        .expect("load second");
    assert_eq!(first.session, Some(snapshot("first")));
    assert_eq!(second.session, Some(snapshot("second")));
}
