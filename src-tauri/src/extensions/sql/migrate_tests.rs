// These tests put migrations into the registry and read the schema directly.
#![allow(clippy::disallowed_methods)]

use haex_crdt::rusqlite::params;

use super::*;
use crate::extensions::ids::migration_id;
use crate::extensions::sql::test_support::{own, setup, t, Setup};

fn extension() -> Uuid {
    crate::extensions::ids::extension_id(&own().public_key, &own().name)
}

fn add(s: &Setup, position: i64, name: &str, sql: &str) {
    let sql = t(sql);
    let hash = sql_sha256(&sql);
    let ext = extension();
    let id = migration_id(ext, name, &hash).to_string();
    let name = name.to_owned();
    let key = own().public_key.as_str().to_owned();
    s.db.write(move |tx| {
        tx.execute(
            "INSERT OR IGNORE INTO extensions (id, public_key, name, enabled, state, purge_data, \
             installed_at, updated_at) VALUES (?1, ?2, 'notes', 1, 'installed', 0, 0, 0)",
            params![ext.to_string(), key],
        )?;
        tx.execute(
            "INSERT INTO extension_migrations (id, extension_id, name, position, sql, sql_sha256) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, ext.to_string(), name, position, sql, hash],
        )?;
        Ok(())
    })
    .unwrap();
}

fn migrate(s: &Setup) -> Result<Vec<String>, MigrationError> {
    apply_pending(&s.vault, extension(), &own(), 1)
}

fn table_exists(s: &Setup, name: &str) -> bool {
    s.count(&format!(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = '{}'",
        t(name)
    )) > 0
}

#[test]
fn migrations_apply_in_order_once() {
    let s = setup();
    add(
        &s,
        1,
        "0001_tags",
        "ALTER TABLE t:books ADD COLUMN tag TEXT",
    );
    add(
        &s,
        0,
        "0000_init",
        "CREATE TABLE t:books (id TEXT PRIMARY KEY)",
    );
    assert_eq!(migrate(&s).unwrap(), ["0000_init", "0001_tags"]);
    assert!(table_exists(&s, "t:books"));
    assert_eq!(migrate(&s).unwrap(), Vec::<String>::new());
    // The new table is a CRDT table with its change triggers.
    assert_eq!(
        s.count(&t(
            "SELECT count(*) FROM pragma_table_info('t:books') WHERE name = 'haex_hlc_no_sync'"
        )),
        1
    );
}

#[test]
fn two_starts_at_once_apply_a_migration_once_and_both_succeed() {
    let s = setup();
    add(
        &s,
        0,
        "0000_init",
        "CREATE TABLE t:books (id TEXT PRIMARY KEY)",
    );
    let results: Vec<_> = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..2).map(|_| scope.spawn(|| migrate(&s))).collect();
        runs.into_iter().map(|r| r.join().unwrap()).collect()
    });
    let applied: Vec<String> = results.into_iter().flat_map(Result::unwrap).collect();
    assert_eq!(applied, ["0000_init"]);
}

#[test]
fn changed_sql_under_an_applied_name_stops_the_extension() {
    let s = setup();
    add(
        &s,
        0,
        "0000_init",
        "CREATE TABLE t:books (id TEXT PRIMARY KEY)",
    );
    migrate(&s).unwrap();
    add(
        &s,
        0,
        "0000_init",
        "CREATE TABLE t:books (id TEXT PRIMARY KEY, x TEXT)",
    );
    assert!(matches!(migrate(&s), Err(MigrationError::Changed { .. })));
}

#[test]
fn a_migration_cannot_build_a_value_larger_than_an_answer() {
    let s = setup();
    add(
        &s,
        0,
        "0000_huge",
        "CREATE TABLE t:books (id TEXT PRIMARY KEY, data BLOB);--> statement-breakpoint\n\
         INSERT INTO t:books (id, data) VALUES ('a', zeroblob(999999999))",
    );
    assert!(matches!(migrate(&s), Err(MigrationError::Failed { .. })));
    assert!(!table_exists(&s, "t:books"), "rolled back");
}

#[test]
fn a_refused_or_failing_migration_applies_nothing() {
    let s = setup();
    add(
        &s,
        0,
        "0000_bad",
        "CREATE TABLE t:books (id TEXT PRIMARY KEY);--> statement-breakpoint\nCREATE VIEW t:v AS SELECT 1",
    );
    assert!(matches!(migrate(&s), Err(MigrationError::Refused { .. })));
    assert!(!table_exists(&s, "t:books"));

    let s = setup();
    add(
        &s,
        0,
        "0000_fails",
        "CREATE TABLE t:books (id TEXT PRIMARY KEY);--> statement-breakpoint\nCREATE TABLE t:books (id TEXT)",
    );
    assert!(matches!(migrate(&s), Err(MigrationError::Failed { .. })));
    assert!(!table_exists(&s, "t:books"), "rolled back as a whole");
    assert_eq!(
        s.count(&format!("SELECT count(*) FROM {MIGRATION_JOURNAL}")),
        0
    );
}

#[test]
fn a_rebuild_keeps_the_rows_and_their_hlcs() {
    let s = setup();
    add(
        &s,
        0,
        "0000_init",
        "CREATE TABLE t:books (id TEXT PRIMARY KEY, title TEXT)",
    );
    migrate(&s).unwrap();
    s.sql(
        "INSERT INTO t:books (id, title) VALUES ('b1', 'x')",
        serde_json::json!([]),
    )
    .unwrap();
    let hlc = |s: &Setup| -> String {
        let sql = t("SELECT haex_hlc_no_sync FROM t:books WHERE id = 'b1'");
        s.db.with_connection(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
            .unwrap()
    };
    let before = hlc(&s);
    add(
        &s,
        1,
        "0001_rebuild",
        "PRAGMA foreign_keys=OFF;--> statement-breakpoint
CREATE TABLE `__new_t:books` (`id` text PRIMARY KEY NOT NULL, `title` text NOT NULL DEFAULT '');--> statement-breakpoint
INSERT INTO `__new_t:books`(\"id\", \"title\") SELECT \"id\", \"title\" FROM `t:books`;--> statement-breakpoint
DROP TABLE `t:books`;--> statement-breakpoint
ALTER TABLE `__new_t:books` RENAME TO `t:books`;--> statement-breakpoint
PRAGMA foreign_keys=ON;",
    );
    migrate(&s).unwrap();
    assert_eq!(
        hlc(&s),
        before,
        "the rebuild copied the row without stamping it anew"
    );
    assert_eq!(
        s.count("SELECT count(*) FROM sqlite_master WHERE type = 'trigger' AND name LIKE 'z_dirty_%books%'"),
        3
    );
}

#[test]
fn the_migration_authorizer_alone_refuses_what_the_rules_refuse() {
    use super::super::authorizer::{migration, MigrationPhase, PhaseCell};
    let s = setup();
    for (phase, sql) in [
        (MigrationPhase::Schema, "CREATE TABLE chat_copy (id TEXT)"),
        (MigrationPhase::Schema, "CREATE VIEW t:v AS SELECT 1"),
        (MigrationPhase::Schema, "CREATE TRIGGER t:tr AFTER INSERT ON t:pages BEGIN SELECT 1; END"),
        (MigrationPhase::Schema, "ALTER TABLE chat_threads ADD COLUMN x TEXT"),
        (MigrationPhase::Schema, "DROP TABLE chat_threads"),
        (MigrationPhase::Data, "INSERT INTO chat_threads (id) VALUES ('x')"),
        (MigrationPhase::Data, "SELECT * FROM sqlite_master"),
        (MigrationPhase::Data, "UPDATE haex_crdt_configs_no_sync SET value = 'x'"),
        (
            MigrationPhase::Data,
            "INSERT INTO extension_migrations_applied_no_sync (extension_id, name, sql_sha256, applied_at) VALUES ('a', 'b', 'c', 1)",
        ),
        (MigrationPhase::Journal, "INSERT INTO chat_threads (id) VALUES ('x')"),
    ] {
        let cell = Arc::new(PhaseCell::default());
        cell.set(phase);
        let guard = SqlGuard {
            authorizer: migration(own(), Arc::clone(&cell)),
            progress: None,
            max_value_bytes: None,
        };
        let text = t(sql);
        let result = s.vault.write_guarded_blocking(
            &guard,
            GuardedWriteOptions {
                schema_mode: true,
                local: false,
            },
            move |tx| tx.execute(&text, &[]).map(drop),
        );
        assert!(result.is_err(), "accepted in {phase:?}: {sql}");
    }
}
