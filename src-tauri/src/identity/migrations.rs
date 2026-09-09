//! holzi-owned migrations shipped via `MigrationSource`.
//!
//! Contract: `tauri-commands.md` §"holzi-owned data layer". The MVP ships the
//! first two tables (`vault_identity`, `known_devices`); further tables named
//! in the plan (`providers`, `models`, `chat_threads`, …) land in later
//! slices onto the already-CRDT-tracked column layout, so the deferred sync
//! layer needs no migration over populated rows.
//!
//! Both tables are plain CRDT-tracked (no `_no_sync` suffix). haex-crdt's
//! `CrdtTransformer` adds the three metadata columns
//! (`haex_hlc_no_sync`, `haex_column_hlcs_no_sync`, `haex_column_sigs_no_sync`)
//! during migration, and `ensure_triggers_initialized` inside `Database::open`
//! installs the triggers — holzi does NOT call `install_crdt` (Etappe-0
//! finding: the transformer + trigger installer cover every non-`_no_sync`
//! table automatically).

use std::collections::BTreeMap;
use std::sync::Arc;

use haex_crdt::{MigrationName, StaticMigrationSource};

/// Returns the frozen holzi migration set at the pinned haex-crdt revision.
pub fn holzi_migration_source() -> Arc<StaticMigrationSource> {
    let mut m: BTreeMap<MigrationName, String> = BTreeMap::new();

    // vault_identity — singleton row inserted at Genesis by the bootstrap
    // hook. Never mutated after Genesis; copied along with the .db, so
    // every replica of the same vault shares the row by construction.
    m.insert(
        MigrationName::from("0001_vault_identity"),
        "CREATE TABLE vault_identity (\
            id INTEGER PRIMARY KEY CHECK (id = 1), \
            pubkey BLOB NOT NULL, \
            privkey BLOB NOT NULL\
         );"
        .to_string(),
    );

    // known_devices — one row per (vault × installation). `installation_uuid`
    // is the immutable primary key carried in CRDT `row_pks`; the further
    // columns are plain CRDT-tracked fields. See contract §"Vault identity
    // and device model" for why installation_uuid is a PK, not a mutable
    // column update.
    m.insert(
        MigrationName::from("0002_known_devices"),
        "CREATE TABLE known_devices (\
            installation_uuid TEXT PRIMARY KEY NOT NULL, \
            vault_device_uuid TEXT NOT NULL UNIQUE, \
            alias TEXT, \
            first_seen INTEGER NOT NULL\
         );"
        .to_string(),
    );

    Arc::new(StaticMigrationSource(m))
}
