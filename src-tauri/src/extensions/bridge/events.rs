//! Messages from holzi to the frames of one extension (contracts/bridge.md §Meldungen): Rust
//! sends `extension-frame-event {frame, type, data, timestamp}` to holzi's window for every open
//! frame of that extension; the window hands it to the frame's port. Frames of other extensions
//! get nothing.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use uuid::Uuid;

use super::dispatch::Emit;
use crate::extensions::host::ExtensionHost;

pub const FRAME_EVENT: &str = "extension-frame-event";

/// Sends `event_type` to every open frame of `extension_id` → the frames it went to.
pub fn emit_to_frames(
    emitter: &dyn Emit,
    host: &ExtensionHost,
    extension_id: Uuid,
    event_type: &str,
    data: &Value,
) -> Vec<String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let mut frames = Vec::new();
    for session in host.frames.of_extension(extension_id) {
        emitter.emit(
            FRAME_EVENT,
            json!({
                "frame": session.frame,
                "type": event_type,
                "data": data,
                "timestamp": timestamp,
            }),
        );
        frames.push(session.frame.clone());
    }
    frames
}
