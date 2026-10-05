//! holzi's [`Desktop`] for extensions in the running app (spec 017, US8): the system's browser
//! through `tauri-plugin-opener`, and system notifications through `tauri-plugin-notification` on
//! every platform. Clicks come back through the plugin's `on_action` (research R20, T099); the
//! plugin is pinned to a fork that reports them on desktop too (Cargo.toml `[patch.crates-io]`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::{Action, ActionType, NotificationExt};
use tauri_plugin_opener::OpenerExt;

use super::host::Desktop;
use super::notifications::{NotificationResponse, NotificationSpec, Respond, ShownNotification};
use crate::sync::keys::hex;

/// The plugin's action id of a click on the notification itself.
const TAP: &str = "tap";
/// Buttons get their own key space, so a button named `tap` stays a button.
const BUTTON: &str = "button:";

/// A shown notification that waits for its response, and its buttons.
struct Waiting {
    respond: Respond,
    buttons: Option<ActionType>,
}

#[derive(Default)]
struct Shown {
    waiting: Mutex<HashMap<i32, Waiting>>,
}

impl Shown {
    fn lock(&self) -> MutexGuard<'_, HashMap<i32, Waiting>> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The action types of every waiting notification: iOS replaces the whole set on each
    /// registration, so it is always registered complete.
    fn action_types(&self) -> Vec<ActionType> {
        self.lock()
            .values()
            .filter_map(|w| w.buttons.clone())
            .collect()
    }
}

pub struct AppDesktop<R: Runtime> {
    app: AppHandle<R>,
    shown: Arc<Shown>,
    next_id: AtomicI32,
}

impl<R: Runtime> AppDesktop<R> {
    /// Also starts listening to the plugin's actions; call it once, after the plugin was added.
    pub fn new(app: AppHandle<R>) -> Self {
        let shown = Arc::new(Shown::default());
        let waiting = Arc::clone(&shown);
        let listening = app.notification().on_action(move |performed| {
            let Some(id) = performed.notification().map(|n| n.id()) else {
                return;
            };
            let Some(entry) = waiting.lock().remove(&id) else {
                return;
            };
            (entry.respond)(response(performed.action_id()));
        });
        if let Err(error) = listening {
            log::warn!("extensions: notification clicks are not reported: {error}");
        }
        Self {
            app,
            shown,
            next_id: AtomicI32::new(1),
        }
    }
}

/// What an action of the plugin means. Besides `tap` and the buttons, Android and iOS report
/// `dismiss` when the user swipes the notification away: that is no click.
fn response(action_id: &str) -> NotificationResponse {
    match action_id {
        TAP => NotificationResponse::Body,
        action => match action.strip_prefix(BUTTON) {
            Some(button) => NotificationResponse::Button(button.to_owned()),
            None => NotificationResponse::Closed,
        },
    }
}

/// The icon as a file the notification system can read, named by its content, in holzi's own
/// cache (not the shared temp directory, where another user could place the folder first). It is
/// written under a name of its own and renamed, so a notification showing the same icon at the
/// same time never reads half a file and a crash leaves no truncated one behind.
fn icon_file(dir: &Path, bytes: &[u8], extension: &str) -> Option<PathBuf> {
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join(format!("{}.{extension}", hex(&Sha256::digest(bytes))));
    if !path.exists() {
        let partial = dir.join(format!("{}.partial", uuid::Uuid::new_v4()));
        std::fs::write(&partial, bytes).ok()?;
        if std::fs::rename(&partial, &path).is_err() {
            let _ = std::fs::remove_file(&partial);
            return None;
        }
    }
    Some(path)
}

/// A notification of the plugin; removing it also forgets its response.
struct PluginNotification<R: Runtime> {
    app: AppHandle<R>,
    shown: Arc<Shown>,
    id: i32,
}

impl<R: Runtime> ShownNotification for PluginNotification<R> {
    fn close(self: Box<Self>) {
        self.shown.lock().remove(&self.id);
        if let Err(error) = self.app.notification().remove_active(vec![self.id]) {
            log::warn!("extensions: a notification could not be removed: {error}");
        }
    }
}

impl<R: Runtime> Desktop for AppDesktop<R> {
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string())
    }

    fn show_notification(
        &self,
        spec: &NotificationSpec,
        respond: Respond,
    ) -> Result<Box<dyn ShownNotification>, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let notifications = self.app.notification();
        let mut builder = notifications.builder().id(id).title(&spec.title);
        if let Some(body) = &spec.body {
            builder = builder.body(body);
        }
        let icons = self.app.path().app_cache_dir().ok();
        if let Some(path) = spec.icon.as_ref().and_then(|(bytes, extension)| {
            icon_file(&icons?.join("notification-icons"), bytes, extension)
        }) {
            builder = builder.icon(path.to_string_lossy());
        }
        let buttons = (!spec.buttons.is_empty()).then(|| {
            ActionType::builder(format!("holzi-{id}"))
                .actions(
                    spec.buttons
                        .iter()
                        .map(|b| Action::builder(format!("{BUTTON}{}", b.id), &b.label).build())
                        .collect(),
                )
                .build()
        });
        if let Some(buttons) = &buttons {
            builder = builder.action_type_id(buttons.id());
        }
        self.shown.lock().insert(id, Waiting { respond, buttons });
        let shown = notifications
            .register_action_types(self.shown.action_types())
            .and_then(|()| builder.show());
        if let Err(error) = shown {
            self.shown.lock().remove(&id);
            return Err(error.to_string());
        }
        Ok(Box::new(PluginNotification {
            app: self.app.clone(),
            shown: Arc::clone(&self.shown),
            id,
        }))
    }
}

#[cfg(test)]
#[path = "desktop_tests.rs"]
mod tests;
