//! Applying the migrations of an extension (T060, contracts/sql-policy.md §Migrationen, R8): in
//! the order of `extension_migrations.position`, each checked as a whole and then run with its
//! journal row in one guarded write in schema mode, so a migration applies completely or not at
//! all (FR-034). The journal (`extension_migrations_applied_no_sync`) belongs to this device.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
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
pub fn check_applied_kept(
    q: &mut impl Query,
    extension_id: Uuid,
    migrations: &[(String, String)],
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
            None => return Err(MigrationError::Missing { name }),
        }
    }
    Ok(())
}

/// The migrations still to apply on this device, in order.
pub fn pending(
    q: &mut impl Query,
    extension_id: Uuid,
) -> Result<Vec<StoredMigration>, MigrationError> {
    let ext = extension_id.to_string();
    let all = q
        .query_map(
            "SELECT name, sql, sql_sha256 FROM extension_migrations \
             WHERE extension_id = ?1 ORDER BY position, name",
            &[&ext],
            |r| {
                Ok(StoredMigration {
                    name: r.get(0)?,
                    sql: r.get(1)?,
                    sql_sha256: r.get(2)?,
                })
            },
        )
        .map_err(|_| MigrationError::Unavailable)?;
    let applied: HashMap<String, String> = q
        .query_map(
            &format!("SELECT name, sql_sha256 FROM {MIGRATION_JOURNAL} WHERE extension_id = ?1"),
            &[&ext],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(|_| MigrationError::Unavailable)?
        .into_iter()
        .collect();

    let mut seen: HashMap<&str, &str> = HashMap::new();
    for migration in &all {
        if let Some(other) = seen.insert(&migration.name, &migration.sql_sha256) {
            if other != migration.sql_sha256 {
                return Err(MigrationError::Changed {
                    name: migration.name.clone(),
                });
            }
        }
    }
    let mut out = Vec::new();
    for migration in all {
        match applied.get(&migration.name) {
            Some(hash) if *hash == migration.sql_sha256 => {}
            Some(_) => {
                return Err(MigrationError::Changed {
                    name: migration.name,
                })
            }
            None if out
                .iter()
                .any(|m: &StoredMigration| m.name == migration.name) => {}
            None => out.push(migration),
        }
    }
    Ok(out)
}

/// Checks and runs one migration with its journal row in one guarded write in schema mode.
fn apply(
    db: &VaultDb,
    extension_id: Uuid,
    own: &TablePrefix,
    migration: &StoredMigration,
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
            local: false,
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

/// Applies every pending migration in order; stops at the first that fails. Returns the names of
/// the migrations applied now. Blocking.
pub fn apply_pending(
    db: &VaultDb,
    extension_id: Uuid,
    own: &TablePrefix,
    now_ms: i64,
) -> Result<Vec<String>, MigrationError> {
    let _applying = APPLYING.lock().unwrap_or_else(PoisonError::into_inner);
    let (pending, limits) = db
        .read_blocking(move |q| Ok((pending(q, extension_id), limits_of(q, extension_id)?)))
        .map_err(|_| MigrationError::Unavailable)?;
    let pending = pending?;
    let mut applied = Vec::with_capacity(pending.len());
    for migration in &pending {
        apply(
            db,
            extension_id,
            own,
            migration,
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
