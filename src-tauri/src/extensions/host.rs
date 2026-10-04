//! The in-memory state of the extension host for the one vault of this process (ADR-0003): open
//! frame sessions and the started bundles the protocol handler serves. It ends with the process.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use uuid::Uuid;

use super::bridge::frames::FrameRegistry;
use super::permissions::prompts::PermissionState;
use super::registry::start::Started;

/// What holzi's window reports about itself for `extension_context_get`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostContext {
    /// `light`, `dark` or `system`, as the SDK's `ApplicationContext` names them.
    pub theme: &'static str,
    pub locale: String,
}

impl Default for HostContext {
    fn default() -> Self {
        Self {
            theme: "system",
            locale: "de".to_owned(),
        }
    }
}

/// What holzi does outside its window for extensions (US8): the app sets it once at start; tests
/// set a recording one. Without it these functions are not available (8001).
pub trait Desktop: Send + Sync {
    /// Opens an address in the system's browser.
    fn open_url(&self, url: &str) -> Result<(), String>;
}

#[derive(Default)]
pub struct ExtensionHost {
    pub frames: FrameRegistry,
    /// Open permission questions and decisions held in memory (US3).
    pub permissions: PermissionState,
    /// Entry and Content-Security-Policy per bundle started in this process.
    started: Mutex<HashMap<Uuid, Arc<Started>>>,
    /// The bundle each extension last started with on this device, to see an update.
    effective: Mutex<HashMap<Uuid, Uuid>>,
    context: Mutex<HostContext>,
    /// Open confirmation dialogs by request id: the frame that asked and where the answer goes.
    dialogs: Mutex<HashMap<String, (String, Sender<bool>)>>,
    /// Running SQL calls per extension (limit `max_concurrent`).
    running_sql: Arc<Mutex<HashMap<Uuid, u64>>>,
    desktop: OnceLock<Arc<dyn Desktop>>,
}

/// A running SQL call; dropping it frees its place.
pub struct SqlSlot {
    running: Arc<Mutex<HashMap<Uuid, u64>>>,
    extension_id: Uuid,
}

impl Drop for SqlSlot {
    fn drop(&mut self) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = running.get_mut(&self.extension_id) {
            *count = count.saturating_sub(1);
        }
    }
}

impl ExtensionHost {
    /// Sets what holzi does on the desktop; only the first call counts.
    pub fn set_desktop(&self, desktop: Arc<dyn Desktop>) {
        let _ = self.desktop.set(desktop);
    }

    pub fn desktop(&self) -> Option<Arc<dyn Desktop>> {
        self.desktop.get().cloned()
    }

    fn started_map(&self) -> MutexGuard<'_, HashMap<Uuid, Arc<Started>>> {
        self.started.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn remember_started(&self, started: Started) -> Arc<Started> {
        let started = Arc::new(started);
        self.started_map()
            .insert(started.bundle_id, Arc::clone(&started));
        started
    }

    pub fn started(&self, bundle_id: Uuid) -> Option<Arc<Started>> {
        self.started_map().get(&bundle_id).cloned()
    }

    /// Records that `extension_id` now runs `bundle_id`; returns the bundle it ran before.
    pub fn note_effective(&self, extension_id: Uuid, bundle_id: Uuid) -> Option<Uuid> {
        self.effective
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(extension_id, bundle_id)
    }

    /// Forgets the bundle of a removed extension: its next start is not an update.
    pub fn forget_effective(&self, extension_id: Uuid) {
        self.effective
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&extension_id);
    }

    pub fn context(&self) -> HostContext {
        self.context
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Takes over the color scheme and language holzi's window reports (a theme outside the SDK's
    /// names counts as `system`); returns whether either changed.
    pub fn set_context(&self, theme: &str, locale: &str) -> bool {
        let theme = match theme {
            "light" => "light",
            "dark" => "dark",
            _ => "system",
        };
        let next = HostContext {
            theme,
            locale: locale.to_owned(),
        };
        let mut current = self.context.lock().unwrap_or_else(PoisonError::into_inner);
        let changed = *current != next;
        *current = next;
        changed
    }

    fn dialogs(&self) -> MutexGuard<'_, HashMap<String, (String, Sender<bool>)>> {
        self.dialogs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers a dialog of `frame`; `false` while that frame already has one open.
    pub fn open_dialog(&self, request_id: &str, frame: &str, answer: Sender<bool>) -> bool {
        let mut dialogs = self.dialogs();
        if dialogs.values().any(|(f, _)| f == frame) {
            return false;
        }
        dialogs.insert(request_id.to_owned(), (frame.to_owned(), answer));
        true
    }

    /// Answers a dialog; an unknown or already answered one is ignored.
    pub fn resolve_dialog(&self, request_id: &str, confirmed: bool) {
        if let Some((_, answer)) = self.dialogs().remove(request_id) {
            // The waiting call may have given up already; nothing to do then.
            let _ = answer.send(confirmed);
        }
    }

    /// Ends the dialogs of a closed frame: their calls answer `false`.
    pub fn drop_dialogs_of(&self, frame: &str) {
        self.dialogs().retain(|_, (f, _)| f != frame);
    }

    /// A place for one more SQL call of `extension_id`, or `None` while `max` are running. Other
    /// extensions are not affected.
    pub fn enter_sql(&self, extension_id: Uuid, max: u64) -> Option<SqlSlot> {
        let mut running = self
            .running_sql
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let count = running.entry(extension_id).or_insert(0);
        if *count >= max {
            return None;
        }
        *count += 1;
        Some(SqlSlot {
            running: Arc::clone(&self.running_sql),
            extension_id,
        })
    }
}
