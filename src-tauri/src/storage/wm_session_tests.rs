//! Tests for `wm_session` (spec 022-session-restore, research R1/R2). The
//! table is `_no_sync`, so no `current_hlc()` is involved, but the schema only
//! exists on a vault opened with holzi's migrations — these open a real
//! `Database` like the other storage tests.

use std::sync::Arc;

use haex_crdt::rusqlite::params;
use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use serde_json::json;
use uuid::Uuid;

use super::wm_session::{delete, load, save, WmSessionError, MAX_SESSION_BYTES};
use crate::identity::{
    holzi_migration_source, installation_id_path, HolziBootstrap, HOLZI_TRIGGER_VERSION,
};

fn open_test_vault(passphrase: &str) -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::open(DatabaseConfig {
        path: dir.path().join("vault.db"),
        key: SqlCipherKey::new(passphrase),
        create_if_missing: true,
        bootstrap: Arc::new(HolziBootstrap::new(installation_id_path(dir.path()))),
        signature_provider: Arc::new(NoopSignatureProvider),
        migration_source: holzi_migration_source(),
        trigger_version: HOLZI_TRIGGER_VERSION,
    })
    .expect("open test vault");
    (dir, db)
}

fn session(marker: &str) -> serde_json::Value {
    json!({ "version": 1, "workspaces": [], "windows": [], "activeWorkspaceId": marker })
}

fn row_count(db: &Database) -> i64 {
    db.with_connection(|conn| {
        Ok(conn.query_row("SELECT COUNT(*) FROM wm_sessions_no_sync", [], |r| r.get(0))?)
    })
    .expect("count rows")
}

#[test]
fn save_inserts_then_overwrites_the_single_row_of_a_device() {
    let (_dir, db) = open_test_vault("wm-session-overwrite");
    let device = Uuid::new_v4();
    db.with_connection(|conn| {
        save(conn, device, &session("first")).expect("first save");
        save(conn, device, &session("second")).expect("second save");
        Ok(())
    })
    .expect("save twice");
    assert_eq!(row_count(&db), 1);
    let loaded = db
        .with_connection(|conn| Ok(load(conn, device).expect("load")))
        .expect("load");
    assert_eq!(loaded, Some(session("second")));
}

#[test]
fn load_returns_none_without_a_saved_session() {
    let (_dir, db) = open_test_vault("wm-session-empty");
    let loaded = db
        .with_connection(|conn| Ok(load(conn, Uuid::new_v4()).expect("load")))
        .expect("load");
    assert_eq!(loaded, None);
}

#[test]
fn delete_removes_only_the_own_device_row() {
    let (_dir, db) = open_test_vault("wm-session-delete");
    let own = Uuid::new_v4();
    let other = Uuid::new_v4();
    db.with_connection(|conn| {
        save(conn, own, &session("own")).expect("save own");
        save(conn, other, &session("other")).expect("save other");
        delete(conn, own).expect("delete own");
        Ok(())
    })
    .expect("save and delete");
    let (own_loaded, other_loaded) = db
        .with_connection(|conn| {
            Ok((
                load(conn, own).expect("own"),
                load(conn, other).expect("other"),
            ))
        })
        .expect("load");
    assert_eq!(own_loaded, None);
    assert_eq!(other_loaded, Some(session("other")));
}

#[test]
fn save_rejects_anything_but_a_json_object() {
    let (_dir, db) = open_test_vault("wm-session-not-object");
    let device = Uuid::new_v4();
    let result = db
        .with_connection(|conn| Ok(save(conn, device, &json!([1, 2, 3]))))
        .expect("run save");
    assert!(matches!(result, Err(WmSessionError::NotAnObject)));
    assert_eq!(row_count(&db), 0);
}

#[test]
fn save_rejects_a_session_above_the_size_limit() {
    let (_dir, db) = open_test_vault("wm-session-too-large");
    let device = Uuid::new_v4();
    let big = json!({ "version": 1, "padding": "x".repeat(MAX_SESSION_BYTES) });
    let result = db
        .with_connection(|conn| Ok(save(conn, device, &big)))
        .expect("run save");
    assert!(matches!(result, Err(WmSessionError::TooLarge { .. })));
    assert_eq!(row_count(&db), 0);
}

#[test]
fn a_corrupt_stored_row_loads_as_an_error_not_a_panic() {
    let (_dir, db) = open_test_vault("wm-session-corrupt");
    let device = Uuid::new_v4();
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO wm_sessions_no_sync (vault_device_uuid, session_json, updated_at) \
             VALUES (?1, 'not json', '2026-09-26T00:00:00Z')",
            params![device.to_string()],
        )?;
        Ok(())
    })
    .expect("insert a corrupt row");
    let result = db
        .with_connection(|conn| Ok(load(conn, device)))
        .expect("run load");
    assert!(matches!(result, Err(WmSessionError::Json(_))));
}
