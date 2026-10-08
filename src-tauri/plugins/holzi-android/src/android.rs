//! The Android side: a handle to `HolziAndroidPlugin.kt`.

use serde::{Deserialize, Serialize};
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
pub struct HolziAndroid<R: Runtime>(PluginHandle<R>);

#[derive(Serialize)]
struct UriArgs<'a> {
    uri: &'a str,
}

#[derive(Deserialize)]
struct DisplayName {
    name: Option<String>,
}

impl<R: Runtime> HolziAndroid<R> {
    /// The name a document provider shows for a `content://` address; `None` if it names none.
    pub fn display_name(&self, uri: &str) -> Option<String> {
        self.0
            .run_mobile_plugin::<DisplayName>("displayName", UriArgs { uri })
            .ok()?
            .name
    }
}
