//! Every platform but Android: nothing to do.

use std::marker::PhantomData;

use tauri::plugin::PluginApi;
use tauri::{AppHandle, Runtime};

pub fn init<R: Runtime>(_app: &AppHandle<R>, _api: PluginApi<R, ()>) -> HolziAndroid<R> {
    HolziAndroid(PhantomData)
}

/// Stand-in with the same methods as the Android side; each does nothing.
pub struct HolziAndroid<R: Runtime>(PhantomData<fn() -> R>);

impl<R: Runtime> HolziAndroid<R> {
    /// Addresses of document providers exist only on Android.
    pub fn display_name(&self, _uri: &str) -> Option<String> {
        None
    }

    /// The system takes no space at the window's edges here.
    pub fn watch_insets(
        &self,
        _on_change: impl Fn(crate::Insets) + Send + Sync + 'static,
    ) -> Result<(), String> {
        Ok(())
    }

    /// The network changes the sync notices by itself here.
    pub fn watch_network(&self, _on_change: impl Fn() + Send + Sync + 'static) -> Result<(), String> {
        Ok(())
    }

    /// The host name is the device's name here.
    pub fn device_name(&self) -> Option<String> {
        None
    }

    /// Screen capture protection exists only on Android.
    pub fn set_secure(&self, _enabled: bool) -> Result<(), String> {
        Ok(())
    }
}
