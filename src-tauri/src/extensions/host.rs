//! The in-memory state of the extension host for the one vault of this process (ADR-0003): open
//! frame sessions and the started bundles the protocol handler serves. It ends with the process.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use uuid::Uuid;

use super::bridge::frames::FrameRegistry;
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

#[derive(Default)]
pub struct ExtensionHost {
    pub frames: FrameRegistry,
    /// Entry and Content-Security-Policy per bundle started in this process.
    started: Mutex<HashMap<Uuid, Arc<Started>>>,
    context: Mutex<HostContext>,
    /// Open confirmation dialogs by request id: the frame that asked and where the answer goes.
    dialogs: Mutex<HashMap<String, (String, Sender<bool>)>>,
}

impl ExtensionHost {
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

    pub fn context(&self) -> HostContext {
        self.context
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Sets the context holzi's window reports; a theme outside the SDK's names counts as
    /// `system`.
    pub fn set_context(&self, theme: &str, locale: &str) {
        let theme = match theme {
            "light" => "light",
            "dark" => "dark",
            _ => "system",
        };
        *self.context.lock().unwrap_or_else(PoisonError::into_inner) = HostContext {
            theme,
            locale: locale.to_owned(),
        };
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
}
