//! Migration tests for `0020_wm_session_no_sync` (spec 022-session-restore,
//! research R5). Each case runs against a genesis vault (every migration
//! applied at `Database::open`) and against a vault upgraded from the
//! previous schema, where 0020 has to drop the spec 015 tables that already
//! hold rows.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use haex_crdt::rusqlite::params;
use haex_crdt::{
    Database, DatabaseConfig, MigrationName, NoopSignatureProvider, SqlCipherKey,
    StaticMigrationSource,
};
use uuid::Uuid;

use crate::identity::{
    holzi_migration_source, installation_id_path, read_or_mint_installation_uuid, HolziBootstrap,
    HOLZI_TRIGGER_VERSION,
};
use crate::storage::known_devices;

const LEGACY_TABLES: [&str; 3] = ["workspaces", "shell_windows", "shell_window_tabs"];

/// The frozen migration set, truncated to everything before `name` — models
/// "a vault provisioned by an older holzi build that hasn't seen this
/// migration yet".
fn migration_source_before(name: &str) -> Arc<StaticMigrationSource> {
    let full = holzi_migration_source();
    let filtered: BTreeMap<MigrationName, String> = full
        .0
        .iter()
        .filter(|(k, _)| k.as_str() < name)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Arc::new(StaticMigrationSource(filtered))
}

fn open(
    dir: &Path,
    passphrase: &str,
    create: bool,
    source: Arc<StaticMigrationSource>,
) -> Database {
    Database::open(DatabaseConfig {
        path: dir.join("vault.db"),
        key: SqlCipherKey::new(passphrase),
        create_if_missing: create,
        bootstrap: Arc::new(HolziBootstrap::new(installation_id_path(dir))),
        signature_provider: Arc::new(NoopSignatureProvider),
        migration_source: source,
        trigger_version: HOLZI_TRIGGER_VERSION,
    })
    .expect("open the test vault")
}

fn device_of(db: &Database, dir: &Path) -> Uuid {
    let installation_uuid =
        read_or_mint_installation_uuid(&installation_id_path(dir)).expect("installation uuid");
    db.with_connection(|conn| {
        Ok(known_devices::get_vault_device_uuid(
            conn,
            installation_uuid,
        )?)
    })
    .expect("read vault_device_uuid")
    .expect("bootstrap registered this installation")
}

fn table_exists(db: &Database, name: &str) -> bool {
    db.with_connection(|conn| {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![name],
            |r| r.get::<_, i64>(0),
        )?)
    })
    .expect("query sqlite_master")
        > 0
}

fn maintenance_tasks(db: &Database) -> Vec<String> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare("SELECT task FROM holzi_maintenance_no_sync ORDER BY task")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
    .expect("read maintenance tasks")
}

fn delete_markers_for_legacy_tables(db: &Database) -> i64 {
    db.with_connection(|conn| {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM haex_deleted_rows \
             WHERE table_name IN ('workspaces', 'shell_windows', 'shell_window_tabs')",
            [],
            |r| r.get(0),
        )?)
    })
    .expect("count delete markers")
}

fn assert_current_schema(db: &Database) {
    for table in LEGACY_TABLES {
        assert!(!table_exists(db, table), "{table} must be dropped by 0020");
    }
    assert!(table_exists(db, "wm_sessions_no_sync"));
    assert!(table_exists(db, "holzi_maintenance_no_sync"));
    assert_eq!(
        maintenance_tasks(db),
        vec!["vacuum_after_legacy_wm_drop".to_string()],
        "0020 queues the one-time VACUUM (research R6)"
    );
}

#[tokio::test]
async fn migration_0020_gives_a_fresh_vault_the_session_tables_only() {
    tokio::task::spawn_blocking(|| {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = open(
            dir.path(),
            "wm-session-migration-fresh",
            true,
            holzi_migration_source(),
        );
        assert_current_schema(&db);
        assert_eq!(delete_markers_for_legacy_tables(&db), 0);
    })
    .await
    .expect("join");
}

#[tokio::test]
async fn migration_0020_drops_populated_spec_015_tables_without_delete_markers() {
    tokio::task::spawn_blocking(|| {
        let dir = tempfile::tempdir().expect("tempdir");

        // First open: the schema before 0020, with a saved spec 015 session.
        {
            let db = open(
                dir.path(),
                "wm-session-migration-upgrade",
                true,
                migration_source_before("0020_wm_session_no_sync"),
            );
            let device = device_of(&db, dir.path()).to_string();
            db.with_connection(|conn| {
                conn.execute(
                    "INSERT INTO workspaces (vault_device_uuid, workspace_id, position, haex_hlc_no_sync) \
                     VALUES (?1, 'ws-1', 0, current_hlc())",
                    params![device],
                )?;
                conn.execute(
                    "INSERT INTO shell_windows \
                     (vault_device_uuid, window_id, workspace_id, x, y, width, height, \
                      is_minimized, is_maximized, stack_order, active_tab_id, haex_hlc_no_sync) \
                     VALUES (?1, 'win-1', 'ws-1', 0, 0, 800, 600, 0, 0, 1, 'tab-1', current_hlc())",
                    params![device],
                )?;
                conn.execute(
                    "INSERT INTO shell_window_tabs \
                     (vault_device_uuid, tab_id, window_id, app_id, position, haex_hlc_no_sync) \
                     VALUES (?1, 'tab-1', 'win-1', 'system.chat', 0, current_hlc())",
                    params![device],
                )?;
                Ok(())
            })
            .expect("save a spec 015 session");
            assert_eq!(delete_markers_for_legacy_tables(&db), 0);
        }

        // Second open: the current build applies 0020 on top.
        let db = open(
            dir.path(),
            "wm-session-migration-upgrade",
            false,
            holzi_migration_source(),
        );
        assert_current_schema(&db);
        assert_eq!(
            delete_markers_for_legacy_tables(&db),
            0,
            "DROP TABLE must not write delete markers that would sync to other devices"
        );
    })
    .await
    .expect("join");
}
