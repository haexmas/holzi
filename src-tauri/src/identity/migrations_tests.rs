//! Spike for research.md R2: does a single-column foreign key that targets
//! a `UNIQUE` (not primary-key) column survive haex-crdt's `CREATE TABLE`
//! rewrite (it only touches the parsed `columns` list to add the three CRDT
//! metadata columns, per `crdt::transformer`'s `add_crdt_columns_to_create_table`
//! — table-level constraints, including `FOREIGN KEY`/`UNIQUE`, are never
//! read or rewritten). Exercised against both a genesis vault (0019 applied
//! at Database::open time) and a vault upgraded from 0018/trigger-version 10
//! (0019 applied as a later migration on top of already-provisioned tables),
//! per the plan's explicit two-case spike requirement.

use std::collections::BTreeMap;
use std::sync::Arc;

use haex_crdt::rusqlite::params;
use haex_crdt::{
    Database, DatabaseConfig, MigrationName, NoopSignatureProvider, SqlCipherKey,
    StaticMigrationSource,
};
use uuid::Uuid;

use crate::identity::{
    holzi_migration_source, installation_id_path, HolziBootstrap, HOLZI_TRIGGER_VERSION,
};
use crate::storage::known_devices;

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

/// Inserts one workspace, one window and one tab for `vault_device_uuid`,
/// then deletes the workspace and asserts the window and tab are gone too —
/// the FK-cascade path (data-model.md I7) that only works if `shell_windows.
/// workspace_id → workspaces(workspace_id)` and `shell_window_tabs.window_id
/// → shell_windows(window_id)` actually held after the rewrite.
fn assert_fk_cascade_works(db: &Database, vault_device_uuid: Uuid) {
    let device = vault_device_uuid.to_string();
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
    .expect("insert workspace, window and tab");

    db.with_connection(|conn| {
        conn.execute(
            "DELETE FROM workspaces WHERE vault_device_uuid = ?1 AND workspace_id = 'ws-1'",
            params![device],
        )?;
        Ok(())
    })
    .expect("delete the workspace");

    let (windows, tabs): (i64, i64) = db
        .with_connection(|conn| {
            let windows = conn.query_row(
                "SELECT COUNT(*) FROM shell_windows WHERE vault_device_uuid = ?1",
                params![device],
                |r| r.get(0),
            )?;
            let tabs = conn.query_row(
                "SELECT COUNT(*) FROM shell_window_tabs WHERE vault_device_uuid = ?1",
                params![device],
                |r| r.get(0),
            )?;
            Ok((windows, tabs))
        })
        .expect("count remaining rows");
    assert_eq!(
        windows, 0,
        "deleting the workspace must cascade to its window"
    );
    assert_eq!(
        tabs, 0,
        "deleting the workspace must cascade to its window's tabs"
    );
}

#[tokio::test]
async fn shell_layout_fk_cascade_survives_the_crdt_rewrite_on_a_fresh_vault() {
    tokio::task::spawn_blocking(|| {
        let dir = tempfile::tempdir().expect("tempdir");
        let install_path = installation_id_path(dir.path());
        let db = Database::open(DatabaseConfig {
            path: dir.path().join("vault.db"),
            key: SqlCipherKey::new("shell-migration-spike-fresh"),
            create_if_missing: true,
            bootstrap: Arc::new(HolziBootstrap::new(install_path.clone())),
            signature_provider: Arc::new(NoopSignatureProvider),
            migration_source: holzi_migration_source(),
            trigger_version: HOLZI_TRIGGER_VERSION,
        })
        .expect("open a fresh vault at the current schema (0019 included)");

        let installation_uuid = crate::identity::read_or_mint_installation_uuid(&install_path)
            .expect("installation uuid");
        let vault_device_uuid = db
            .with_connection(|conn| {
                Ok(known_devices::get_vault_device_uuid(
                    conn,
                    installation_uuid,
                )?)
            })
            .expect("read vault_device_uuid")
            .expect("bootstrap registered this installation");

        assert_fk_cascade_works(&db, vault_device_uuid);
    })
    .await
    .expect("join");
}

#[tokio::test]
async fn shell_layout_fk_cascade_survives_the_crdt_rewrite_on_an_upgraded_vault() {
    tokio::task::spawn_blocking(|| {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("vault.db");
        let install_path = installation_id_path(dir.path());

        // First open: provision at 0018 / trigger version 10, as if this vault
        // predates the 015-workspace-shell migration entirely.
        {
            Database::open(DatabaseConfig {
                path: db_path.clone(),
                key: SqlCipherKey::new("shell-migration-spike-upgrade"),
                create_if_missing: true,
                bootstrap: Arc::new(HolziBootstrap::new(install_path.clone())),
                signature_provider: Arc::new(NoopSignatureProvider),
                migration_source: migration_source_before("0019_shell_layout"),
                trigger_version: 10,
            })
            .expect("open and provision the vault at 0018/version 10");
        }

        // Second open: the current holzi build, applying 0019 on top of the
        // already-provisioned schema and recreating triggers at version 11.
        let db = Database::open(DatabaseConfig {
            path: db_path,
            key: SqlCipherKey::new("shell-migration-spike-upgrade"),
            create_if_missing: false,
            bootstrap: Arc::new(HolziBootstrap::new(install_path.clone())),
            signature_provider: Arc::new(NoopSignatureProvider),
            migration_source: holzi_migration_source(),
            trigger_version: HOLZI_TRIGGER_VERSION,
        })
        .expect("reopen the vault, applying 0019 as an upgrade");

        let installation_uuid = crate::identity::read_or_mint_installation_uuid(&install_path)
            .expect("installation uuid");
        let vault_device_uuid = db
            .with_connection(|conn| {
                Ok(known_devices::get_vault_device_uuid(
                    conn,
                    installation_uuid,
                )?)
            })
            .expect("read vault_device_uuid")
            .expect("bootstrap registered this installation");

        assert_fk_cascade_works(&db, vault_device_uuid);
    })
    .await
    .expect("join");
}
