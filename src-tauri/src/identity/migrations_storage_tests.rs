//! Migration tests for `0028_storage_connections` (spec 038, data-model.md, research R3): a genesis
//! vault and a vault upgraded from the schema before it both end with the three tables, the
//! connections and storages tracked by haex-crdt (they reach every own device, FR-006) and the test
//! results not, and no synced table with a UNIQUE constraint.

// These tests read `sqlite_master` and `pragma_*` directly, which the CRDT write path does not
// expose.
#![allow(clippy::disallowed_methods)]

use haex_crdt::Database;

use super::migrations_extensions_tests::{has_hlc_column, unique_indexes};
use super::migrations_storage::{DEVICE_TABLES, SYNCED_TABLES};
use super::migrations_tests::{migration_source_before, open, own_columns, table_exists};
use crate::identity::holzi_migration_source;

const COLUMNS: [(&str, &[&str]); 3] = [
    (
        "haex_storage_connections",
        &[
            "id",
            "provider_name",
            "provider_kind",
            "endpoint",
            "endpoint_scope",
            "region",
            "addressing",
            "credentials_item_id",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "haex_storages",
        &[
            "id",
            "connection_id",
            "name",
            "bucket",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "storage_tests_no_sync",
        &["storage_id", "tested_at", "outcome"],
    ),
];

fn assert_storage_schema(db: &Database) {
    assert_eq!(
        SYNCED_TABLES.len() + DEVICE_TABLES.len(),
        COLUMNS.len(),
        "every table of 0028 is listed once"
    );
    for (table, columns) in COLUMNS {
        assert!(table_exists(db, table), "{table} must exist after 0028");
        assert_eq!(own_columns(db, table), columns, "columns of {table}");
    }
    for table in SYNCED_TABLES {
        assert!(
            has_hlc_column(db, table),
            "{table} is vault-wide, so haex-crdt must track it"
        );
        assert_eq!(
            unique_indexes(db, table),
            0,
            "{table}: a UNIQUE constraint would halt the sync on a conflict"
        );
    }
    for table in DEVICE_TABLES {
        assert!(
            !has_hlc_column(db, table),
            "{table} is an observation of this device and must not be tracked"
        );
    }
}

#[tokio::test]
async fn migration_0028_gives_a_fresh_vault_the_storage_tables() {
    tokio::task::spawn_blocking(|| {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = open(dir.path(), "storage-fresh", true, holzi_migration_source());
        assert_storage_schema(&db);
    })
    .await
    .expect("join");
}

#[tokio::test]
async fn migration_0028_upgrades_a_vault_from_the_schema_before_it() {
    tokio::task::spawn_blocking(|| {
        let dir = tempfile::tempdir().expect("tempdir");
        let old = open(
            dir.path(),
            "storage-upgrade",
            true,
            migration_source_before("0028_storage_connections"),
        );
        assert!(!table_exists(&old, "haex_storages"));
        drop(old);

        let db = open(
            dir.path(),
            "storage-upgrade",
            false,
            holzi_migration_source(),
        );
        assert_storage_schema(&db);
    })
    .await
    .expect("join");
}
