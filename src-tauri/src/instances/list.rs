//! `list_instances` — directory scan under `<AppLocalData>/instances/`.

use std::fs;
use std::time::UNIX_EPOCH;

use tauri::AppHandle;

use crate::error::{HolziError, Result};

use super::info::InstanceInfo;
use super::paths::{
    get_instances_directory, INSTANCE_EXTENSION, PENDING_MARKER_EXTENSION, TRASH_DIRECTORY,
};

/// Scans `<AppLocalData>/instances/` for `.db` files (excluding
/// `.trash/`, hidden files, and pending Genesis files). Returns entries
/// sorted by mtime descending.
#[tauri::command]
pub async fn list_instances(app: AppHandle) -> Result<Vec<InstanceInfo>> {
    let dir = get_instances_directory(&app)?;
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<InstanceInfo> = Vec::new();
    for entry in fs::read_dir(&dir).map_err(HolziError::from)? {
        let entry = entry.map_err(HolziError::from)?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Skip hidden files, trash directory contents, and pending markers.
        if filename.starts_with('.') || filename == TRASH_DIRECTORY {
            continue;
        }
        if filename.ends_with(&format!(".{PENDING_MARKER_EXTENSION}")) {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some(INSTANCE_EXTENSION) {
            continue;
        }
        // Instance whose Genesis is still in progress: skip until cleanup.
        let pending_sibling = super::paths::get_pending_marker_path(&path);
        if pending_sibling.exists() {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let metadata = entry.metadata().map_err(HolziError::from)?;
        let last_access = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        out.push(InstanceInfo {
            name,
            alias: None,
            last_access,
        });
    }

    out.sort_by_key(|instance| std::cmp::Reverse(instance.last_access));
    Ok(out)
}
