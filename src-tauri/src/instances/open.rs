//! `open_instance` — unlock existing DB + atomic active-instance switch.
//!
//! Contract: `tauri-commands.md` §`open_instance`. The state mutex is
//! held across validate → stop-old → publish-new so no concurrent
//! observer sees a half-open state. WrongPassphrase and NotFound both
//! surface as generic "open failed" on the frontend (FR-021); the typed
//! discriminator is for logs and telemetry only.

use std::path::Path;
use std::sync::Arc;

use filetime::{set_file_mtime, FileTime};
use haex_crdt::{
    rusqlite, Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey,
    DEFAULT_TRIGGER_VERSION,
};
use serde::Deserialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::error::{HolziError, Result};
use crate::identity::{holzi_migration_source, installation_id_path, HolziBootstrap};
use crate::state::{ActiveInstanceHandle, AppState};

use super::events::emit_instance_list_changed;
use super::info::InstanceInfo;
use super::paths::{
    get_app_local_data, get_instance_path, get_pending_marker_path, validate_instance_name,
};

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct OpenInstanceArgs {
    pub name: String,
    pub passphrase: String,
}

/// Opens an encrypted instance and makes it the active application instance.
#[tauri::command]
pub async fn open_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    args: OpenInstanceArgs,
) -> Result<InstanceInfo> {
    validate_instance_name(&args.name)?;

    let db_path = get_instance_path(&app, &args.name)?;
    let pending_marker = get_pending_marker_path(&db_path);
    let app_local_data = get_app_local_data(&app)?;
    let installation_id_file = installation_id_path(&app_local_data);

    if !db_path.exists() {
        return Err(HolziError::NotFound {
            name: args.name.clone(),
        });
    }
    // A pending marker means Genesis for this name never completed —
    // treat the file as non-instance until startup cleanup runs.
    if pending_marker.exists() {
        return Err(HolziError::NotFound {
            name: args.name.clone(),
        });
    }

    // Database::open acquires the active database's advisory lock, so an
    // already-active request for the same name needs a credential check that
    // does not mount a second Database handle.
    let active_database = {
        let guard = state
            .active_instance
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("active_instance mutex poisoned: {e}"),
            })?;
        guard
            .as_ref()
            .filter(|active| active.name == args.name)
            .map(|active| Arc::clone(&active.database))
    };

    if let Some(_active_database) = active_database {
        let passphrase = args.passphrase.clone();
        let validation_path = db_path.clone();
        let validation = tauri::async_runtime::spawn_blocking(move || {
            let connection = rusqlite::Connection::open_with_flags(
                &validation_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            );
            let result = match connection {
                Ok(connection) => {
                    connection
                        .pragma_update(None, "key", passphrase)
                        .and_then(|()| {
                            connection.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
                                row.get::<_, i64>(0)
                            })
                        })
                }
                Err(e) => Err(e),
            };

            match result {
                Ok(_) => Ok(()),
                Err(e)
                    if matches!(
                        &e,
                        rusqlite::Error::SqliteFailure(code, _)
                            if code.code == rusqlite::ErrorCode::NotADatabase
                    ) || e.to_string().to_lowercase().contains("not a database") =>
                {
                    Err(HolziError::WrongPassphrase)
                }
                Err(e) => Err(HolziError::CrdtSqlite {
                    reason: e.to_string(),
                }),
            }
        })
        .await
        .map_err(|e| HolziError::CrdtInit {
            reason: format!("passphrase validation task failed: {e}"),
        })?;
        validation?;

        // The active slot may have changed while the read-only validation ran.
        // Only return the existing instance if it is still the requested one.
        let guard = state
            .active_instance
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("active_instance mutex poisoned: {e}"),
            })?;
        if guard
            .as_ref()
            .is_some_and(|active| active.name == args.name)
        {
            drop(guard);
            let _ = set_file_mtime(&db_path, FileTime::now());
            let info = InstanceInfo {
                name: args.name.clone(),
                alias: None,
                last_access: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
            };
            emit_instance_list_changed(&app, "opened", Some(args.name.clone()));
            return Ok(info);
        }
    }

    // Open the candidate without holding active_instance. Failure to open the
    // candidate leaves the previous runtime intact (contract postcondition).
    let passphrase = args.passphrase.clone();
    let open_path = db_path.clone();
    let open_installation_id_file = installation_id_file.clone();
    let candidate_result = tauri::async_runtime::spawn_blocking(move || {
        open_existing_database(&passphrase, &open_path, &open_installation_id_file)
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("database open task failed: {e}"),
    })?;
    let candidate = candidate_result?;

    // Hold the state lock only for the atomic old-runtime drop/new-runtime
    // publish. The blocking SQLCipher open above cannot stall observers.
    let mut guard = state
        .active_instance
        .lock()
        .map_err(|e| HolziError::CrdtInit {
            reason: format!("active_instance mutex poisoned: {e}"),
        })?;

    // Candidate is up — drop the previous handle (releases fs2 lock)
    // and publish the new one atomically.
    let previous = guard.take();
    if let Some(prev) = previous {
        // The previous handle's Arc drops when `prev` goes out of scope.
        // A rare failure to release the fs2 lock here (subsystem still
        // holding a clone) would let both Arcs live on — acceptable for
        // MVP, tightens later once background tasks (relay, iroh) enter.
        drop(prev);
    }
    *guard = Some(ActiveInstanceHandle {
        name: args.name.clone(),
        database: candidate,
    });
    drop(guard);

    // Refresh mtime so `list_instances` shows this instance at the top.
    let _ = set_file_mtime(&db_path, FileTime::now());

    let info = InstanceInfo {
        name: args.name.clone(),
        alias: None,
        last_access: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    };
    emit_instance_list_changed(&app, "opened", Some(args.name.clone()));
    Ok(info)
}

/// Opens an existing database with the lifecycle command's runtime configuration.
fn open_existing_database(
    passphrase: &str,
    db_path: &Path,
    installation_id_file: &Path,
) -> Result<Arc<Database>> {
    let config = DatabaseConfig {
        path: db_path.to_path_buf(),
        key: SqlCipherKey::new(passphrase),
        create_if_missing: false,
        bootstrap: Arc::new(HolziBootstrap::new(installation_id_file.to_path_buf())),
        signature_provider: Arc::new(NoopSignatureProvider),
        migration_source: holzi_migration_source(),
        trigger_version: DEFAULT_TRIGGER_VERSION,
    };
    match Database::open(config) {
        Ok(db) => Ok(Arc::new(db)),
        Err(e) if is_wrong_passphrase(&e) => Err(HolziError::WrongPassphrase),
        Err(e) => Err(HolziError::from(e)),
    }
}

/// SQLCipher rejects a bad key by reporting `SQLITE_NOTADB` (`code = 26`)
/// from the first page read. We can't reach that as a typed variant
/// because rusqlite wraps it inside `Error::SqliteFailure`, so pattern-
/// match the primary error code. Fallback string-check catches the same
/// condition when the error was massaged by an intermediate layer.
fn is_wrong_passphrase(err: &haex_crdt::Error) -> bool {
    if let haex_crdt::Error::Sqlite(rusqlite::Error::SqliteFailure(code, _)) = err {
        if code.code == rusqlite::ErrorCode::NotADatabase {
            return true;
        }
    }
    err.to_string().to_lowercase().contains("not a database")
}
