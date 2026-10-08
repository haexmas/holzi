//! The Android side: a handle to `HolziAndroidPlugin.kt`.

use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::plugin::{PluginApi, PluginHandle};
use tauri::{AppHandle, Runtime};

use crate::Insets;

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

#[derive(Serialize)]
struct SecureArgs {
    enabled: bool,
}

#[derive(Serialize)]
struct InsetsArgs {
    channel: Channel,
}

impl<R: Runtime> HolziAndroid<R> {
    /// The name a document provider shows for a `content://` address; `None` if it names none.
    pub fn display_name(&self, uri: &str) -> Option<String> {
        self.0
            .run_mobile_plugin::<DisplayName>("displayName", UriArgs { uri })
            .ok()?
            .name
    }

    /// Calls `on_change` with the insets now and on every change (FR-011, FR-012).
    pub fn watch_insets(
        &self,
        on_change: impl Fn(Insets) + Send + Sync + 'static,
    ) -> Result<(), String> {
        let channel = Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                match serde_json::from_str::<Insets>(&json) {
                    Ok(insets) => on_change(insets),
                    Err(error) => log::warn!("insets: {error}"),
                }
            }
            Ok(())
        });
        self.0
            .run_mobile_plugin::<()>("watchInsets", InsetsArgs { channel })
            .map_err(|error| error.to_string())
    }

    /// Hides the window from screenshots, screen recordings and the recent apps view (FR-011a).
    pub fn set_secure(&self, enabled: bool) -> Result<(), String> {
        self.0
            .run_mobile_plugin::<()>("setSecure", SecureArgs { enabled })
            .map_err(|error| error.to_string())
    }
}
