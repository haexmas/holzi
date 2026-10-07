//! Every platform but Android: nothing to do.

use std::marker::PhantomData;

use tauri::plugin::PluginApi;
use tauri::{AppHandle, Runtime};

pub fn init<R: Runtime>(_app: &AppHandle<R>, _api: PluginApi<R, ()>) -> HolziAndroid<R> {
    HolziAndroid(PhantomData)
}

/// Stand-in with the same methods as the Android side; each does nothing.
pub struct HolziAndroid<R: Runtime>(PhantomData<fn() -> R>);
