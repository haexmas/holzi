//! Import commands (spec 034, US7, `contracts/tauri-commands.md` §Import). All call the service as
//! `Caller::User`. The password and key file path travel in a type that prints nothing; the file
//! is read again by every command.

use std::fmt;

use serde::Deserialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, State};
use ts_rs::TS;
use zeroize::Zeroizing;

use super::service;
use crate::error::{HolziError, Result};
use crate::passwords::access::Caller;
use crate::passwords::import::apply::{OnDuplicate, Progress};
use crate::passwords::import::report::render_text;
use crate::passwords::import::ImportSource;
use crate::passwords::model::{ImportPreview, ImportReport};
use crate::passwords::service::import::ImportRequest;
use crate::state::AppState;

const PROGRESS_EVENT: &str = "passwords-import-progress";

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/", optional_fields)]
#[serde(rename_all = "camelCase")]
pub struct ImportArgs {
    pub source: ImportSource,
    pub path: String,
    pub password: Option<String>,
    pub key_file_path: Option<String>,
}

impl fmt::Debug for ImportArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImportArgs")
            .field("source", &self.source)
            .field("credentials", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl From<ImportArgs> for ImportRequest {
    fn from(args: ImportArgs) -> Self {
        ImportRequest {
            source: args.source,
            path: args.path,
            password: args.password.map(Zeroizing::new),
            key_file_path: args.key_file_path.filter(|p| !p.is_empty()),
        }
    }
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/", optional_fields)]
#[serde(rename_all = "camelCase")]
pub struct ImportRunArgs {
    pub source: ImportSource,
    pub path: String,
    pub password: Option<String>,
    pub key_file_path: Option<String>,
    pub on_duplicate: OnDuplicate,
}

impl fmt::Debug for ImportRunArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImportRunArgs")
            .field("source", &self.source)
            .field("credentials", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ReportSaveArgs {
    pub report: ImportReport,
    /// A path from the save dialog.
    pub path: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct IconPreviewArgs {
    pub hash: String,
}

/// Counts what the file would bring over; writes nothing.
#[tauri::command]
pub async fn passwords_import_preview(
    state: State<'_, AppState>,
    args: ImportArgs,
) -> Result<ImportPreview> {
    service(&state)?
        .import_preview(&Caller::User, args.into())
        .await
}

/// Writes the file in steps and returns the report; a cancel or a fatal error removes what was
/// written and fails (`ImportFailed { reason: "cancelled" }` for a cancel).
#[tauri::command]
pub async fn passwords_import_run(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ImportRunArgs,
) -> Result<ImportReport> {
    let service = service(&state)?;
    let guard = state.password_import().begin()?;
    let on_duplicate = args.on_duplicate;
    let request = ImportRequest {
        source: args.source,
        path: args.path,
        password: args.password.map(Zeroizing::new),
        key_file_path: args.key_file_path.filter(|p| !p.is_empty()),
    };
    let emit = move |progress: Progress| {
        if let Err(e) = app.emit(PROGRESS_EVENT, progress) {
            log::warn!("emit {PROGRESS_EVENT} failed: {e}");
        }
    };
    service
        .import_run(&Caller::User, request, on_duplicate, guard.flag(), &emit)
        .await
}

/// Stops the running import and undoes what it wrote.
#[tauri::command]
pub async fn passwords_import_cancel(state: State<'_, AppState>) -> Result<()> {
    state.password_import().cancel();
    Ok(())
}

/// Writes the report as text to the chosen path: titles, folder paths, kinds, field and file names
/// and sizes, never a value of a secret.
#[tauri::command]
pub async fn passwords_import_report_save(args: ReportSaveArgs) -> Result<()> {
    let text = render_text(&args.report);
    let path = std::path::PathBuf::from(args.path);
    tauri::async_runtime::spawn_blocking(move || std::fs::write(path, text))
        .await
        .map_err(|_| HolziError::PasswordsImportFailed {
            reason: "unwritable".to_string(),
        })?
        .map_err(|_| HolziError::PasswordsImportFailed {
            reason: "unwritable".to_string(),
        })
}

/// The bytes of an imported picture; anything that is not a stored picture is `NotFound`.
#[tauri::command]
pub async fn passwords_icon_preview(
    state: State<'_, AppState>,
    args: IconPreviewArgs,
) -> Result<Response> {
    let bytes = service(&state)?
        .icon_preview(&Caller::User, args.hash)
        .await?;
    Ok(Response::new(bytes))
}
