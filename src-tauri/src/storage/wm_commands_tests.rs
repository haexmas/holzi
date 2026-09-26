//! Tests for `wm_commands`'s core logic (spec 015-workspace-shell).
//! Declared as a child module of `wm_commands` itself (`#[path = ...]`
//! in that file), not a `storage::` sibling, so it can reach the private
//! `async fn`s directly — mirroring `models::commands`'s own
//! `commands_tests.rs`. A real `VaultDb` comes from `AppState::install`,
//! the same route a live command takes, rather than from a raw
//! `Database::open` — this exercises the exact handle type the `#[tauri::
//! command]` shims pass down.

use std::sync::Arc;

use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
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
                name: "wm-commands-test".to_string(),
                database: Arc::clone(&db),
            },
            || Ok(()),
        )
        .expect("install the test vault");

    (dir, state, device)
}

fn row(position: i64) -> WorkspaceRow {
    WorkspaceRow {
        workspace_id: Uuid::new_v4(),
        position,
    }
}

#[test]
fn picks_the_previous_neighbor_when_the_deleted_workspace_was_not_first() {
    let remaining = vec![row(0), row(1)];
    let expected = remaining[0].workspace_id;
    assert_eq!(neighbor_after_delete(1, &remaining), Some(expected));
}

#[test]
fn picks_the_next_neighbor_shifted_into_position_zero_when_the_deleted_workspace_was_first() {
    let remaining = vec![row(0), row(1)];
    let expected = remaining[0].workspace_id;
    assert_eq!(neighbor_after_delete(0, &remaining), Some(expected));
}

#[test]
fn returns_none_when_nothing_remains() {
    let remaining: Vec<WorkspaceRow> = vec![];
    assert_eq!(neighbor_after_delete(0, &remaining), None);
}

#[tokio::test]
async fn load_layout_creates_a_default_workspace_and_persists_it_as_active() {
    let (_dir, state, device) = open_test_state("wm-commands-load-default");
    let db = state.database().expect("active");

    let layout = load_layout(db.clone(), device).await.expect("load layout");
    assert_eq!(layout.workspaces.len(), 1);
    assert_eq!(
        layout.active_workspace_id,
        layout.workspaces[0].workspace_id
    );
    assert!(layout.windows.is_empty());

    let stored = db
        .with_connection(|conn| {
            preferences::get(conn, PrefScope::Device(device), ACTIVE_WORKSPACE_PREF_KEY)
                .map_err(Into::into)
        })
        .expect("read pref");
    assert_eq!(stored, Some(layout.active_workspace_id.to_string()));
}

#[tokio::test]
async fn load_layout_self_heals_a_stale_active_workspace_pref() {
    let (_dir, state, device) = open_test_state("wm-commands-load-heal");
    let db = state.database().expect("active");

    db.with_connection(|conn| {
        preferences::insert_or_update(
            conn,
            PrefScope::Device(device),
            ACTIVE_WORKSPACE_PREF_KEY,
            &Uuid::new_v4().to_string(),
        )
        .map(|_| ())
        .map_err(Into::into)
    })
    .expect("seed a stale pref pointing at nothing");

    let layout = load_layout(db, device).await.expect("load layout");
    assert_eq!(
        layout.active_workspace_id,
        layout.workspaces[0].workspace_id
    );
}

#[tokio::test]
async fn create_workspace_appends_a_new_one() {
    let (_dir, state, device) = open_test_state("wm-commands-create");
    let db = state.database().expect("active");

    let first = load_layout(db.clone(), device).await.expect("load");
    let created = create_workspace(db.clone(), device).await.expect("create");
    assert_eq!(created.position, 1);

    let layout = load_layout(db, device).await.expect("reload");
    assert_eq!(layout.workspaces.len(), 2);
    assert!(layout
        .workspaces
        .iter()
        .any(|w| w.workspace_id == first.workspaces[0].workspace_id));
}

#[tokio::test]
async fn delete_workspace_refuses_the_last_remaining_one() {
    let (_dir, state, device) = open_test_state("wm-commands-delete-last");
    let db = state.database().expect("active");
    let layout = load_layout(db.clone(), device).await.expect("load");

    let outcome = delete_workspace(db, device, layout.workspaces[0].workspace_id).await;
    assert!(matches!(outcome, Err(HolziError::InvalidInput { .. })));
}

#[tokio::test]
async fn delete_workspace_moves_the_active_pointer_to_the_previous_neighbor() {
    let (_dir, state, device) = open_test_state("wm-commands-delete-active");
    let db = state.database().expect("active");
    let first = load_layout(db.clone(), device).await.expect("load");
    let first_id = first.workspaces[0].workspace_id;
    let second = create_workspace(db.clone(), device)
        .await
        .expect("create second");

    set_active_workspace(db.clone(), device, second.workspace_id)
        .await
        .expect("switch to the second workspace");

    let result = delete_workspace(db, device, second.workspace_id)
        .await
        .expect("delete the active workspace");
    assert_eq!(result.workspaces.len(), 1);
    assert_eq!(result.active_workspace_id, first_id);
}

#[tokio::test]
async fn set_active_workspace_rejects_an_unknown_id() {
    let (_dir, state, device) = open_test_state("wm-commands-set-active-unknown");
    let db = state.database().expect("active");

    let outcome = set_active_workspace(db, device, Uuid::new_v4()).await;
    assert!(matches!(outcome, Err(HolziError::InvalidInput { .. })));
}

#[tokio::test]
async fn save_windows_then_load_layout_roundtrips_a_window() {
    let (_dir, state, device) = open_test_state("wm-commands-save-windows");
    let db = state.database().expect("active");
    let layout = load_layout(db.clone(), device).await.expect("load");
    let workspace_id = layout.workspaces[0].workspace_id;

    let tab_id = Uuid::new_v4();
    let window_id = Uuid::new_v4();
    let window = WindowDto {
        window_id,
        workspace_id,
        x: 10,
        y: 20,
        width: 800,
        height: 600,
        is_minimized: false,
        is_maximized: false,
        stack_order: 1,
        active_tab_id: tab_id,
        tabs: vec![TabDto {
            tab_id,
            app_id: "system.chat".to_string(),
        }],
    };
    save_windows(db.clone(), device, vec![window.into()])
        .await
        .expect("save windows");

    let reloaded = load_layout(db, device).await.expect("reload");
    assert_eq!(reloaded.windows.len(), 1);
    assert_eq!(reloaded.windows[0].window_id, window_id);
    assert_eq!(reloaded.windows[0].tabs[0].tab_id, tab_id);
}

/// Spec 015-workspace-shell, T051 (User Story 7 — multi-instance apps): `app_id` is an opaque,
/// non-unique column (data-model.md) -- nothing in the storage layer treats it as a singleton key.
/// Two tabs in two different windows sharing the same `app_id` must save and reload as two
/// distinct entries, exactly like two tabs of different apps would.
#[tokio::test]
async fn save_windows_persists_two_tabs_of_the_same_multi_instance_app_id() {
    let (_dir, state, device) = open_test_state("wm-commands-multi-instance");
    let db = state.database().expect("active");
    let layout = load_layout(db.clone(), device).await.expect("load");
    let workspace_id = layout.workspaces[0].workspace_id;

    let tab_a = Uuid::new_v4();
    let window_a = Uuid::new_v4();
    let tab_b = Uuid::new_v4();
    let window_b = Uuid::new_v4();
    let app_id = "extension.multi-instance-example".to_string();
    let windows = vec![
        WindowDto {
            window_id: window_a,
            workspace_id,
            x: 10,
            y: 20,
            width: 800,
            height: 600,
            is_minimized: false,
            is_maximized: false,
            stack_order: 1,
            active_tab_id: tab_a,
            tabs: vec![TabDto {
                tab_id: tab_a,
                app_id: app_id.clone(),
            }],
        },
        WindowDto {
            window_id: window_b,
            workspace_id,
            x: 40,
            y: 50,
            width: 800,
            height: 600,
            is_minimized: false,
            is_maximized: false,
            stack_order: 2,
            active_tab_id: tab_b,
            tabs: vec![TabDto {
                tab_id: tab_b,
                app_id: app_id.clone(),
            }],
        },
    ];

    save_windows(
        db.clone(),
        device,
        windows.into_iter().map(Into::into).collect(),
    )
    .await
    .expect("save two windows sharing the same app_id");

    let reloaded = load_layout(db, device).await.expect("reload");
    assert_eq!(reloaded.windows.len(), 2);
    let reloaded_window_ids: Vec<Uuid> = reloaded.windows.iter().map(|w| w.window_id).collect();
    assert!(reloaded_window_ids.contains(&window_a));
    assert!(reloaded_window_ids.contains(&window_b));
    for window in &reloaded.windows {
        assert_eq!(window.tabs.len(), 1);
        assert_eq!(window.tabs[0].app_id, app_id);
    }
}

#[tokio::test]
async fn save_windows_rejects_an_unknown_workspace() {
    let (_dir, state, device) = open_test_state("wm-commands-save-windows-bad-workspace");
    let db = state.database().expect("active");
    let tab_id = Uuid::new_v4();
    let window = WindowDto {
        window_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        x: 0,
        y: 0,
        width: 800,
        height: 600,
        is_minimized: false,
        is_maximized: false,
        stack_order: 1,
        active_tab_id: tab_id,
        tabs: vec![TabDto {
            tab_id,
            app_id: "system.chat".to_string(),
        }],
    };

    let outcome = save_windows(db, device, vec![window.into()]).await;
    assert!(matches!(outcome, Err(HolziError::InvalidInput { .. })));
}

#[tokio::test]
async fn close_windows_batch_removes_a_saved_window() {
    let (_dir, state, device) = open_test_state("wm-commands-close-windows");
    let db = state.database().expect("active");
    let layout = load_layout(db.clone(), device).await.expect("load");
    let workspace_id = layout.workspaces[0].workspace_id;

    let tab_id = Uuid::new_v4();
    let window_id = Uuid::new_v4();
    let window = WindowDto {
        window_id,
        workspace_id,
        x: 0,
        y: 0,
        width: 800,
        height: 600,
        is_minimized: false,
        is_maximized: false,
        stack_order: 1,
        active_tab_id: tab_id,
        tabs: vec![TabDto {
            tab_id,
            app_id: "system.chat".to_string(),
        }],
    };
    save_windows(db.clone(), device, vec![window.into()])
        .await
        .expect("save windows");

    close_windows_batch(db.clone(), device, vec![window_id])
        .await
        .expect("close the window");

    let reloaded = load_layout(db, device).await.expect("reload");
    assert!(reloaded.windows.is_empty());
}
