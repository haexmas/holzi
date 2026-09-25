//! Tests for `shell_windows` (spec 015-workspace-shell). Every function
//! under test needs `current_hlc()`, so — same reasoning as
//! `shell_workspaces_tests` — these open a real `Database` directly.

use std::sync::Arc;

use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use uuid::Uuid;

use super::shell_windows::{
    close_windows, load_all, save_batch, ShellWindowError, TabWrite, WindowWrite,
};
use super::shell_workspaces::create as create_workspace;
use crate::identity::{
    holzi_migration_source, installation_id_path, read_or_mint_installation_uuid, HolziBootstrap,
    HOLZI_TRIGGER_VERSION,
};
use crate::storage::known_devices;

fn open_test_vault(passphrase: &str) -> (tempfile::TempDir, Database, Uuid) {
    let dir = tempfile::tempdir().expect("tempdir");
    let install_path = installation_id_path(dir.path());
    let db = Database::open(DatabaseConfig {
        path: dir.path().join("vault.db"),
        key: SqlCipherKey::new(passphrase),
        create_if_missing: true,
        bootstrap: Arc::new(HolziBootstrap::new(install_path.clone())),
        signature_provider: Arc::new(NoopSignatureProvider),
        migration_source: holzi_migration_source(),
        trigger_version: HOLZI_TRIGGER_VERSION,
    })
    .expect("open test vault");
    let installation_uuid =
        read_or_mint_installation_uuid(&install_path).expect("installation uuid");
    let vault_device_uuid = db
        .with_connection(|conn| {
            Ok(known_devices::get_vault_device_uuid(
                conn,
                installation_uuid,
            )?)
        })
        .expect("read vault_device_uuid")
        .expect("bootstrap registered this installation");
    (dir, db, vault_device_uuid)
}

fn one_tab_window(workspace_id: Uuid, stack_order: i64) -> WindowWrite {
    let tab_id = Uuid::new_v4();
    WindowWrite {
        window_id: Uuid::new_v4(),
        workspace_id,
        x: 10,
        y: 20,
        width: 800,
        height: 600,
        is_minimized: false,
        is_maximized: false,
        stack_order,
        active_tab_id: tab_id,
        tabs: vec![TabWrite {
            tab_id,
            app_id: "system.chat".to_string(),
        }],
    }
}

#[test]
fn save_batch_inserts_a_window_with_its_tabs_in_order() {
    let (_dir, db, device) = open_test_vault("shell-windows-insert");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");

    let tab_a = Uuid::new_v4();
    let tab_b = Uuid::new_v4();
    let window = WindowWrite {
        tabs: vec![
            TabWrite {
                tab_id: tab_a,
                app_id: "system.chat".to_string(),
            },
            TabWrite {
                tab_id: tab_b,
                app_id: "system.settings".to_string(),
            },
        ],
        active_tab_id: tab_a,
        ..one_tab_window(workspace.workspace_id, 1)
    };

    db.with_connection(|conn| Ok(save_batch(conn, device, std::slice::from_ref(&window))))
        .expect("with_connection succeeds")
        .expect("save batch");

    let loaded = db
        .with_connection(|conn| Ok(load_all(conn, device)?))
        .expect("load");
    assert_eq!(loaded.len(), 1);
    let loaded_window = &loaded[0];
    assert_eq!(loaded_window.window_id, window.window_id);
    assert_eq!(
        loaded_window
            .tabs
            .iter()
            .map(|t| t.tab_id)
            .collect::<Vec<_>>(),
        vec![tab_a, tab_b]
    );
}

#[test]
fn save_batch_updates_geometry_and_replaces_tabs() {
    let (_dir, db, device) = open_test_vault("shell-windows-update");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");
    let window = one_tab_window(workspace.workspace_id, 1);
    db.with_connection(|conn| Ok(save_batch(conn, device, std::slice::from_ref(&window))))
        .expect("with_connection succeeds")
        .expect("save initial batch");

    let new_tab = Uuid::new_v4();
    let updated = WindowWrite {
        x: 99,
        active_tab_id: new_tab,
        tabs: vec![TabWrite {
            tab_id: new_tab,
            app_id: "system.federation".to_string(),
        }],
        ..window.clone()
    };
    db.with_connection(|conn| Ok(save_batch(conn, device, std::slice::from_ref(&updated))))
        .expect("with_connection succeeds")
        .expect("save updated batch");

    let loaded = db
        .with_connection(|conn| Ok(load_all(conn, device)?))
        .expect("load");
    assert_eq!(loaded.len(), 1, "still the same window, not a duplicate");
    assert_eq!(loaded[0].x, 99);
    assert_eq!(loaded[0].tabs.len(), 1);
    assert_eq!(loaded[0].tabs[0].tab_id, new_tab);
    assert_eq!(loaded[0].tabs[0].app_id, "system.federation");
}

#[test]
fn save_batch_rejects_a_window_with_no_tabs() {
    let (_dir, db, device) = open_test_vault("shell-windows-empty-tabs");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");
    let window = WindowWrite {
        tabs: vec![],
        ..one_tab_window(workspace.workspace_id, 1)
    };

    let outcome: std::result::Result<(), ShellWindowError> = db
        .with_connection(|conn| Ok(save_batch(conn, device, &[window])))
        .expect("with_connection succeeds");
    assert!(matches!(outcome, Err(ShellWindowError::TabCountOutOfRange)));
}

#[test]
fn save_batch_rejects_an_active_tab_id_outside_the_window() {
    let (_dir, db, device) = open_test_vault("shell-windows-bad-active-tab");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");
    let window = WindowWrite {
        active_tab_id: Uuid::new_v4(),
        ..one_tab_window(workspace.workspace_id, 1)
    };

    let outcome: std::result::Result<(), ShellWindowError> = db
        .with_connection(|conn| Ok(save_batch(conn, device, &[window])))
        .expect("with_connection succeeds");
    assert!(matches!(
        outcome,
        Err(ShellWindowError::ActiveTabNotInWindow)
    ));
}

#[test]
fn save_batch_rejects_a_tab_id_reused_across_two_windows() {
    let (_dir, db, device) = open_test_vault("shell-windows-duplicate-tab");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");
    let shared_tab = Uuid::new_v4();
    let first = WindowWrite {
        tabs: vec![TabWrite {
            tab_id: shared_tab,
            app_id: "system.chat".to_string(),
        }],
        active_tab_id: shared_tab,
        ..one_tab_window(workspace.workspace_id, 1)
    };
    let second = WindowWrite {
        tabs: vec![TabWrite {
            tab_id: shared_tab,
            app_id: "system.chat".to_string(),
        }],
        active_tab_id: shared_tab,
        ..one_tab_window(workspace.workspace_id, 2)
    };

    let outcome: std::result::Result<(), ShellWindowError> = db
        .with_connection(|conn| Ok(save_batch(conn, device, &[first, second])))
        .expect("with_connection succeeds");
    assert!(matches!(outcome, Err(ShellWindowError::DuplicateTabId)));
}

#[test]
fn save_batch_rejects_an_invalid_app_id() {
    let (_dir, db, device) = open_test_vault("shell-windows-bad-app-id");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");
    let tab_id = Uuid::new_v4();
    let window = WindowWrite {
        tabs: vec![TabWrite {
            tab_id,
            app_id: String::new(),
        }],
        active_tab_id: tab_id,
        ..one_tab_window(workspace.workspace_id, 1)
    };

    let outcome: std::result::Result<(), ShellWindowError> = db
        .with_connection(|conn| Ok(save_batch(conn, device, &[window])))
        .expect("with_connection succeeds");
    assert!(matches!(outcome, Err(ShellWindowError::InvalidAppId)));
}

#[test]
fn save_batch_rejects_geometry_outside_the_allowed_bounds() {
    let (_dir, db, device) = open_test_vault("shell-windows-bad-geometry");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");
    let window = WindowWrite {
        width: 0,
        ..one_tab_window(workspace.workspace_id, 1)
    };

    let outcome: std::result::Result<(), ShellWindowError> = db
        .with_connection(|conn| Ok(save_batch(conn, device, &[window])))
        .expect("with_connection succeeds");
    assert!(matches!(
        outcome,
        Err(ShellWindowError::GeometryOutOfBounds)
    ));
}

#[test]
fn save_batch_rejects_an_unknown_workspace() {
    let (_dir, db, device) = open_test_vault("shell-windows-unknown-workspace");
    let window = one_tab_window(Uuid::new_v4(), 1);

    let outcome: std::result::Result<(), ShellWindowError> = db
        .with_connection(|conn| Ok(save_batch(conn, device, &[window])))
        .expect("with_connection succeeds");
    assert!(matches!(outcome, Err(ShellWindowError::UnknownWorkspace)));
}

#[test]
fn save_batch_enforces_the_device_window_cap() {
    let (_dir, db, device) = open_test_vault("shell-windows-cap");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");

    let full_batch: Vec<WindowWrite> = (0..500)
        .map(|i| one_tab_window(workspace.workspace_id, i))
        .collect();
    db.with_connection(|conn| Ok(save_batch(conn, device, &full_batch)))
        .expect("with_connection succeeds")
        .expect("saving exactly 500 windows is allowed");

    let one_more = one_tab_window(workspace.workspace_id, 500);
    let outcome: std::result::Result<(), ShellWindowError> = db
        .with_connection(|conn| Ok(save_batch(conn, device, &[one_more])))
        .expect("with_connection succeeds");
    assert!(matches!(outcome, Err(ShellWindowError::TooManyWindows)));
}

#[test]
fn close_windows_removes_windows_and_their_tabs_and_ignores_unknown_ids() {
    let (_dir, db, device) = open_test_vault("shell-windows-close");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");
    let window = one_tab_window(workspace.workspace_id, 1);
    db.with_connection(|conn| Ok(save_batch(conn, device, std::slice::from_ref(&window))))
        .expect("with_connection succeeds")
        .expect("save batch");

    db.with_connection(|conn| {
        Ok(close_windows(
            conn,
            device,
            &[window.window_id, Uuid::new_v4()],
        )?)
    })
    .expect("close windows, ignoring the unknown id");

    let loaded = db
        .with_connection(|conn| Ok(load_all(conn, device)?))
        .expect("load");
    assert!(loaded.is_empty());
}

#[test]
fn load_all_orders_windows_by_stack_order_and_skips_ones_without_tabs() {
    let (_dir, db, device) = open_test_vault("shell-windows-load-order");
    let workspace = db
        .with_connection(|conn| Ok(create_workspace(conn, device)?))
        .expect("create workspace");
    let back = one_tab_window(workspace.workspace_id, 1);
    let front = one_tab_window(workspace.workspace_id, 2);
    db.with_connection(|conn| Ok(save_batch(conn, device, &[front.clone(), back.clone()])))
        .expect("with_connection succeeds")
        .expect("save batch");

    let loaded = db
        .with_connection(|conn| Ok(load_all(conn, device)?))
        .expect("load");
    assert_eq!(
        loaded.iter().map(|w| w.window_id).collect::<Vec<_>>(),
        vec![back.window_id, front.window_id],
        "ascending stack_order: back (1) before front (2)"
    );
}
