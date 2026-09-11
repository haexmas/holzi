//! Model file path resolution.
//!
//! Layout: `<AppLocalData>/models/<slug>/<filename>`. `<slug>` is the
//! model id — same value used across `models.id` and
//! `device_downloaded_models_no_sync.id`. Filenames come from the
//! catalog / HuggingFace and are validated to not contain path
//! separators so the `slug/filename` join cannot escape the models
//! directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::error::{HolziError, Result};

pub const MODELS_DIRECTORY: &str = "models";

static MODEL_PUBLICATION_LOCKS: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    OnceLock::new();

/// Serializes finalized-file publication for one model slug within this
/// process. The guard must be held from the finalized-file conflict check
/// through the download/import and catalog registration.
pub async fn acquire_model_publication_lock(
    slug: &str,
) -> Result<tokio::sync::OwnedMutexGuard<()>> {
    validate_slug(slug)?;
    let lock = {
        let locks = MODEL_PUBLICATION_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut locks = locks.lock().map_err(|e| HolziError::CrdtInit {
            reason: format!("model publication lock map poisoned: {e}"),
        })?;
        locks
            .entry(slug.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    Ok(lock.lock_owned().await)
}

/// Resolves `<AppLocalData>/models/`. Creates the directory if missing.
pub fn models_root(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .resolve(MODELS_DIRECTORY, BaseDirectory::AppLocalData)
        .map_err(|e| HolziError::PathResolution {
            reason: format!("resolve models dir: {e}"),
        })?;
    std::fs::create_dir_all(&dir).map_err(HolziError::from)?;
    Ok(dir)
}

/// Resolves `<AppLocalData>/models/<slug>/`. Creates the directory if
/// missing. `slug` is validated the same way as instance names — no
/// path separators, no leading dot.
pub fn slug_dir(app: &AppHandle, slug: &str) -> Result<PathBuf> {
    validate_slug(slug)?;
    let dir = models_root(app)?.join(slug);
    std::fs::create_dir_all(&dir).map_err(HolziError::from)?;
    Ok(dir)
}

/// Resolves `<AppLocalData>/models/<slug>/<filename>`. Validates both
/// parts.
pub fn model_file_path(app: &AppHandle, slug: &str, filename: &str) -> Result<PathBuf> {
    validate_filename(filename)?;
    Ok(slug_dir(app, slug)?.join(filename))
}

/// Builds the string `<slug>/<filename>` used in
/// `device_downloaded_models_no_sync.relative_path`. Relative on
/// purpose so mobile sandboxing does not invalidate the row on
/// container-path changes (plan §"Datenmodell").
pub fn relative_path(slug: &str, filename: &str) -> Result<String> {
    validate_slug(slug)?;
    validate_filename(filename)?;
    Ok(format!("{slug}/{filename}"))
}

/// Resolves a stored relative path back to an absolute one.
pub fn resolve_relative(app: &AppHandle, relative: &str) -> Result<PathBuf> {
    // Guard: reject anything that could escape the root even if the DB
    // was tampered with. Relative paths are always exactly
    // "<slug>/<filename>".
    let mut parts = relative.split('/');
    let slug = parts
        .next()
        .ok_or_else(|| bad_input("empty relative path"))?;
    let filename = parts
        .next()
        .ok_or_else(|| bad_input("relative path missing filename"))?;
    if parts.next().is_some() {
        return Err(bad_input("relative path has more than two segments"));
    }
    model_file_path(app, slug, filename)
}

fn validate_slug(slug: &str) -> Result<()> {
    if slug.is_empty() || slug.len() > 128 {
        return Err(bad_input("slug empty or too long"));
    }
    if slug.starts_with('.') {
        return Err(bad_input("slug cannot start with a dot"));
    }
    if slug
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'))
    {
        return Err(bad_input("slug may only contain [A-Za-z0-9_.-]"));
    }
    Ok(())
}

fn validate_filename(filename: &str) -> Result<()> {
    if filename.is_empty() || filename.len() > 255 {
        return Err(bad_input("filename empty or too long"));
    }
    if filename.starts_with('.') {
        return Err(bad_input("filename cannot start with a dot"));
    }
    if filename
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'))
    {
        return Err(bad_input("filename may only contain [A-Za-z0-9_.-]"));
    }
    if filename.contains("..") {
        return Err(bad_input("filename cannot contain '..'"));
    }
    Ok(())
}

fn bad_input(reason: impl Into<String>) -> HolziError {
    HolziError::InvalidInput {
        reason: reason.into(),
    }
}

/// Returns true when `p` names a file that already exists at the given
/// absolute path. Convenience wrapper the download / import commands
/// use before starting a transfer.
pub fn exists(p: &Path) -> bool {
    p.is_file()
}

/// A resolved on-disk model file plus its accessible metadata. Returned
/// by [`canonical_model_file`] for both discovery (`list_installed_models`)
/// and loading (`load_local_model_by_id`) so both paths agree on which
/// file represents a slug.
#[derive(Debug, Clone)]
pub struct CanonicalModelFile {
    /// Absolute path to the file on disk.
    pub absolute_path: PathBuf,
    /// `<slug>/<filename>` for display and portability.
    pub relative_path: String,
    /// File size in bytes from `fs::metadata`.
    pub size_bytes: u64,
    /// The finalised filename inside `<slug>/`.
    pub filename: String,
}

/// Selects the canonical local GGUF file for a slug from
/// `<AppLocalData>/models/<slug>/`.
///
/// The scan filters to regular files with a `.gguf` extension whose
/// filename is valid UTF-8, ignores in-flight downloads/imports
/// (`.part`, `.tmp` and other non-`.gguf` sidecars), and returns the
/// returns an explicit ambiguity error when more than one finalised file is
/// present. This prevents `list_installed_models` and
/// `load_local_model_by_id` from silently operating on an arbitrary file.
///
/// Returns `Ok(None)` when the slug directory is missing or holds no
/// finalised `.gguf` file. Filesystem errors other than `NotFound` surface as
/// [`HolziError::Io`].
pub fn canonical_model_file(app: &AppHandle, slug: &str) -> Result<Option<CanonicalModelFile>> {
    validate_slug(slug)?;
    let dir = models_root(app)?.join(slug);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(HolziError::from(e)),
    };

    let mut candidate: Option<(String, PathBuf, u64)> = None;
    for entry in entries {
        let entry = entry.map_err(HolziError::from)?;
        let file_type = entry.file_type().map_err(HolziError::from)?;
        if !file_type.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if !name.to_ascii_lowercase().ends_with(".gguf") {
            continue;
        }
        let meta = entry.metadata().map_err(HolziError::from)?;
        let path = entry.path();
        let size = meta.len();
        if candidate.is_some() {
            return Err(HolziError::InvalidInput {
                reason: format!(
                    "multiple finalized GGUF files found for model slug '{slug}'; remove all but one"
                ),
            });
        }
        candidate = Some((name.to_string(), path, size));
    }

    let Some((filename, absolute_path, size_bytes)) = candidate else {
        return Ok(None);
    };
    let relative_path = relative_path(slug, &filename)?;
    Ok(Some(CanonicalModelFile {
        absolute_path,
        relative_path,
        size_bytes,
        filename,
    }))
}
