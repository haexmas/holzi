//! Migration tests for `0020_wm_session_no_sync` (spec 022-session-restore,
//! research R5) and `0021_own_device_sync` (spec 024, research R2/R3). Each
//! case runs against a genesis vault (every migration applied at
//! `Database::open`) and against a vault upgraded from the previous schema,
//! where the migration has to carry existing rows along.

// These tests build legacy schemas and read `sqlite_master` and the delete log
// directly, which the CRDT write path does not expose.
#![allow(clippy::disallowed_methods)]

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
        max_transaction_bytes: haex_crdt::MAX_CRDT_TRANSACTION_BYTES,
    })
    .expect("open the test vault")
}

fn device_of(db: &Database, dir: &Path) -> Uuid {
    let installation_uuid =
        read_or_mint_installation_uuid(&installation_id_path(dir)).expect("installation uuid");
    crate::storage::query::read(db, |r| {
        known_devices::get_vault_device_uuid(r, installation_uuid)
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

const SYNC_TABLES: [&str; 10] = [
    "vault_identity_secret_no_sync",
    "device_keys_no_sync",
    "device_lists",
    "vault_key_generations",
    "vault_key_envelopes",
    "vault_content_keys_no_sync",
    "sync_progress_no_sync",
    "pending_links_no_sync",
    "admission_requests",
    "device_presence_no_sync",
];

fn column_names(db: &Database, table: &str) -> Vec<String> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")?;
        let rows = stmt.query_map(params![table], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
    .expect("read columns")
}

fn count(db: &Database, sql: &str) -> i64 {
    db.with_connection(|conn| Ok(conn.query_row(sql, [], |r| r.get(0))?))
        .expect("count")
}

fn stored_seed(db: &Database) -> Option<Vec<u8>> {
    use haex_crdt::rusqlite::OptionalExtension;
    db.with_connection(|conn| {
        Ok(conn
            .query_row(
                "SELECT privkey FROM vault_identity_secret_no_sync WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .optional()?)
    })
    .expect("read seed")
}

/// Opens a vault on the schema before 0021 and gives it the placeholder
/// identity the bootstrap minted before spec 024.
fn legacy_vault_with_placeholder(dir: &Path, passphrase: &str, placeholder: [u8; 32]) {
    let db = open(
        dir,
        passphrase,
        true,
        migration_source_before("0021_own_device_sync"),
    );
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO vault_identity (id, pubkey, privkey) VALUES (1, ?1, ?2)",
            params![[3u8; 33].as_slice(), placeholder.as_slice()],
        )?;
        Ok(())
    })
    .expect("placeholder identity");
}

#[tokio::test]
async fn migration_0021_gives_a_fresh_vault_the_sync_tables_and_an_empty_identity() {
    tokio::task::spawn_blocking(|| {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = open(
            dir.path(),
            "sync-migration-fresh",
            true,
            holzi_migration_source(),
        );
        for table in SYNC_TABLES {
            assert!(table_exists(&db, table), "{table} is created by 0021");
        }
        let columns = column_names(&db, "vault_identity");
        assert!(columns.contains(&"pubkey".to_string()));
        assert!(!columns.contains(&"privkey".to_string()), "{columns:?}");
        assert!(
            columns.contains(&"haex_hlc_no_sync".to_string()),
            "the rebuilt table stays a CRDT table"
        );
        assert_eq!(count(&db, "SELECT COUNT(*) FROM vault_identity"), 0);
        assert_eq!(stored_seed(&db), None);
    })
    .await
    .expect("join");
}

#[tokio::test]
async fn migration_0021_keeps_the_placeholder_as_seed_without_delete_markers() {
    tokio::task::spawn_blocking(|| {
        let dir = tempfile::tempdir().expect("tempdir");
        let placeholder = [11u8; 32];
        legacy_vault_with_placeholder(dir.path(), "sync-migration-upgrade", placeholder);

        let db = open(
            dir.path(),
            "sync-migration-upgrade",
            false,
            holzi_migration_source(),
        );

        assert_eq!(stored_seed(&db), Some(placeholder.to_vec()));
        assert_eq!(count(&db, "SELECT COUNT(*) FROM vault_identity"), 0);
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM haex_deleted_rows WHERE table_name = 'vault_identity'"
            ),
            0,
            "rebuilding the identity table writes no delete marker"
        );
    })
    .await
    .expect("join");
}

#[tokio::test]
async fn two_copies_of_a_legacy_vault_derive_the_same_identity() {
    tokio::task::spawn_blocking(|| {
        let first_dir = tempfile::tempdir().expect("tempdir");
        let second_dir = tempfile::tempdir().expect("tempdir");
        let placeholder = [12u8; 32];
        legacy_vault_with_placeholder(first_dir.path(), "sync-migration-copies", placeholder);
        std::fs::copy(
            first_dir.path().join("vault.db"),
            second_dir.path().join("vault.db"),
        )
        .expect("copy the vault file");

        let identity = |dir: &Path| {
            let db = open(
                dir,
                "sync-migration-copies",
                false,
                holzi_migration_source(),
            );
            let installation =
                read_or_mint_installation_uuid(&installation_id_path(dir)).expect("installation");
            crate::sync::genesis::ensure_sync_state(&db, installation, false)
                .expect("sync state")
                .vault_pubkey
                .expect("the placeholder yields an identity")
        };
        let first = identity(first_dir.path());
        let second = identity(second_dir.path());

        let derived = crate::sync::keys::derive_vault_identity(&placeholder);
        assert_eq!(
            first, second,
            "research R3: every copy derives the same identity"
        );
        assert_eq!(
            first,
            crate::sync::signing::xonly_public_key(&derived).expect("public key")
        );
    })
    .await
    .expect("join");
}
