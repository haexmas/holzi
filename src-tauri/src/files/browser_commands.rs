//! Tauri commands of the file browser's window (spec 044, contracts/tauri-commands.md). Every
//! command acts as [`Caller::User`]; paths are resolved and checked in Rust before anything is
//! read. Storages (US5) answer `unsupported` until their source is built.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::files::access::{check, AgentGrants, Target, Verdict, Want};
use crate::files::kind::{viewer_kind, ViewerKind};
use crate::files::local::drives::{drives, Drive};
use crate::files::local::text::{read_text, TextContent, TEXT_LIMIT};
use crate::files::local::{edit, ops, resolve, resolve_entry};
use crate::files::media::MediaServer;
use crate::files::state::FilesState;
use crate::files::streaming::LocalFileSource;
use crate::files::transfer::local::{prepare, touches_own, Job};
use crate::files::transfer::{
    platform_removal, same_device, ConflictChoice, TransferEvent, TransferManager, TransferOp,
};
use crate::files::{thumbnails, Entry, EntryKind, FilesError, FilesErrorCode, SourceRef};
use crate::passwords::access::Caller;
use crate::state::AppState;

/// A known place for the sidebar.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct KnownPlace {
    pub name: String,
    pub path: String,
}

/// What the sidebar shows.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct Sources {
    pub drives: Vec<Drive>,
    pub known: Vec<KnownPlace>,
}

/// A change in a watched folder; the window reloads it.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct FolderChanged {
    pub paths: Vec<String>,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, FilesError> + Send + 'static,
) -> Result<T, FilesError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| {
            log::error!("files: a blocking task failed: {error}");
            Err(FilesError::new(
                FilesErrorCode::NotFound,
                "the operation failed",
            ))
        })
}

/// The resolved device path the user may `want`, or why not.
fn device_path(
    files: &FilesState,
    source: &SourceRef,
    path: &str,
    want: Want,
) -> Result<PathBuf, FilesError> {
    checked(files, source, path, want, resolve)
}

/// Like [`device_path`] for an entry itself: a link stays the link ([`resolve_entry`]).
fn device_entry(
    files: &FilesState,
    source: &SourceRef,
    path: &str,
    want: Want,
) -> Result<PathBuf, FilesError> {
    checked(files, source, path, want, resolve_entry)
}

fn checked(
    files: &FilesState,
    source: &SourceRef,
    path: &str,
    want: Want,
    resolver: fn(&Path) -> Result<PathBuf, FilesError>,
) -> Result<PathBuf, FilesError> {
    if !matches!(source, SourceRef::Device) {
        return Err(FilesError::new(
            FilesErrorCode::Unsupported,
            "storages come with a later version",
        ));
    }
    let real = resolver(Path::new(path))?;
    match check(
        &Caller::User,
        &Target::Device(real.clone()),
        want,
        &files.own,
        &AgentGrants::default(),
    ) {
        Verdict::Allowed | Verdict::ReadOnly => Ok(real),
        Verdict::Refused(error) => Err(error),
        Verdict::Ask => Err(FilesError::new(FilesErrorCode::NotGranted, "not granted")),
    }
}

/// Drives and known places of this device.
#[tauri::command]
pub async fn files_sources(files: State<'_, FilesState>) -> Result<Sources, FilesError> {
    let known = files
        .known
        .iter()
        .map(|place| KnownPlace {
            name: place.name.to_owned(),
            path: place.path.to_string_lossy().into_owned(),
        })
        .collect();
    let drives = blocking(|| Ok(drives())).await?;
    Ok(Sources { drives, known })
}

/// Every entry of a folder.
#[tauri::command]
pub async fn files_list(
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
) -> Result<Vec<Entry>, FilesError> {
    let real = device_path(&files, &source, &path, Want::Read)?;
    let own = files.own.clone();
    blocking(move || ops::list(&real, &own)).await
}

/// One entry.
#[tauri::command]
pub async fn files_stat(
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
) -> Result<Entry, FilesError> {
    let real = device_path(&files, &source, &path, Want::Read)?;
    let own = files.own.clone();
    blocking(move || ops::stat(&real, &own)).await
}

/// The text of a file for the viewer, at most 5 MB (FR-015).
#[tauri::command]
pub async fn files_read_text(
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
) -> Result<TextContent, FilesError> {
    let real = device_path(&files, &source, &path, Want::Read)?;
    blocking(move || read_text(&real, TEXT_LIMIT)).await
}

/// What the viewer gets for a file (contracts/tauri-commands.md, `files_open`).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct Opened {
    pub kind: ViewerKind,
    /// The media server's URL for image, video, audio and PDF; released with the tab.
    pub url: Option<String>,
    pub entry: Entry,
}

/// Opens a file for the viewer of `tab_id`: images, video, audio and PDF get a URL of the media
/// server (FR-012, FR-016), text is read with [`files_read_text`], the rest is the info view.
#[tauri::command]
pub async fn files_open(
    files: State<'_, FilesState>,
    media: State<'_, MediaServer>,
    source: SourceRef,
    path: String,
    tab_id: String,
) -> Result<Opened, FilesError> {
    let real = device_path(&files, &source, &path, Want::Read)?;
    let own = files.own.clone();
    let stat_path = real.clone();
    let entry = blocking(move || ops::stat(&stat_path, &own)).await?;
    if entry.kind != EntryKind::File {
        return Err(FilesError::invalid_path("not a file"));
    }
    let kind = viewer_kind(&entry.name);
    let url = matches!(
        kind,
        ViewerKind::Image | ViewerKind::Video | ViewerKind::Audio | ViewerKind::Pdf
    )
    .then(|| {
        let mime = entry
            .mime
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_owned());
        media.register(&tab_id, Arc::new(LocalFileSource::new(real, mime)))
    });
    Ok(Opened { kind, url, entry })
}

/// Ends one URL of [`files_open`].
#[tauri::command]
pub fn files_release(media: State<'_, MediaServer>, url: String) {
    media.release(&url);
}

/// Ends every URL of a tab; the window calls it when the tab's file browser goes away.
#[tauri::command]
pub fn files_release_tab(media: State<'_, MediaServer>, tab_id: String) {
    media.release_tab(&tab_id);
}

/// The JPEG thumbnail of an image (FR-005).
#[tauri::command]
pub async fn files_thumbnail(
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
    size: u64,
    modified_ms: i64,
) -> Result<Response, FilesError> {
    let real = device_path(&files, &source, &path, Want::Read)?;
    let cache = files.thumbnails.clone();
    let bytes =
        blocking(move || thumbnails::thumbnail(&cache, "device", &real, size, modified_ms)).await?;
    Ok(Response::new(bytes))
}

/// Watches the open folder (not its sub folders); every debounced batch goes to `channel`.
#[tauri::command]
pub fn files_watch(
    files: State<'_, FilesState>,
    path: String,
    channel: Channel<FolderChanged>,
) -> Result<u64, FilesError> {
    let real = device_path(&files, &SourceRef::Device, &path, Want::Read)?;
    #[cfg(not(target_os = "ios"))]
    {
        let watch = crate::files::local::watch::watch_folder(&real, false, move |changes| {
            let paths = changes
                .into_iter()
                .map(|change| change.path.to_string_lossy().into_owned())
                .collect();
            let _ = channel.send(FolderChanged { paths });
        })
        .map_err(|reason| {
            log::warn!("files: watching {} failed: {reason}", real.display());
            FilesError::new(FilesErrorCode::Unsupported, "this folder cannot be watched")
        })?;
        Ok(files.keep_watch(watch))
    }
    #[cfg(target_os = "ios")]
    {
        let _ = (real, channel);
        Err(FilesError::new(
            FilesErrorCode::Unsupported,
            "no watching on this platform",
        ))
    }
}

/// Ends a watch of [`files_watch`].
#[tauri::command]
pub fn files_unwatch(files: State<'_, FilesState>, id: u64) {
    files.unwatch(id);
}

/// Opens a file with the system's app for it (FR-014).
#[tauri::command]
pub fn files_open_system(
    app: AppHandle,
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
) -> Result<(), FilesError> {
    let real = device_path(&files, &source, &path, Want::Read)?;
    #[cfg(desktop)]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_path(real.to_string_lossy(), None::<&str>)
            .map_err(|error| {
                log::warn!(
                    "files: the system could not open {}: {error}",
                    real.display()
                );
                FilesError::new(FilesErrorCode::Unsupported, "no app for this file")
            })
    }
    #[cfg(mobile)]
    {
        let _ = (app, real);
        Err(FilesError::new(
            FilesErrorCode::Unsupported,
            "not on this platform yet",
        ))
    }
}

/// Creates a folder (FR-017).
#[tauri::command]
pub async fn files_create_folder(
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
    name: String,
) -> Result<Entry, FilesError> {
    let parent = device_path(&files, &source, &path, Want::Write)?;
    let own = files.own.clone();
    blocking(move || edit::create_folder(&parent, &name, &own)).await
}

/// Renames an entry in its folder (FR-017).
#[tauri::command]
pub async fn files_rename(
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
    new_name: String,
) -> Result<Entry, FilesError> {
    let entry = device_entry(&files, &source, &path, Want::Write)?;
    let own = files.own.clone();
    blocking(move || edit::rename(&entry, &new_name, &own)).await
}

/// Where a copy or move goes.
#[derive(Debug, Clone, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct TransferTarget {
    pub source: SourceRef,
    pub path: String,
}

/// Checks a transfer of `paths` and starts it (FR-018 to FR-023); refusals before the start
/// (`intoItself`, `noSpace`, `holziOwned`, …) come back as the error.
#[allow(clippy::too_many_arguments)]
async fn start_transfer(
    state: &AppState,
    files: &FilesState,
    transfers: &TransferManager,
    op: TransferOp,
    from: &SourceRef,
    paths: &[String],
    to: Option<TransferTarget>,
    channel: Channel<TransferEvent>,
) -> Result<String, FilesError> {
    // Copying reads the sources; moving and deleting change their folders.
    let want = if op == TransferOp::Copy {
        Want::Read
    } else {
        Want::Write
    };
    let sources = paths
        .iter()
        .map(|path| device_entry(files, from, path, want))
        .collect::<Result<Vec<_>, _>>()?;
    let target = match (op, to) {
        (TransferOp::Delete, _) => None,
        (_, Some(to)) => Some(device_path(files, &to.source, &to.path, Want::Write)?),
        (_, None) => return Err(FilesError::invalid_path("no target folder")),
    };
    let job = Job {
        op,
        sources,
        target,
    };
    if touches_own(&job, &files.own) {
        return Err(FilesError::new(
            FilesErrorCode::HolziOwned,
            "holzi's own data is read-only",
        ));
    }
    let (job, totals) = blocking(move || {
        let totals = prepare(
            &job,
            |path| crate::files::local::drives::space(path).map(|(_, free)| free),
            same_device,
        )?;
        Ok((job, totals))
    })
    .await?;
    transfers.start(state.gate(), job, totals, platform_removal(), channel)
}

/// Copies, moves or deletes a selection; the progress comes through `channel`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn files_transfer_start(
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    transfers: State<'_, TransferManager>,
    op: TransferOp,
    from: SourceRef,
    paths: Vec<String>,
    to: Option<TransferTarget>,
    channel: Channel<TransferEvent>,
) -> Result<String, FilesError> {
    start_transfer(&state, &files, &transfers, op, &from, &paths, to, channel).await
}

/// Copies files and folders dropped from the system into a folder (FR-024).
#[tauri::command]
pub async fn files_import_dropped(
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    transfers: State<'_, TransferManager>,
    paths: Vec<String>,
    to: TransferTarget,
    channel: Channel<TransferEvent>,
) -> Result<String, FilesError> {
    start_transfer(
        &state,
        &files,
        &transfers,
        TransferOp::Copy,
        &SourceRef::Device,
        &paths,
        Some(to),
        channel,
    )
    .await
}

/// Answers the name conflict a transfer waits on.
#[tauri::command]
pub fn files_transfer_answer(
    transfers: State<'_, TransferManager>,
    transfer_id: String,
    choice: ConflictChoice,
    for_all: bool,
) {
    transfers.answer(&transfer_id, choice, for_all);
}

/// Cancels a transfer; nothing half-written stays behind (FR-020).
#[tauri::command]
pub fn files_transfer_cancel(transfers: State<'_, TransferManager>, transfer_id: String) {
    transfers.cancel(&transfer_id);
}

/// Starts a failed transfer again.
#[tauri::command]
pub fn files_transfer_retry(
    state: State<'_, AppState>,
    transfers: State<'_, TransferManager>,
    transfer_id: String,
) -> Result<(), FilesError> {
    transfers.retry(state.gate(), &transfer_id)
}
