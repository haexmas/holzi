//! holzi's [`Desktop`] for extensions in the running app (spec 017, US8): the system's browser
//! through `tauri-plugin-opener`, and system notifications. On Linux and the BSDs they go straight
//! to the notification server (`notify-rust`), which reports clicks (research R20, T099); elsewhere
//! `tauri-plugin-notification` shows them and no click comes back.

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

use super::host::Desktop;
use super::notifications::{NotificationSpec, Respond, ShownNotification};

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

    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    fn show_notification(
        &self,
        notification: &NotificationSpec,
        respond: Respond,
    ) -> Result<Box<dyn ShownNotification>, String> {
        xdg::show(notification, respond)
    }

    #[cfg(not(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )))]
    fn show_notification(
        &self,
        notification: &NotificationSpec,
        _respond: Respond,
    ) -> Result<Box<dyn ShownNotification>, String> {
        use tauri_plugin_notification::NotificationExt;
        let mut builder = self.app.notification().builder().title(&notification.title);
        if let Some(body) = &notification.body {
            builder = builder.body(body);
        }
        builder.show().map_err(|e| e.to_string())?;
        Ok(Box::new(Untracked))
    }

    fn focus_window(&self) {
        for window in self.app.webview_windows().values() {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// A notification holzi cannot remove again.
#[cfg(not(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
)))]
struct Untracked;

#[cfg(not(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
)))]
impl ShownNotification for Untracked {
    fn close(self: Box<Self>) {}
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
mod xdg {
    use std::path::PathBuf;

    use sha2::{Digest, Sha256};

    use crate::extensions::notifications::{
        NotificationResponse, NotificationSpec, Respond, ShownNotification,
    };
    use crate::sync::keys::hex;

    /// The action key of a click on the notification itself (Desktop Notifications spec).
    const BODY: &str = "default";
    /// Buttons get their own key space, so a button named `default` stays a button.
    const BUTTON: &str = "button:";

    struct Shown(notify_rust::NotificationHandle);

    impl ShownNotification for Shown {
        fn close(self: Box<Self>) {
            self.0.close();
        }
    }

    /// The icon as a file the notification server can read, named by its content.
    fn icon_file(bytes: &[u8], extension: &str) -> Option<PathBuf> {
        let dir = std::env::temp_dir().join("holzi-notification-icons");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join(format!("{}.{extension}", hex(&Sha256::digest(bytes))));
        if !path.exists() {
            std::fs::write(&path, bytes).ok()?;
        }
        Some(path)
    }

    pub(super) fn show(
        spec: &NotificationSpec,
        respond: Respond,
    ) -> Result<Box<dyn ShownNotification>, String> {
        let mut notification = notify_rust::Notification::new();
        notification
            .appname("holzi")
            .summary(&spec.title)
            .action(BODY, &spec.title);
        if let Some(body) = &spec.body {
            notification.body(body);
        }
        for button in &spec.buttons {
            notification.action(&format!("{BUTTON}{}", button.id), &button.label);
        }
        if let Some(path) = spec
            .icon
            .as_ref()
            .and_then(|(bytes, extension)| icon_file(bytes, extension))
        {
            notification.icon(&path.to_string_lossy());
        }
        let handle = notification.show().map_err(|e| e.to_string())?;
        let id = handle.id();
        std::thread::Builder::new()
            .name("notification".into())
            .spawn(move || {
                let _ = notify_rust::handle_action(id, |response| {
                    respond(match response {
                        notify_rust::ActionResponse::Custom(BODY) => NotificationResponse::Body,
                        notify_rust::ActionResponse::Custom(key) => {
                            match key.strip_prefix(BUTTON) {
                                Some(button) => NotificationResponse::Button(button.to_owned()),
                                None => NotificationResponse::Body,
                            }
                        }
                        notify_rust::ActionResponse::Closed(_) => NotificationResponse::Closed,
                    });
                });
            })
            .map_err(|e| e.to_string())?;
        Ok(Box::new(Shown(handle)))
    }
}
