//! Events the sync emits to the frontend (spec 024,
//! contracts/tauri-commands.md). Payloads are structured data, never
//! localized text.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

/// The device list, a device's online state, a name, a problem or an
/// admission request changed (FR-034).
pub const SYNC_DEVICES_CHANGED: &str = "sync-devices-changed";
/// The state of a link hosted by the open vault changed.
pub const LINK_HOST_STATE_CHANGED: &str = "link-host-state-changed";
/// The state of a link joining a vault from the start page changed.
pub const LINK_JOIN_STATE_CHANGED: &str = "link-join-state-changed";

/// Emits `event`; a failed emit is logged, the frontend catches up on its
/// next read.
pub fn emit<R: Runtime, T: Serialize + Clone>(app: &AppHandle<R>, event: &str, payload: T) {
    if let Err(e) = app.emit(event, payload) {
        log::warn!("emit {event} failed: {e}");
    }
}
