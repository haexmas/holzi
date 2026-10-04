use super::*;
use crate::extensions::sql::test_support::{own, t};

fn steps(migration: &str) -> Result<Vec<Step>, BridgeError> {
    plan(&t(migration), &own())
}

#[test]
fn a_drizzle_migration_with_a_rebuild_is_planned_step_by_step() {
    let migration = "PRAGMA foreign_keys=OFF;--> statement-breakpoint
CREATE TABLE `__new_t:tags` (`id` text PRIMARY KEY NOT NULL, `name` text, `created_at` integer DEFAULT (unixepoch()));
--> statement-breakpoint
INSERT INTO `__new_t:tags`(\"id\", \"name\") SELECT \"id\", \"name\" FROM `t:tags`;--> statement-breakpoint
DROP TABLE `t:tags`;--> statement-breakpoint
ALTER TABLE `__new_t:tags` RENAME TO `t:tags`;--> statement-breakpoint
CREATE UNIQUE INDEX `t:tags_name_unique` ON `t:tags` (`name`);--> statement-breakpoint
PRAGMA foreign_keys=ON;";
    let planned = steps(migration).map_err(|e| format!("{e:?}")).unwrap();
    assert_eq!(planned.len(), 5, "the pragmas only switch the schema mode");
    assert!(matches!(planned[0], Step::Schema(_)));
    assert!(matches!(planned[1], Step::CopyVerbatim(_)));
    assert!(matches!(planned[2], Step::Schema(_)));
}

#[test]
fn own_tables_references_columns_and_data_are_allowed() {
    for ok in [
        "CREATE TABLE t:pages (id TEXT PRIMARY KEY, book TEXT REFERENCES t:books(id))",
        "CREATE TABLE t:a (id TEXT, FOREIGN KEY (id) REFERENCES t:b(id) ON DELETE CASCADE)",
        "CREATE TABLE t:cache_no_sync (k TEXT PRIMARY KEY)",
        "ALTER TABLE t:pages ADD COLUMN orientation TEXT DEFAULT 'portrait' NOT NULL",
        "ALTER TABLE t:pages DROP COLUMN orientation",
        "ALTER TABLE t:pages RENAME COLUMN body TO text",
        "DROP INDEX IF EXISTS t:pages_idx",
        "INSERT INTO t:settings (id) VALUES ('default')",
        "UPDATE t:pages SET body = '' WHERE body IS NULL",
        "SELECT 1",
    ] {
        assert!(steps(ok).is_ok(), "{ok}: {:?}", steps(ok));
    }
}

#[test]
fn forbidden_migration_statements_refuse_the_whole_migration() {
    for bad in [
        "CREATE VIEW t:v AS SELECT * FROM t:pages",
        "CREATE TRIGGER t:tr AFTER INSERT ON t:pages BEGIN SELECT 1; END",
        "CREATE VIRTUAL TABLE t:fts USING fts5(body)",
        "CREATE TEMP TABLE t:tmp (id TEXT)",
        "CREATE TABLE t:copy AS SELECT * FROM t:pages",
        "CREATE TABLE t:x_no_sync (id TEXT, haex_hlc_no_sync TEXT)",
        "CREATE TABLE t:x (id TEXT REFERENCES chat_threads(id))",
        "CREATE TABLE chat_copy (id TEXT)",
        "CREATE TABLE t:x__y (id TEXT)",
        "CREATE INDEX idx_pages ON t:pages (id)",
        "CREATE INDEX t:i ON chat_threads (id)",
        "ALTER TABLE chat_threads ADD COLUMN x TEXT",
        "ALTER TABLE t:pages RENAME TO t:pages_no_sync",
        "DROP TABLE chat_threads",
        "PRAGMA writable_schema = ON",
        "ATTACH 'x.db' AS x",
        "VACUUM",
        "BEGIN",
        "INSERT INTO chat_threads (id) VALUES ('x')",
        "UPDATE haex_crdt_configs_no_sync SET value = 'x'",
        "INSERT INTO t:pages (id) SELECT id FROM chat_threads",
        format!("SELECT * FROM {}__cal__events", "b".repeat(64)).leak(),
        // Another extension's tables, whatever it granted (US6).
        format!("CREATE TABLE {}__cal__extra (id TEXT)", "b".repeat(64)).leak(),
        format!(
            "ALTER TABLE {}__cal__events ADD COLUMN x TEXT",
            "b".repeat(64)
        )
        .leak(),
        format!("DROP TABLE {}__cal__events", "b".repeat(64)).leak(),
        format!("CREATE INDEX t:i ON {}__cal__events (id)", "b".repeat(64)).leak(),
        format!(
            "INSERT INTO {}__cal__events (id) VALUES ('x')",
            "b".repeat(64)
        )
        .leak(),
        format!(
            "INSERT INTO t:pages (id) SELECT id FROM {}__cal__events",
            "b".repeat(64)
        )
        .leak(),
    ] {
        let migration = format!("CREATE TABLE t:ok (id TEXT);--> statement-breakpoint\n{bad}");
        assert!(steps(&migration).is_err(), "{bad} was accepted");
    }
}
