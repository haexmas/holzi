//! Startup orphan cleanup: remove `.pending` markers and their sibling
//! `.db` files. Called from `lib.rs::setup` before any command runs.
//!
//! Contract: `tauri-commands.md` — a crash between `create_instance`
//! step 3 (write marker + create DB) and step 7 (remove marker) leaves
//! a `.pending` file. That file is the signal that the DB is
//! half-initialised and must be removed.

use std::fs;
use std::path::Path;

use tauri::AppHandle;

use crate::error::{HolziError, Result};

use super::events::emit_instance_list_changed;
use super::paths::{get_instances_directory, PENDING_MARKER_EXTENSION};

/// Tauri-facing entry. Resolves the instances directory and delegates
/// to the pure `cleanup_orphans_in_dir` helper.
pub fn cleanup_orphans_on_startup(app: &AppHandle) -> Result<()> {
    let dir = get_instances_directory(app)?;
    let removed = cleanup_orphans_in_dir(&dir)?;
    if removed > 0 {
        emit_instance_list_changed(app, "startup-cleanup", None);
    }
    Ok(())
}

/// Pure filesystem worker: scans `dir` for `<name>.db.pending` markers,
/// deletes each together with its sibling `<name>.db`. Returns the
/// number of orphans removed. Missing `dir` is not an error.
pub fn cleanup_orphans_in_dir(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut removed = 0usize;
    for entry in fs::read_dir(dir).map_err(HolziError::from)? {
        let entry = entry.map_err(HolziError::from)?;
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // `foo.db.pending` — trailing suffix, not the last dot-component
        // returned by `path.extension()`.
        let marker_suffix = format!(".{PENDING_MARKER_EXTENSION}");
        let Some(sibling_filename) = filename.strip_suffix(".pending") else {
            continue;
        };
        if !filename.ends_with(&marker_suffix) {
            continue;
        }
        // Sibling `.db` first. A missing sibling is expected if the crash
        // happened before Database::open; other deletion failures retain the
        // marker so the file cannot be published as a healthy instance.
        // `with_extension("db")` would turn `foo.db.pending` into
        // `foo.db.db`; strip the `.pending` suffix from the filename
        // instead.
        let sibling_db = path.with_file_name(sibling_filename);
        match fs::remove_file(&sibling_db) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                log::warn!("failed to remove orphan database {sibling_db:?}: {e}");
                continue;
            }
        }
        if let Err(e) = fs::remove_file(&path) {
            log::warn!("failed to remove orphan pending marker {path:?}: {e}");
            continue;
        }
        removed += 1;
    }
    Ok(removed)
}
