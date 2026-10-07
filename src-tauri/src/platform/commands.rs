//! The capability table for the interface.

use super::{capabilities, PlatformCapabilities};

/// What this device can do; the interface reads it once at start.
#[tauri::command]
pub fn platform_capabilities() -> PlatformCapabilities {
    capabilities()
}
