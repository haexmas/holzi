//! Tests for `maintenance` (spec 022-session-restore, research R5/R6,
//! contracts/wm-session.md §2). They open a real vault: the legacy
//! preference delete needs `current_hlc()` and the maintenance table only
//! exists after holzi's migrations.

use std::sync::Arc;

use haex_crdt::rusqlite::params;
use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use uuid::Uuid;

use super::maintenance::{
    fold_scoped_preferences, run_after_open, run_pending_tasks, LEGACY_ACTIVE_WORKSPACE_KEY,
    VACUUM_TASK,
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

fn set(db: &Database, scope: PrefScope, key: &str, value: &str) {
    db.with_connection(|conn| Ok(preferences::insert_or_update(conn, scope, key, value)?))
        .expect("set preference");
}

fn get(db: &Database, scope: PrefScope, key: &str) -> Option<String> {
    db.with_connection(|conn| Ok(preferences::get(conn, scope, key)?))
        .expect("get preference")
}

fn fold(db: &Database, device: Uuid) {
    db.with_connection(|conn| fold_scoped_preferences(conn, device))
        .expect("fold preferences");
}

#[test]
fn a_device_value_of_a_vault_setting_becomes_the_vault_value() {
    let (_dir, db, device) = open_test_vault("fold-vault-settings");
    set(
        &db,
        PrefScope::Device(device),
        "chat.autonomy_mode",
        "ungated",
    );
    set(
        &db,
        PrefScope::Device(device),
        "cli_delegate.deny_rules",
        "[\"git_push\"]",
    );
    set(
        &db,
        PrefScope::Device(device),
        "chat.reasoning_option.qwen",
        "high",
    );

    fold(&db, device);

    for (key, value) in [
        ("chat.autonomy_mode", "ungated"),
        ("cli_delegate.deny_rules", "[\"git_push\"]"),
        ("chat.reasoning_option.qwen", "high"),
    ] {
        assert_eq!(
            get(&db, PrefScope::Vault, key).as_deref(),
            Some(value),
            "{key}"
        );
        assert_eq!(get(&db, PrefScope::Device(device), key), None, "{key}");
    }
}

#[test]
fn an_existing_vault_value_wins_and_the_device_value_is_dropped() {
    let (_dir, db, device) = open_test_vault("fold-vault-wins");
    set(&db, PrefScope::Vault, "wm.session_restore", "false");
    set(&db, PrefScope::Device(device), "wm.session_restore", "true");

    fold(&db, device);

    assert_eq!(
        get(&db, PrefScope::Vault, "wm.session_restore").as_deref(),
        Some("false")
    );
    assert_eq!(
        get(&db, PrefScope::Device(device), "wm.session_restore"),
        None
    );
}

#[test]
fn the_vault_default_model_becomes_this_devices_when_it_has_none() {
    let (_dir, db, device) = open_test_vault("fold-default-model");
    set(&db, PrefScope::Vault, "chat.default_model_id", "qwen");

    fold(&db, device);

    assert_eq!(
        get(&db, PrefScope::Device(device), "chat.default_model_id").as_deref(),
        Some("qwen")
    );
    assert_eq!(get(&db, PrefScope::Vault, "chat.default_model_id"), None);

    set(&db, PrefScope::Vault, "chat.default_model_id", "llama");
    fold(&db, device);
    assert_eq!(
        get(&db, PrefScope::Device(device), "chat.default_model_id").as_deref(),
        Some("qwen"),
        "this device keeps its own value"
    );
    assert_eq!(get(&db, PrefScope::Vault, "chat.default_model_id"), None);
}

#[test]
fn folding_leaves_device_settings_alone_and_runs_on_open() {
    let (_dir, db, device) = open_test_vault("fold-on-open");
    set(
        &db,
        PrefScope::Device(device),
        "voice.stt_model_id",
        "whisper-tiny",
    );
    set(
        &db,
        PrefScope::Device(device),
        "appearance.color_scheme",
        "dark",
    );

    run_after_open(&db);
    run_after_open(&db);

    assert_eq!(
        get(&db, PrefScope::Device(device), "voice.stt_model_id").as_deref(),
        Some("whisper-tiny")
    );
    assert_eq!(
        get(&db, PrefScope::Vault, "appearance.color_scheme").as_deref(),
        Some("dark")
    );
    assert_eq!(
        get(&db, PrefScope::Device(device), "appearance.color_scheme"),
        None
    );
}
