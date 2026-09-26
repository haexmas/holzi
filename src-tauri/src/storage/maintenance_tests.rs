//! Tests for `maintenance` (spec 022-session-restore, research R5/R6,
//! contracts/wm-session.md §2). They open a real vault: the legacy
//! preference delete needs `current_hlc()` and the maintenance table only
//! exists after holzi's migrations.

use std::sync::Arc;

use haex_crdt::rusqlite::params;
use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use uuid::Uuid;

use super::maintenance::{
    run_after_open, run_pending_tasks, LEGACY_ACTIVE_WORKSPACE_KEY, VACUUM_TASK,
};
use super::preferences::{self, PrefScope};
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
    let device = db
        .with_connection(|conn| {
            Ok(known_devices::get_vault_device_uuid(
                conn,
                installation_uuid,
            )?)
        })
        .expect("read vault_device_uuid")
        .expect("bootstrap registered this installation");
    (dir, db, device)
}

fn query_i64(db: &Database, sql: &str) -> i64 {
    db.with_connection(|conn| Ok(conn.query_row(sql, [], |r| r.get(0))?))
        .expect("query")
}

fn pending_vacuum(db: &Database) -> bool {
    db.with_connection(|conn| {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM holzi_maintenance_no_sync WHERE task = ?1",
            params![VACUUM_TASK],
            |r| r.get::<_, i64>(0),
        )?)
    })
    .expect("read maintenance")
        > 0
}

#[test]
fn run_after_open_turns_on_secure_delete() {
    let (_dir, db, _device) = open_test_vault("maintenance-secure-delete");
    run_after_open(&db);
    assert_eq!(query_i64(&db, "PRAGMA secure_delete"), 1);
}

#[test]
fn run_after_open_deletes_the_legacy_active_workspace_preference_everywhere() {
    let (_dir, db, device) = open_test_vault("maintenance-legacy-pref");
    db.with_connection(|conn| {
        preferences::insert_or_update(
            conn,
            PrefScope::Device(device),
            LEGACY_ACTIVE_WORKSPACE_KEY,
            "ws-1",
        )?;
        preferences::insert_or_update(conn, PrefScope::Vault, LEGACY_ACTIVE_WORKSPACE_KEY, "ws-2")?;
        preferences::insert_or_update(conn, PrefScope::Device(device), "voice.auto_send", "true")?;
        Ok(())
    })
    .expect("write preferences");

    run_after_open(&db);

    let legacy = db
        .with_connection(|conn| Ok(preferences::list_by_key(conn, LEGACY_ACTIVE_WORKSPACE_KEY)?))
        .expect("list legacy");
    assert!(legacy.is_empty());
    let kept = db
        .with_connection(|conn| {
            Ok(preferences::get(
                conn,
                PrefScope::Device(device),
                "voice.auto_send",
            )?)
        })
        .expect("read kept");
    assert_eq!(kept.as_deref(), Some("true"));
}

#[test]
fn the_queued_vacuum_runs_once_and_leaves_no_free_pages() {
    let (_dir, db, _device) = open_test_vault("maintenance-vacuum");
    assert!(pending_vacuum(&db), "migration 0020 queues the VACUUM");

    run_after_open(&db);
    assert!(!pending_vacuum(&db));
    assert_eq!(query_i64(&db, "PRAGMA freelist_count"), 0);

    // A second run finds nothing to do.
    run_after_open(&db);
    assert!(!pending_vacuum(&db));
}

#[test]
fn a_failing_vacuum_keeps_the_task_for_the_next_open() {
    let (_dir, db, _device) = open_test_vault("maintenance-vacuum-fails");
    // VACUUM cannot run inside a transaction: this makes the task fail.
    let result = db.with_connection(|conn| {
        let tx = conn.unchecked_transaction()?;
        let outcome = run_pending_tasks(&tx);
        tx.rollback()?;
        Ok(outcome)
    });
    assert!(result.expect("run in a transaction").is_err());
    assert!(pending_vacuum(&db), "the task stays queued");

    run_after_open(&db);
    assert!(!pending_vacuum(&db), "the next open catches up");
}
