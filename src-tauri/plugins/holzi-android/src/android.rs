//! The Android side: a handle to `HolziAndroidPlugin.kt`.

use tauri::plugin::{PluginApi, PluginHandle};
use tauri::{AppHandle, Runtime};

const PLUGIN_IDENTIFIER: &str = "space.haex.holzi.android";

pub fn init<R: Runtime>(
    _app: &AppHandle<R>,
    api: PluginApi<R, ()>,
) -> Result<HolziAndroid<R>, Box<dyn std::error::Error>> {
    let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "HolziAndroidPlugin")?;
    Ok(HolziAndroid(handle))
}

/// The registered Kotlin plugin.
pub struct HolziAndroid<R: Runtime>(#[allow(dead_code)] PluginHandle<R>);
