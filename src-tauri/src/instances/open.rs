//! `open_instance` — unlock an existing vault as the process's one active instance.
//!
//! Contract: `tauri-commands.md` §`open_instance`. One app process serves at most one vault
//! session (spec 013 FR-010, ADR 0003): opening or creating while one is already active or a
//! close is under way is refused, checked before any path is resolved. WrongPassphrase and
//! NotFound both surface as generic "open failed" on the frontend (FR-021); the typed
//! discriminator is for logs and telemetry only.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use filetime::{set_file_mtime, FileTime};
use haex_crdt::{rusqlite, Database};
use serde::Deserialize;
use tauri::{AppHandle, Runtime, State};
use ts_rs::TS;

use crate::chat::default_model::start_default_model_preload;
use crate::chat::events::emit_model_load_status;
use crate::chat::session::ChatState;
use crate::error::{HolziError, Result};
use crate::identity::installation_id_path;
use crate::state::{ActiveInstanceHandle, AppState};
use crate::voice::VoiceState;

use super::events::emit_instance_list_changed;
use super::info::InstanceInfo;
use super::lock_retry::{retry_while_locked, OPEN_RETRY_POLL_INTERVAL, OPEN_RETRY_WINDOW};
use super::passphrase::Passphrase;
use super::paths::{
    get_app_local_data, get_instance_path, get_pending_marker_path, validate_instance_name,
};
use super::vault_config::vault_config;

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct OpenInstanceArgs {
    pub name: String,
    #[ts(type = "string")]
    pub passphrase: Passphrase,
}

/// Everything `open_instance` does up to and including publishing the opened vault as the active
/// instance — generic over `R: Runtime`, not the concrete `AppHandle`, so a test can call it with
/// `tauri::test::MockRuntime`'s handle instead of a real, windowed one (spec 013 T053). Returns
/// the vault's own database path, so the command wrapper's tail (mtime, events, preload — all of
/// which need the real running app) does not have to resolve it a second time.
///
/// A refusal from [`crate::vault_gate::VaultGate::ensure_can_open`] is the first thing checked,
/// before any path is even computed (spec 013 FR-010). A failed unlock (wrong passphrase) fails
/// before [`AppState::install`] ever runs, so it never touches the gate: [`VaultGate::begin_session`]
/// only moves `Idle` to `Active` once the database has genuinely opened.
pub async fn open_instance_core<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    chat: &ChatState,
    name: &str,
    passphrase: Passphrase,
) -> Result<PathBuf> {
    state.gate().ensure_can_open()?;
    let _operation = chat.acquire_operation()?;
    validate_instance_name(name)?;

    let db_path = get_instance_path(app, name)?;
    let pending_marker = get_pending_marker_path(&db_path);
    let app_local_data = get_app_local_data(app)?;
    let installation_id_file = installation_id_path(&app_local_data);

    if !db_path.exists() {
        return Err(HolziError::NotFound {
            name: name.to_string(),
        });
    }
    // A pending marker means Genesis for this name never completed —
    // treat the file as non-instance until startup cleanup runs.
    if pending_marker.exists() {
        return Err(HolziError::NotFound {
            name: name.to_string(),
        });
    }

    // Shared, not duplicated: `Passphrase` deliberately has no `Clone` (see its own doc comment),
    // so a retry attempt clones the `Arc` pointer, never the secret bytes — one buffer exists in
    // memory for however many attempts this open takes. `db_path` itself is cloned once here so
    // the retry closure can own its copy while the outer `db_path` still returns below.
    let retry_path = db_path.clone();
    let passphrase = Arc::new(passphrase);
    let token = state.gate().token();
    let candidate = retry_while_locked(
        &token,
        OPEN_RETRY_WINDOW,
        OPEN_RETRY_POLL_INTERVAL,
        move || {
            // The candidate is validated without holding `active_instance`, so a failed unlock
            // leaves the gate untouched (see the doc comment above); this also holds across
            // retries — nothing is published until one attempt actually succeeds.
            let open_path = retry_path.clone();
            let open_installation_id_file = installation_id_file.clone();
            let passphrase = passphrase.clone();
            async move {
                tauri::async_runtime::spawn_blocking(move || {
                    open_existing_database(
                        passphrase.as_str(),
                        &open_path,
                        &open_installation_id_file,
                    )
                })
                .await
                .map_err(|e| HolziError::CrdtInit {
                    reason: format!("database open task failed: {e}"),
                })?
            }
        },
    )
    .await?;

    state.install(
        ActiveInstanceHandle {
            name: name.to_string(),
            database: candidate,
        },
        || {
            *chat.session.lock().unwrap_or_else(|e| e.into_inner()) = None;
            Ok(())
        },
    )?;
    Ok(db_path)
}

/// Opens an encrypted instance and makes it the active application instance.
#[tauri::command]
pub async fn open_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    chat: State<'_, ChatState>,
    voice: State<'_, VoiceState>,
    args: OpenInstanceArgs,
) -> Result<InstanceInfo> {
    let OpenInstanceArgs { name, passphrase } = args;
    let db_path = open_instance_core(&app, &state, &chat, &name, passphrase).await?;

    chat.bump_vault_generation();
    voice.invalidate_whisper_cache().await;
    emit_model_load_status(&app, &chat);

    // Refresh mtime so `list_instances` shows this instance at the top.
    let _ = set_file_mtime(&db_path, FileTime::now());

    let info = InstanceInfo {
        name: name.clone(),
        alias: None,
        last_access: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    };
    emit_instance_list_changed(&app, "opened", Some(name));
    start_default_model_preload(app.clone(), chat.inner().clone());
    Ok(info)
}

/// Opens an existing database with the lifecycle command's runtime configuration.
fn open_existing_database(
    passphrase: &str,
    db_path: &Path,
    installation_id_file: &Path,
) -> Result<Arc<Database>> {
    let config = vault_config(passphrase, db_path, installation_id_file, false);
    match Database::open(config) {
        Ok(db) => Ok(Arc::new(db)),
        Err(e) if is_wrong_passphrase(&e) => Err(HolziError::WrongPassphrase),
        Err(e) if is_locked_elsewhere(&e) => Err(HolziError::VaultAlreadyOpenElsewhere),
        Err(e) => Err(HolziError::from(e)),
    }
}

/// haex-crdt's own `DatabaseError::VaultAlreadyOpenElsewhere { path, reason }` (its `fs2` advisory
/// lock is already held by another process) exists as a distinct variant, but its error boundary
/// collapses every `DatabaseError` into an opaque `Error::Message(String)` before it reaches this
/// crate (`From<DatabaseError> for Error` keeps only the rendered `Display` text) — so, like
/// [`is_wrong_passphrase`] above, the message is classified by its stable text rather than a typed
/// variant that never survives the crossing.
fn is_locked_elsewhere(err: &haex_crdt::Error) -> bool {
    matches!(err, haex_crdt::Error::Message(msg) if msg.contains("already open in another instance"))
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
