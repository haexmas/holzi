//! Model file path resolution.
//!
//! Layout: `<AppLocalData>/models/<slug>/<filename>`. `<slug>` is the
//! model id — same value used across `models.id` and
//! `device_downloaded_models_no_sync.id`. Filenames come from the
//! catalog / HuggingFace and are validated to not contain path
//! separators so the `slug/filename` join cannot escape the models
//! directory.

use std::path::{Path, PathBuf};

use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::error::{HolziError, Result};

pub const MODELS_DIRECTORY: &str = "models";

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
    let slug = parts.next().ok_or_else(|| bad_input("empty relative path"))?;
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
        return Err(bad_input(
            "slug may only contain [A-Za-z0-9_.-]",
        ));
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
