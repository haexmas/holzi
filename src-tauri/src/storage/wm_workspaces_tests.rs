//! Tests for `wm_workspaces` (spec 015-workspace-shell). Every function
//! under test calls `current_hlc()`, which only exists on an open
//! `haex_crdt::Database` — these open one directly, following
//! `identity::migrations_tests`'s style rather than splitting a separate
//! `tests/` integration file (there is no pure-validation surface here to
//! split out, unlike `preferences`/`chat_messages`).

use std::sync::Arc;

use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use uuid::Uuid;

use super::wm_workspaces::{
    create, delete, ensure_default, list, WorkspaceDeleteError, WorkspaceRow,
};
use crate::identity::{
    holzi_migration_source, installation_id_path, read_or_mint_installation_uuid, HolziBootstrap,
    HOLZI_TRIGGER_VERSION,
};
use crate::storage::known_devices;

/// Opens a fresh vault in a throwaway temp dir and resolves its bootstrap-
/// minted `vault_device_uuid`. The `TempDir` must stay alive for as long as
/// the database is used, hence it is returned alongside.
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

#[test]
fn ensure_default_creates_exactly_one_workspace_on_an_empty_device() {
    let (_dir, db, device) = open_test_vault("wm-workspaces-ensure-default");

    let rows = db
        .with_connection(|conn| Ok(ensure_default(conn, device)?))
        .expect("ensure default");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].position, 0);

    let rows_again = db
        .with_connection(|conn| Ok(ensure_default(conn, device)?))
        .expect("ensure default again");
    assert_eq!(
        rows_again, rows,
        "a second call must not create a duplicate"
    );
}

#[test]
fn create_appends_at_a_dense_position() {
    let (_dir, db, device) = open_test_vault("wm-workspaces-create");

    let first = db
        .with_connection(|conn| Ok(create(conn, device)?))
        .expect("first create");
    let second = db
        .with_connection(|conn| Ok(create(conn, device)?))
        .expect("second create");
    assert_eq!(first.position, 0);
    assert_eq!(second.position, 1);
    assert_ne!(first.workspace_id, second.workspace_id);

    let listed = db
        .with_connection(|conn| Ok(list(conn, device)?))
        .expect("list");
    assert_eq!(listed, vec![first, second]);
}

#[test]
fn deleting_the_last_workspace_is_refused() {
    let (_dir, db, device) = open_test_vault("wm-workspaces-last");
    let only = db
        .with_connection(|conn| Ok(create(conn, device)?))
        .expect("create");

    let outcome: std::result::Result<Vec<WorkspaceRow>, WorkspaceDeleteError> = db
        .with_connection(|conn| Ok(delete(conn, device, only.workspace_id)))
        .expect("with_connection succeeds even though delete refuses");
    assert!(matches!(outcome, Err(WorkspaceDeleteError::LastWorkspace)));

    // Refused means unchanged: the workspace is still there.
    let listed = db
        .with_connection(|conn| Ok(list(conn, device)?))
        .expect("list");
    assert_eq!(listed, vec![only]);
}

#[test]
fn deleting_an_unknown_workspace_is_refused() {
    let (_dir, db, device) = open_test_vault("wm-workspaces-unknown");
    db.with_connection(|conn| Ok(create(conn, device)?))
        .expect("create");

    let outcome: std::result::Result<Vec<WorkspaceRow>, WorkspaceDeleteError> = db
        .with_connection(|conn| Ok(delete(conn, device, Uuid::new_v4())))
        .expect("with_connection succeeds even though delete refuses");
    assert!(matches!(outcome, Err(WorkspaceDeleteError::NotFound)));
}

#[test]
fn deleting_a_middle_workspace_densely_renumbers_the_rest() {
    let (_dir, db, device) = open_test_vault("wm-workspaces-renumber");
    let first = db
        .with_connection(|conn| Ok(create(conn, device)?))
        .expect("create 0");
    let middle = db
        .with_connection(|conn| Ok(create(conn, device)?))
        .expect("create 1");
    let last = db
        .with_connection(|conn| Ok(create(conn, device)?))
        .expect("create 2");

    let remaining = db
        .with_connection(|conn| Ok(delete(conn, device, middle.workspace_id)))
        .expect("with_connection succeeds")
        .expect("delete the middle workspace");

    assert_eq!(
        remaining,
        vec![
            WorkspaceRow {
                workspace_id: first.workspace_id,
                position: 0
            },
            WorkspaceRow {
                workspace_id: last.workspace_id,
                position: 1
            },
        ]
    );

    let listed = db
        .with_connection(|conn| Ok(list(conn, device)?))
        .expect("list reflects the same renumbering");
    assert_eq!(listed, remaining);
}

#[test]
fn deleting_a_workspace_with_windows_still_succeeds_via_the_fk_cascade() {
    use haex_crdt::rusqlite::params;

    let (_dir, db, device) = open_test_vault("wm-workspaces-cascade");
    let doomed = db
        .with_connection(|conn| Ok(create(conn, device)?))
        .expect("create doomed workspace");
    db.with_connection(|conn| Ok(create(conn, device)?))
        .expect("create a second workspace so deletion is allowed");

    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO shell_windows \
             (vault_device_uuid, window_id, workspace_id, x, y, width, height, \
              is_minimized, is_maximized, stack_order, active_tab_id, haex_hlc_no_sync) \
             VALUES (?1, 'win-1', ?2, 0, 0, 800, 600, 0, 0, 1, 'tab-1', current_hlc())",
            params![device.to_string(), doomed.workspace_id.to_string()],
        )?;
        conn.execute(
            "INSERT INTO shell_window_tabs \
             (vault_device_uuid, tab_id, window_id, app_id, position, haex_hlc_no_sync) \
             VALUES (?1, 'tab-1', 'win-1', 'system.chat', 0, current_hlc())",
            params![device.to_string()],
        )?;
        Ok(())
    })
    .expect("seed a window and tab in the doomed workspace");

    db.with_connection(|conn| Ok(delete(conn, device, doomed.workspace_id)))
        .expect("with_connection succeeds")
        .expect("delete the workspace despite its open window");

    let (windows, tabs): (i64, i64) = db
        .with_connection(|conn| {
            let windows = conn.query_row(
                "SELECT COUNT(*) FROM shell_windows WHERE vault_device_uuid = ?1",
                params![device.to_string()],
                |r| r.get(0),
            )?;
            let tabs = conn.query_row(
                "SELECT COUNT(*) FROM shell_window_tabs WHERE vault_device_uuid = ?1",
                params![device.to_string()],
                |r| r.get(0),
            )?;
            Ok((windows, tabs))
        })
        .expect("count remaining rows");
    assert_eq!(windows, 0);
    assert_eq!(tabs, 0);
}
