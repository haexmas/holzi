//! holzi's [`Desktop`] for extensions in the running app (spec 017, US8): the system's browser
//! through `tauri-plugin-opener`.

use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;

use super::host::Desktop;

pub struct AppDesktop<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AppDesktop<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> Desktop for AppDesktop<R> {
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string())
    }
}
