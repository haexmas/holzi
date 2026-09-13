//! `create_instance` — Genesis flow with `.pending` marker + rollback.
//!
//! Contract: `tauri-commands.md` §`create_instance`. Marker precedes the
//! DB so a crash between marker creation and marker removal is detectable
//! by `startup::cleanup_orphans_on_startup`.

use std::path::Path;
use std::sync::Arc;

use haex_crdt::Database;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::chat::session::ChatState;
use crate::error::{HolziError, Result};
use crate::identity::installation_id_path;
use crate::state::{ActiveInstanceHandle, AppState};

use super::events::emit_instance_list_changed;
use super::info::InstanceInfo;
use super::paths::{
    get_app_local_data, get_instance_path, get_pending_marker_path, validate_instance_name,
};
use super::vault_config::vault_config;

/// Minimum passphrase length. Kept low for the MVP; can be tightened
/// later without a wire-contract change.
const MIN_PASSPHRASE_LEN: usize = 8;

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct CreateInstanceArgs {
    pub name: String,
    pub passphrase: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct CreateInstanceResult {
    pub info: InstanceInfo,
}

/// Creates, initializes, and publishes a new encrypted instance.
#[tauri::command]
pub async fn create_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    chat: State<'_, ChatState>,
    args: CreateInstanceArgs,
) -> Result<CreateInstanceResult> {
    let _operation = chat.acquire_operation()?;
    validate_instance_name(&args.name)?;
    if args.passphrase.len() < MIN_PASSPHRASE_LEN {
        return Err(HolziError::WeakPassphrase {
            reason: format!("passphrase must be at least {MIN_PASSPHRASE_LEN} characters"),
        });
    }

    // Refuse if another instance is currently active — one active
    // instance per process (FR-022).
    {
        let guard = state
            .active_instance
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("active_instance mutex poisoned: {e}"),
            })?;
        if guard.is_some() {
            return Err(HolziError::InstanceAlreadyActive);
        }
    }

    let db_path = get_instance_path(&app, &args.name)?;
    let pending_marker = get_pending_marker_path(&db_path);
    let app_local_data = get_app_local_data(&app)?;
    let installation_id_file = installation_id_path(&app_local_data);

    if db_path.exists() {
        return Err(HolziError::NameConflict {
            name: args.name.clone(),
        });
    }

    // Marker is atomic (`create_new`) — a parallel create in flight
    // fails with AlreadyExists here, before any Database::open work.
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&pending_marker)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => HolziError::NameConflict {
                name: args.name.clone(),
            },
            _ => HolziError::from(e),
        })?;

    // From here on, any early return MUST clean up the marker + .db.
    let passphrase = args.passphrase.clone();
    let open_path = db_path.clone();
    let open_installation_id_file = installation_id_file.clone();
    let open_result = match tauri::async_runtime::spawn_blocking(move || {
        open_new_database(&passphrase, &open_path, &open_installation_id_file)
    })
    .await
    {
        Ok(result) => result,
        Err(e) => {
            let _ = std::fs::remove_file(&db_path);
            let _ = std::fs::remove_file(&pending_marker);
            return Err(HolziError::CrdtInit {
                reason: format!("database open task failed: {e}"),
            });
        }
    };

    match open_result {
        Ok(db_arc) => match publish_active(&state, &args.name, &db_arc, &pending_marker) {
            Ok(result) => {
                *chat.session.lock().unwrap_or_else(|e| e.into_inner()) = None;
                emit_instance_list_changed(&app, "created", Some(args.name.clone()));
                Ok(result)
            }
            Err(e) => {
                drop(db_arc);
                let _ = std::fs::remove_file(&db_path);
                let _ = std::fs::remove_file(&pending_marker);
                Err(e)
            }
        },
        Err(e) => {
            let _ = std::fs::remove_file(&db_path);
            let _ = std::fs::remove_file(&pending_marker);
            Err(e)
        }
    }
}

/// Publishes a newly opened database as the active instance.
fn publish_active(
    state: &AppState,
    name: &str,
    db_arc: &Arc<Database>,
    pending_marker: &Path,
) -> Result<CreateInstanceResult> {
    {
        let mut guard = state
            .active_instance
            .lock()
            .map_err(|e| HolziError::CrdtInit {
                reason: format!("active_instance mutex poisoned during publish: {e}"),
            })?;
        if guard.is_some() {
            return Err(HolziError::InstanceAlreadyActive);
        }
        // Publication commits Genesis: a leftover marker would cause startup
        // cleanup to delete this vault, so removal must succeed first.
        std::fs::remove_file(pending_marker)?;
        *guard = Some(ActiveInstanceHandle {
            name: name.to_string(),
            database: Arc::clone(db_arc),
        });
    }

    let info = InstanceInfo {
        name: name.to_string(),
        alias: None,
        last_access: now_millis(),
    };
    Ok(CreateInstanceResult { info })
}

/// Opens a new database with the lifecycle command's bootstrap configuration.
fn open_new_database(
    passphrase: &str,
    db_path: &Path,
    installation_id_file: &Path,
) -> Result<Arc<Database>> {
    let config = vault_config(passphrase, db_path, installation_id_file, true);
    Ok(Arc::new(Database::open(config)?))
}

/// Returns the current UNIX timestamp in milliseconds.
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_is_not_published_if_pending_marker_cannot_be_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let db = open_new_database(
            "test-passphrase",
            &tmp.path().join("vault.db"),
            &installation_id_path(tmp.path()),
        )
        .unwrap();
        let marker = tmp.path().join("vault.db.pending");
        std::fs::create_dir(&marker).unwrap(); // remove_file deterministically fails
        let state = AppState::new();
        assert!(publish_active(&state, "vault", &db, &marker).is_err());
        assert!(state.active_instance.lock().unwrap().is_none());
    }

    #[test]
    fn published_genesis_survives_startup_cleanup() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vault.db");
        let db =
            open_new_database("test-passphrase", &path, &installation_id_path(tmp.path())).unwrap();
        let marker = get_pending_marker_path(&path);
        std::fs::write(&marker, "").unwrap();
        let state = AppState::new();
        publish_active(&state, "vault", &db, &marker).unwrap();
        assert!(!marker.exists());
        assert_eq!(
            super::super::startup::cleanup_orphans_in_dir(tmp.path()).unwrap(),
            0
        );
        assert!(path.is_file());
    }
}
