//! Applying the migrations of an extension (T060, contracts/sql-policy.md §Migrationen, R8): in
//! the order of `extension_migrations.position`, each checked as a whole and then run with its
//! journal row in one guarded write in schema mode, so a migration applies completely or not at
//! all (FR-034). The journal (`extension_migrations_applied_no_sync`) belongs to this device.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use haex_crdt::rusqlite::params;
use haex_crdt::{GuardedWriteOptions, SqlGuard};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::authorizer::{self, MigrationPhase, PhaseCell, MIGRATION_JOURNAL};
use super::exec::limits_of;
use super::migrate_rules::{plan, Step};
use crate::extensions::default_limits::MIGRATION_TIMEOUT_MS;
use crate::extensions::error::BridgeError;
use crate::extensions::ids::TablePrefix;
use crate::storage::query::Query;
use crate::sync::keys::hex;
use crate::vault_gate::VaultDb;

/// A migration as the registry keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMigration {
    pub name: String,
    pub sql: String,
    pub sql_sha256: String,
}

/// Why the migrations of an extension cannot run; the extension does not start then.
#[derive(Debug, Clone, PartialEq)]
pub enum MigrationError {
    /// A migration of this name was applied with other SQL, or two bundles disagree on it.
    Changed { name: String },
    /// A statement breaks a rule (FR-033); nothing of the migration ran.
    Refused {
        name: String,
        error: Box<BridgeError>,
    },
    /// SQLite failed while running it; everything was rolled back.
    Failed { name: String, message: String },
    /// The effective bundle lacks a migration this device applied (research R11).
    Missing { name: String },
    /// The vault could not be read.
    Unavailable,
}

pub fn sql_sha256(sql: &str) -> String {
    hex(&Sha256::digest(sql.as_bytes()))
}

/// Whether `migrations` (name, SQL) of the effective bundle keep every migration this device
/// applied, with the same SQL (research R11). Only then may the device switch to that bundle.
/// After a confirmed `downgrade` an applied migration the older bundle does not know stays: nothing
/// is taken back (US7-3); one it knows must still have the same SQL.
pub fn check_applied_kept(
    q: &mut impl Query,
    extension_id: Uuid,
    migrations: &[(String, String)],
    downgrade: bool,
) -> Result<(), MigrationError> {
    let applied: Vec<(String, String)> = q
        .query_map(
            &format!("SELECT name, sql_sha256 FROM {MIGRATION_JOURNAL} WHERE extension_id = ?1"),
            &[&extension_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| MigrationError::Unavailable)?;
    let offered: HashMap<&str, String> = migrations
        .iter()
        .map(|(name, sql)| (name.as_str(), sql_sha256(sql)))
        .collect();
    for (name, hash) in applied {
        match offered.get(name.as_str()) {
            Some(offered) if *offered == hash => {}
            Some(_) => return Err(MigrationError::Changed { name }),
            None if downgrade => {}
            None => return Err(MigrationError::Missing { name }),
        }
    }
    Ok(())
}

/// The migrations of `migrations` (name, SQL of the verified effective bundle, in its order) still
/// to apply on this device. Only verified SQL runs (FR-003): the synced `extension_migrations` rows
/// are not read here, and the journal gets the hash of the SQL that ran.
pub fn pending(
    q: &mut impl Query,
    extension_id: Uuid,
    migrations: &[(String, String)],
) -> Result<Vec<StoredMigration>, MigrationError> {
    let applied: HashMap<String, String> = q
        .query_map(
            &format!("SELECT name, sql_sha256 FROM {MIGRATION_JOURNAL} WHERE extension_id = ?1"),
            &[&extension_id.to_string()],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(|_| MigrationError::Unavailable)?
        .into_iter()
        .collect();
    let mut out = Vec::new();
    for (name, sql) in migrations {
        let hash = sql_sha256(sql);
        match applied.get(name) {
            Some(applied) if *applied == hash => {}
            Some(_) => return Err(MigrationError::Changed { name: name.clone() }),
            None => out.push(StoredMigration {
                name: name.clone(),
                sql: sql.clone(),
                sql_sha256: hash,
            }),
        }
    }
    Ok(out)
}

/// The migrations (name, SQL) of an extension removed with "keep data", from the
/// `extension_migrations` rows that stay for it (FR-008): no bundle is left to verify against.
/// A row whose SQL does not match its hash, or two rows of one name with other SQL, stop it.
// ponytail: these rows are synced data without a signature; the migration authorizer still limits
// them to the extension's own prefix. Upgrade path: keep the last bundle's signed `signature.json`
// on removal and check each SQL against its file hash.
pub fn kept_migrations(
    q: &mut impl Query,
    extension_id: Uuid,
) -> Result<Vec<(String, String)>, MigrationError> {
    let rows: Vec<StoredMigration> = q
        .query_map(
            "SELECT name, sql, sql_sha256 FROM extension_migrations \
             WHERE extension_id = ?1 ORDER BY position, name",
            &[&extension_id.to_string()],
            |r| {
                Ok(StoredMigration {
                    name: r.get(0)?,
                    sql: r.get(1)?,
                    sql_sha256: r.get(2)?,
                })
            },
        )
        .map_err(|_| MigrationError::Unavailable)?;
    let mut out: Vec<(String, String)> = Vec::new();
    for row in rows {
        if sql_sha256(&row.sql) != row.sql_sha256 {
            return Err(MigrationError::Changed { name: row.name });
        }
        match out.iter().find(|(name, _)| *name == row.name) {
            Some((_, sql)) if *sql == row.sql => {}
            Some(_) => return Err(MigrationError::Changed { name: row.name }),
            None => out.push((row.name, row.sql)),
        }
    }
    Ok(out)
}

/// Where the tables of a migration live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tables {
    /// Synced vault data, with haex-crdt's columns and triggers.
    Synced,
    /// This device only, without CRDT columns: a development version (US12, research R16).
    DeviceLocal,
}

/// Checks and runs one migration with its journal row in one guarded write in schema mode.
fn apply(
    db: &VaultDb,
    extension_id: Uuid,
    own: &TablePrefix,
    migration: &StoredMigration,
    tables: Tables,
    max_value_bytes: u64,
    now_ms: i64,
) -> Result<(), MigrationError> {
    let steps = plan(&migration.sql, own).map_err(|error| MigrationError::Refused {
        name: migration.name.clone(),
        error: Box::new(error),
    })?;
    let phase = Arc::new(PhaseCell::default());
    // Extension SQL never holds the vault's write lock without a limit; a migration gets more time
    // than a call because it may rebuild a large table.
    let deadline = Instant::now() + Duration::from_millis(MIGRATION_TIMEOUT_MS);
    let guard = SqlGuard {
        authorizer: authorizer::migration(own.clone(), Arc::clone(&phase)),
        progress: Some((1000, Arc::new(move || Instant::now() > deadline))),
        // As at run time: no value or row larger than an answer may be.
        max_value_bytes: Some(usize::try_from(max_value_bytes).unwrap_or(usize::MAX)),
    };
    let record = migration.clone();
    db.write_guarded_blocking(
        &guard,
        GuardedWriteOptions {
            schema_mode: true,
            local: tables == Tables::DeviceLocal,
        },
        move |tx| {
            for step in &steps {
                match step {
                    Step::Schema(sql) => {
                        phase.set(MigrationPhase::Schema);
                        tx.execute(sql, &[])?;
                    }
                    Step::Data(sql) => {
                        phase.set(MigrationPhase::Data);
                        tx.execute(sql, &[])?;
                    }
                    Step::CopyVerbatim(sql) => {
                        phase.set(MigrationPhase::Data);
                        tx.copy_rows_verbatim(sql, &[])?;
                    }
                }
            }
            phase.set(MigrationPhase::Journal);
            tx.execute(
                &format!(
                    "INSERT INTO {MIGRATION_JOURNAL} (extension_id, name, sql_sha256, applied_at) \
                     VALUES (?1, ?2, ?3, ?4)"
                ),
                params![
                    extension_id.to_string(),
                    record.name,
                    record.sql_sha256,
                    now_ms
                ],
            )?;
            phase.set(MigrationPhase::Data);
            Ok(())
        },
    )
    .map_err(|e| MigrationError::Failed {
        name: migration.name.clone(),
        message: e
            .sqlite_error()
            .map_or_else(|| e.to_string(), ToString::to_string),
    })
}

/// Held while migrations are read and applied: two starts at once (two tabs of a session restore)
/// would both see the same migration as pending, and the second run would fail.
static APPLYING: Mutex<()> = Mutex::new(());

/// The lock migrations run under; replaying parked sync groups takes it too, so no group lands
/// between two migrations of an extension (research R10).
pub(crate) fn applying() -> &'static Mutex<()> {
    &APPLYING
}

/// Applies every pending one of `migrations` (verified, see [`pending`]) in order; stops at the
/// first that fails. Returns the names of the migrations applied now. Blocking.
pub fn apply_pending(
    db: &VaultDb,
    extension_id: Uuid,
    own: &TablePrefix,
    migrations: &[(String, String)],
    now_ms: i64,
) -> Result<Vec<String>, MigrationError> {
    let applying = APPLYING.lock().unwrap_or_else(PoisonError::into_inner);
    apply_pending_locked(&applying, db, extension_id, own, migrations, now_ms)
}

/// [`apply_pending`] for a development version: the migrations it registers, with the same rules,
/// create tables in haex-crdt's local mode, so they never sync (US12, research R16). The caller
/// holds [`applying`] and read `own` under it, so an unload cannot come in between.
pub fn apply_pending_local(
    _applying: &MutexGuard<'_, ()>,
    db: &VaultDb,
    extension_id: Uuid,
    own: &TablePrefix,
    migrations: &[(String, String)],
    now_ms: i64,
) -> Result<Vec<String>, MigrationError> {
    run_pending(
        db,
        extension_id,
        own,
        migrations,
        Tables::DeviceLocal,
        now_ms,
    )
}

/// [`apply_pending`] for a caller that already holds [`applying`], so a check it made under the
/// lock still holds when the migrations run.
pub(crate) fn apply_pending_locked(
    _applying: &MutexGuard<'_, ()>,
    db: &VaultDb,
    extension_id: Uuid,
    own: &TablePrefix,
    migrations: &[(String, String)],
    now_ms: i64,
) -> Result<Vec<String>, MigrationError> {
    run_pending(db, extension_id, own, migrations, Tables::Synced, now_ms)
}

/// Applies what of `migrations` is still pending, in order; the caller holds [`applying`].
fn run_pending(
    db: &VaultDb,
    extension_id: Uuid,
    own: &TablePrefix,
    migrations: &[(String, String)],
    tables: Tables,
    now_ms: i64,
) -> Result<Vec<String>, MigrationError> {
    let offered = migrations.to_vec();
    let (pending, limits) = db
        .read_blocking(move |q| {
            Ok((
                pending(q, extension_id, &offered),
                limits_of(q, extension_id)?,
            ))
        })
        .map_err(|_| MigrationError::Unavailable)?;
    let pending = pending?;
    let mut applied = Vec::with_capacity(pending.len());
    for migration in &pending {
        apply(
            db,
            extension_id,
            own,
            migration,
            tables,
            limits.max_response_bytes,
            now_ms,
        )?;
        applied.push(migration.name.clone());
    }
    Ok(applied)
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
