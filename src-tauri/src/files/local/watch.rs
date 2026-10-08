//! Watching one folder (spec 044 FR-006, research R3, R11; spec 017 FR-046): debounced, links
//! below the folder not followed (their targets were never checked). The file browser watches its
//! open folder, extensions their granted folders; both on desktops and Android, not on iOS.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer_opt, DebounceEventResult, Debouncer, RecommendedCache};

/// Changes within this time come as one batch.
pub const DEBOUNCE: Duration = Duration::from_millis(500);

/// What happened to a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Created,
    Modified,
    Removed,
    Other,
}

impl ChangeKind {
    /// The name extensions see in `filesync:file-changed`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Modified => "modified",
            Self::Removed => "removed",
            Self::Other => "any",
        }
    }
}

/// One changed path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub path: PathBuf,
}

/// A running watch; dropping it ends the watch, and no batch reaches the handler afterwards.
pub struct FolderWatch {
    _watcher: Debouncer<RecommendedWatcher, RecommendedCache>,
    /// Cleared on drop: the debouncer's thread outlives the watch by one tick and may still hand
    /// over a batch.
    live: Arc<AtomicBool>,
}

impl Drop for FolderWatch {
    fn drop(&mut self) {
        self.live.store(false, Ordering::SeqCst);
    }
}

/// Watches `root` (with everything below it when `recursive`) and hands each debounced batch to
/// `on_changes`.
pub fn watch_folder(
    root: &Path,
    recursive: bool,
    on_changes: impl Fn(Vec<Change>) + Send + 'static,
) -> Result<FolderWatch, String> {
    let live = Arc::new(AtomicBool::new(true));
    let handler_live = Arc::clone(&live);
    let handler = move |result: DebounceEventResult| {
        let Ok(events) = result else {
            return;
        };
        if !handler_live.load(Ordering::SeqCst) {
            return;
        }
        let changes: Vec<Change> = events
            .into_iter()
            .flat_map(|event| {
                let kind = match event.kind {
                    EventKind::Create(_) => ChangeKind::Created,
                    EventKind::Modify(_) => ChangeKind::Modified,
                    EventKind::Remove(_) => ChangeKind::Removed,
                    _ => ChangeKind::Other,
                };
                event
                    .event
                    .paths
                    .into_iter()
                    .map(move |path| Change { kind, path })
            })
            .collect();
        if !changes.is_empty() && handler_live.load(Ordering::SeqCst) {
            on_changes(changes);
        }
    };
    let mut watcher = new_debouncer_opt::<_, RecommendedWatcher, _>(
        DEBOUNCE,
        None,
        handler,
        RecommendedCache::new(),
        Config::default().with_follow_symlinks(false),
    )
    .map_err(|e| e.to_string())?;
    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    watcher.watch(root, mode).map_err(|e| e.to_string())?;
    Ok(FolderWatch {
        _watcher: watcher,
        live,
    })
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
