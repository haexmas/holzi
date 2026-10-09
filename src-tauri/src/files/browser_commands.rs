//! Tauri commands of the file browser's window (spec 044, contracts/tauri-commands.md). Every
//! command acts as [`Caller::User`]; paths are resolved and checked in Rust before anything is
//! read. Storages (US5) answer `unsupported` until their source is built.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::error::HolziError;
use crate::files::access::{check, AgentGrants, Target, Verdict, Want};
use crate::files::kind::{viewer_kind, ViewerKind};
use crate::files::local::drives::{drives, Drive};
use crate::files::local::text::{read_text, text_from, TextContent, TEXT_LIMIT};
use crate::files::local::{edit, ops, resolve, resolve_entry};
use crate::files::media::MediaServer;
use crate::files::search::{
    FilesSearchEvent, SearchFilters, SearchLimits, SearchManager, SearchOptions,
};
use crate::files::state::FilesState;
use crate::files::storage_source::{object_key, storage_error, StorageFiles};
use crate::files::streaming::{LocalFileSource, StorageFileSource};
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
    /// The storages of spec 038 (US5).
    pub storages: Vec<StorageSource>,
}

/// A storage for the sidebar.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct StorageSource {
    pub id: String,
    pub name: String,
}

/// The files of the storage `storage_id` (spec 044 US5), or why not. Its credentials stay inside
/// `remote_storage`; no answer carries them (FR-038).
async fn storage_files(
    state: &State<'_, AppState>,
    storage_id: &str,
) -> Result<StorageFiles, FilesError> {
    let service = crate::remote_storage::commands::service(state)
        .map_err(|_| FilesError::new(FilesErrorCode::Unsupported, "no open vault"))?;
    let access = service
        .access_of(storage_id)
        .await
        .map_err(|error| match error {
            HolziError::StorageNotFound => {
                FilesError::new(FilesErrorCode::NotFound, "no such storage")
            }
            HolziError::StorageCredentialsUnavailable { .. } => FilesError::new(
                FilesErrorCode::Credentials,
                "the storage's credentials are not on this device",
            ),
            other => {
                log::warn!("files: a storage could not be opened: {other}");
                FilesError::new(FilesErrorCode::Unsupported, "the storage cannot be opened")
            }
        })?;
    Ok(StorageFiles::new(
        crate::remote_storage::commands::s3_store(),
        access,
    ))
}

/// The storage id of `source`, if it is one.
fn storage_of(source: &SourceRef) -> Option<&str> {
    match source {
        SourceRef::Storage { storage_id } => Some(storage_id),
        SourceRef::Device => None,
    }
}

/// The answer for what storages cannot do before the transfers across sources (PR F2).
fn later() -> FilesError {
    FilesError::new(
        FilesErrorCode::Unsupported,
        "copying to and from storages comes with a later version",
    )
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
pub async fn files_sources(
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
) -> Result<Sources, FilesError> {
    let known = files
        .known
        .iter()
        .map(|place| KnownPlace {
            name: place.name.to_owned(),
            path: place.path.to_string_lossy().into_owned(),
        })
        .collect();
    let drives = blocking(|| Ok(drives())).await?;
    // Without an open vault (or with a storage list that fails) the device alone shows.
    let storages = match crate::remote_storage::commands::service(&state) {
        Ok(service) => service
            .overview()
            .await
            .map(|overview| {
                overview
                    .storages
                    .into_iter()
                    .map(|storage| StorageSource {
                        id: storage.id,
                        name: storage.name,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    Ok(Sources {
        drives,
        known,
        storages,
    })
}

/// Every entry of a folder.
#[tauri::command]
pub async fn files_list(
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
) -> Result<Vec<Entry>, FilesError> {
    if let Some(id) = storage_of(&source) {
        return storage_files(&state, id).await?.list(&path).await;
    }
    let real = device_path(&files, &source, &path, Want::Read)?;
    let own = files.own.clone();
    blocking(move || ops::list(&real, &own)).await
}

/// One entry.
#[tauri::command]
pub async fn files_stat(
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
) -> Result<Entry, FilesError> {
    if let Some(id) = storage_of(&source) {
        return storage_files(&state, id).await?.stat(&path).await;
    }
    let real = device_path(&files, &source, &path, Want::Read)?;
    let own = files.own.clone();
    blocking(move || ops::stat(&real, &own)).await
}

/// The text of a file for the viewer, at most 5 MB (FR-015).
#[tauri::command]
pub async fn files_read_text(
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
) -> Result<TextContent, FilesError> {
    if let Some(id) = storage_of(&source) {
        let storage = storage_files(&state, id).await?;
        let entry = storage.stat(&path).await?;
        if entry.kind != EntryKind::File {
            return Err(FilesError::invalid_path("not a file"));
        }
        let bytes = storage
            .read_start(&path, entry.size.unwrap_or(0), TEXT_LIMIT)
            .await?;
        return text_from(bytes, TEXT_LIMIT);
    }
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
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    media: State<'_, MediaServer>,
    source: SourceRef,
    path: String,
    tab_id: String,
) -> Result<Opened, FilesError> {
    if let Some(id) = storage_of(&source) {
        let storage = storage_files(&state, id).await?;
        let entry = storage.stat(&path).await?;
        if entry.kind != EntryKind::File {
            return Err(FilesError::invalid_path("not a file"));
        }
        let kind = viewer_kind(&entry.name);
        let url = if streams(kind) {
            let mime = entry
                .mime
                .clone()
                .unwrap_or_else(|| "application/octet-stream".to_owned());
            let file = StorageFileSource::new(
                storage.store().clone(),
                storage.access().clone(),
                object_key(&path)?,
                entry.size.unwrap_or(0),
                mime,
            );
            Some(media.register(&tab_id, Arc::new(file)))
        } else {
            None
        };
        return Ok(Opened { kind, url, entry });
    }
    let real = device_path(&files, &source, &path, Want::Read)?;
    let own = files.own.clone();
    let stat_path = real.clone();
    let entry = blocking(move || ops::stat(&stat_path, &own)).await?;
    if entry.kind != EntryKind::File {
        return Err(FilesError::invalid_path("not a file"));
    }
    let kind = viewer_kind(&entry.name);
    let url = streams(kind).then(|| {
        let mime = entry
            .mime
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_owned());
        media.register(&tab_id, Arc::new(LocalFileSource::new(real, mime)))
    });
    Ok(Opened { kind, url, entry })
}

/// Whether the viewer shows `kind` from the media server.
fn streams(kind: ViewerKind) -> bool {
    matches!(
        kind,
        ViewerKind::Image | ViewerKind::Video | ViewerKind::Audio | ViewerKind::Pdf
    )
}

/// The largest image of a storage holzi downloads for a thumbnail.
const STORAGE_THUMBNAIL_LIMIT: u64 = 50 * 1024 * 1024;

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
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
    size: u64,
    modified_ms: i64,
) -> Result<Response, FilesError> {
    if let Some(id) = storage_of(&source) {
        let cache = files.thumbnails.clone();
        let label = format!("storage:{id}");
        if let Some(hit) = thumbnails::cached(&cache, &label, &path, size, modified_ms) {
            return hit.map(Response::new);
        }
        if size > STORAGE_THUMBNAIL_LIMIT {
            return Err(FilesError::new(
                FilesErrorCode::TooLarge,
                "too large for a thumbnail",
            ));
        }
        let storage = storage_files(&state, id).await?;
        let key = object_key(&path)?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut reader = storage
            .store()
            .get_range(storage.access(), &key, 0, size, deadline)
            .await
            .map_err(storage_error)?;
        let temp = cache.join(format!(".download-{}", uuid::Uuid::new_v4()));
        let downloaded = async {
            tokio::fs::create_dir_all(&cache).await?;
            let mut file = tokio::fs::File::create(&temp).await?;
            tokio::io::copy(&mut reader, &mut file).await?;
            Ok::<(), std::io::Error>(())
        }
        .await;
        if downloaded.is_err() {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(storage_error(crate::remote_storage::StorageError::Network));
        }
        let bytes = blocking(move || {
            let made =
                thumbnails::render_into_cache(&cache, &label, &path, size, modified_ms, &temp);
            let _ = std::fs::remove_file(&temp);
            made
        })
        .await?;
        return Ok(Response::new(bytes));
    }
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
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
    name: String,
) -> Result<Entry, FilesError> {
    if let Some(id) = storage_of(&source) {
        return storage_files(&state, id)
            .await?
            .create_folder(&path, &name)
            .await;
    }
    let parent = device_path(&files, &source, &path, Want::Write)?;
    let own = files.own.clone();
    blocking(move || edit::create_folder(&parent, &name, &own)).await
}

/// Renames an entry in its folder (FR-017).
#[tauri::command]
pub async fn files_rename(
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    source: SourceRef,
    path: String,
    new_name: String,
) -> Result<Entry, FilesError> {
    if let Some(id) = storage_of(&source) {
        return storage_files(&state, id)
            .await?
            .rename(&path, &new_name)
            .await;
    }
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
    if let Some(id) = storage_of(&from) {
        // A storage has no trash: the window asked before (FR-023).
        if op != TransferOp::Delete {
            return Err(later());
        }
        let storage = storage_files(&state, id).await?;
        return transfers.start_storage_delete(state.gate(), storage, paths, channel);
    }
    if to
        .as_ref()
        .is_some_and(|target| storage_of(&target.source).is_some())
    {
        return Err(later());
    }
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
    if storage_of(&to.source).is_some() {
        return Err(later());
    }
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

/// Searches the folder `path` and below by name (FR-027 to FR-030); hits come through `channel`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn files_search_start(
    state: State<'_, AppState>,
    files: State<'_, FilesState>,
    searches: State<'_, SearchManager>,
    source: SourceRef,
    path: String,
    query: String,
    filters: SearchFilters,
    show_hidden: bool,
    channel: Channel<FilesSearchEvent>,
) -> Result<String, FilesError> {
    let root = device_path(&files, &source, &path, Want::Read)?;
    let options = SearchOptions {
        filters,
        show_hidden,
        own: files.own.clone(),
        hide_own: false,
        limits: SearchLimits::USER,
    };
    searches.start(state.gate(), root, query, options, channel)
}

/// Ends a search; the window calls it when the query changes or the tab goes away (FR-028).
#[tauri::command]
pub fn files_search_cancel(searches: State<'_, SearchManager>, search_id: String) {
    searches.cancel(&search_id);
}
