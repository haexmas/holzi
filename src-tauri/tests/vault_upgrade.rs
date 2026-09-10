//! Regression coverage for CRDT trigger upgrades on an existing vault.
//!
//! haex-crdt bakes the tracked-column list into the `AFTER UPDATE OF
//! <cols>` trigger at install time, and `ensure_triggers_initialized`
//! early-returns when the vault's stored trigger version already equals
//! the one the config passes. A migration that adds a column to a
//! CRDT-tracked table therefore leaves that column *untracked* on every
//! already-provisioned vault unless the trigger version is bumped in
//! lockstep — writes to it never mark the row dirty and never record a
//! per-column HLC, so the value silently fails to reach other devices.
//!
//! Migration `0009_models_add_tokenizer_repo` is exactly that case.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use haex_crdt::{
    Database, DatabaseConfig, MigrationName, NoopSignatureProvider, SqlCipherKey,
    StaticMigrationSource, DEFAULT_TRIGGER_VERSION,
};

use holzi_lib::identity::{holzi_migration_source, installation_id_path, HolziBootstrap};
use holzi_lib::instances::vault_config::vault_config;

const PASSPHRASE: &str = "vault-upgrade-integration-test";
const TOKENIZER_MIGRATION: &str = "0009_models_add_tokenizer_repo";

/// Opens the vault under `dir` with `source` at `trigger_version`.
fn open_vault(
    dir: &Path,
    source: Arc<dyn haex_crdt::MigrationSource>,
    trigger_version: i32,
) -> Database {
    let db_path: PathBuf = dir.join("vault.db");
    let installation_id = installation_id_path(dir);
    Database::open(DatabaseConfig {
        path: db_path,
        key: SqlCipherKey::new(PASSPHRASE),
        create_if_missing: true,
        bootstrap: Arc::new(HolziBootstrap::new(installation_id).with_alias("test")),
        signature_provider: Arc::new(NoopSignatureProvider),
        migration_source: source,
        trigger_version,
    })
    .expect("vault open")
}

/// The production migration set with `0009` removed, standing in for a
/// vault provisioned before that migration shipped.
fn pre_0009_source() -> Arc<StaticMigrationSource> {
    let mut m: BTreeMap<MigrationName, String> = holzi_migration_source().0.clone();
    m.remove(&MigrationName::from(TOKENIZER_MIGRATION))
        .expect("0009 must exist in the production migration set");
    Arc::new(StaticMigrationSource(m))
}

/// The SQL of the `models` UPDATE trigger as currently installed.
fn models_update_trigger_sql(db: &Database) -> String {
    db.with_connection(|conn| {
        let sql: String = conn.query_row(
            "SELECT sql FROM sqlite_master \
             WHERE type = 'trigger' AND name = 'z_dirty_models_update'",
            [],
            |r| r.get(0),
        )?;
        Ok(sql)
    })
    .expect("models update trigger must exist")
}

#[test]
fn reopening_a_pre_0009_vault_tracks_tokenizer_repo() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("vault.db");
    let installation_id = installation_id_path(dir.path());

    // Provision the vault as an older holzi release would have: no 0009,
    // triggers installed against the pre-0009 `models` shape.
    let old = open_vault(dir.path(), pre_0009_source(), DEFAULT_TRIGGER_VERSION);
    assert!(
        !models_update_trigger_sql(&old).contains("tokenizer_repo"),
        "precondition: the pre-0009 trigger must not know the column yet"
    );
    drop(old);

    // Reopen through the *production* config — the same one
    // `open_instance` uses. Migration 0009 adds the column, and the
    // trigger version must be high enough that the triggers are rewritten
    // against the migrated schema.
    let upgraded = Database::open(vault_config(PASSPHRASE, &db_path, &installation_id, false))
        .expect("reopen upgraded vault");
    let sql = models_update_trigger_sql(&upgraded);
    assert!(
        sql.contains("tokenizer_repo"),
        "0009 added `tokenizer_repo` to the CRDT-tracked `models` table, but the \
         production open path left the trigger untracked — writes to it will not \
         sync. Bump HOLZI_TRIGGER_VERSION.\n{sql}"
    );
}

/// The negative control: reopening at the version the vault already
/// stores is precisely what leaves the column untracked. This is the bug
/// the bump fixes, and it documents why the constant must move whenever a
/// migration reshapes a CRDT-tracked table.
#[test]
fn reopening_without_a_version_bump_leaves_tokenizer_repo_untracked() {
    let dir = tempfile::tempdir().expect("tempdir");

    let old = open_vault(dir.path(), pre_0009_source(), DEFAULT_TRIGGER_VERSION);
    drop(old);

    let stale = open_vault(
        dir.path(),
        holzi_migration_source(),
        DEFAULT_TRIGGER_VERSION,
    );
    assert!(
        !models_update_trigger_sql(&stale).contains("tokenizer_repo"),
        "ensure_triggers_initialized is expected to early-return here; if this \
         now passes, haex-crdt changed its upgrade semantics and \
         HOLZI_TRIGGER_VERSION may no longer be needed"
    );
}
