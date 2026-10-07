//! holzi's Android platform code (spec 043, ADR-0010).
//!
//! Everything only Android has goes through this crate, so its Kotlin, manifest entries and
//! Gradle dependencies stay out of the generated `gen/android` project. On every other
//! platform the crate compiles and does nothing.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

#[cfg(target_os = "android")]
mod android;
#[cfg(not(target_os = "android"))]
mod other;

#[cfg(target_os = "android")]
pub use android::HolziAndroid;
#[cfg(not(target_os = "android"))]
pub use other::HolziAndroid;

/// Access to the platform code from any [`Manager`].
pub trait HolziAndroidExt<R: Runtime> {
    fn holzi_android(&self) -> &HolziAndroid<R>;
}

impl<R: Runtime, T: Manager<R>> HolziAndroidExt<R> for T {
    fn holzi_android(&self) -> &HolziAndroid<R> {
        self.state::<HolziAndroid<R>>().inner()
    }
}

/// The plugin; register it once on the app builder.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("holzi-android")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let platform = android::init(app, api)?;
            #[cfg(not(target_os = "android"))]
            let platform = other::init(app, api);
            app.manage(platform);
            Ok(())
        })
        .build()
}
