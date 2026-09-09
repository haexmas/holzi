//! Backend-emitted events consumed by the Pinia store.
//!
//! Contract: `events.md`. The frontend re-syncs `list_instances` on every
//! `instance-list-changed` event.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const INSTANCE_LIST_CHANGED: &str = "instance-list-changed";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceListChanged {
    /// Machine-readable reason string: `"created"`, `"opened"`,
    /// `"closed"`, `"imported"`, `"trashed"`, `"startup-cleanup"`.
    pub reason: &'static str,
    /// The affected instance name if any (may be `None` for a bulk
    /// startup cleanup).
    pub affected_name: Option<String>,
}

pub fn emit_instance_list_changed(
    app: &AppHandle,
    reason: &'static str,
    affected_name: Option<String>,
) {
    // A failed emit does not roll the underlying state change back — the
    // frontend will pick the change up on the next explicit `syncAsync`.
    // Log-and-continue is intentional.
    if let Err(e) = app.emit(
        INSTANCE_LIST_CHANGED,
        InstanceListChanged {
            reason,
            affected_name,
        },
    ) {
        log::warn!("emit {INSTANCE_LIST_CHANGED} failed: {e}");
    }
}
