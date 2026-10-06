//! What the host keeps for the remote storage of extensions (spec 038, research R6): the provider
//! and resolver (S3 and the system's resolver in holzi, fakes in tests) and the storage dialogs that
//! wait for the user.
//!
//! A management call of an extension (add, update, test, remove) waits like
//! `extension_dialog_confirm`: holzi sends `extension-storage-request` to its window, which shows a
//! confirmation over the tab (never with credential fields) and, when new credentials are needed,
//! holzi's own window over the whole app. The answer comes back through `storage_dialog_resolve`;
//! credentials in it travel from holzi's window to Rust only and never reach the extension.
//! Closing the frame or [`DIALOG_WAIT`] counts as cancel.

use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use ts_rs::TS;

use super::bridge::dispatch::CallContext;
use super::error::{BridgeError, ExtensionErrorCode};
use super::protocol::token;
use crate::remote_storage::address::{Resolver, SystemResolver};
use crate::remote_storage::model::CredentialsInput;
use crate::remote_storage::RemoteStore;

/// A storage dialog nobody answers ends as cancel, like a confirmation dialog.
pub const DIALOG_WAIT: Duration = Duration::from_secs(300);

/// The user's answer to a storage dialog (contracts/tauri-commands.md `storage_dialog_resolve`).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StorageAnswer {
    Cancel,
    #[serde(rename_all = "camelCase")]
    Confirm {
        /// An existing connection the user chose for the new storage.
        #[ts(optional)]
        #[serde(default)]
        connection_id: Option<String>,
        /// New credentials; only holzi's window over the whole app sends them.
        #[ts(optional)]
        #[serde(default)]
        credentials: Option<CredentialsInput>,
        #[ts(optional)]
        #[serde(default)]
        name: Option<String>,
        #[ts(optional)]
        #[serde(default)]
        bucket: Option<String>,
    },
}

/// The provider, the resolver and the open storage dialogs of this process.
#[derive(Default)]
pub struct StorageState {
    store: OnceLock<Arc<dyn RemoteStore>>,
    resolver: OnceLock<Arc<dyn Resolver>>,
    /// Open dialogs by request id: the frame that asked and where the answer goes.
    dialogs: Mutex<HashMap<String, (String, Sender<StorageAnswer>)>>,
}

impl StorageState {
    /// Sets provider and resolver; only the first call counts (tests set fakes).
    pub fn set(&self, store: Arc<dyn RemoteStore>, resolver: Arc<dyn Resolver>) {
        let _ = self.store.set(store);
        let _ = self.resolver.set(resolver);
    }

    /// The provider: S3 unless a test set another.
    pub fn store(&self) -> Arc<dyn RemoteStore> {
        Arc::clone(
            self.store
                .get_or_init(crate::remote_storage::commands::s3_store),
        )
    }

    /// The resolver: the system's unless a test set another.
    pub fn resolver(&self) -> Arc<dyn Resolver> {
        Arc::clone(self.resolver.get_or_init(|| Arc::new(SystemResolver)))
    }

    fn dialogs(&self) -> MutexGuard<'_, HashMap<String, (String, Sender<StorageAnswer>)>> {
        self.dialogs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers a dialog of `frame`; `false` while that frame already has one open.
    fn open(&self, request_id: &str, frame: &str, answer: Sender<StorageAnswer>) -> bool {
        let mut dialogs = self.dialogs();
        if dialogs.values().any(|(f, _)| f == frame) {
            return false;
        }
        dialogs.insert(request_id.to_owned(), (frame.to_owned(), answer));
        true
    }

    /// Answers a dialog; an unknown or already answered one is ignored.
    pub fn resolve(&self, request_id: &str, answer: StorageAnswer) {
        if let Some((_, sender)) = self.dialogs().remove(request_id) {
            // The waiting call may have given up already.
            let _ = sender.send(answer);
        }
    }

    /// Ends the dialogs of a closed frame: their calls are cancelled.
    pub fn drop_of(&self, frame: &str) {
        self.dialogs().retain(|_, (f, _)| f != frame);
    }

    /// Whether a dialog with this id waits (tests).
    pub fn is_open(&self, request_id: &str) -> bool {
        self.dialogs().contains_key(request_id)
    }
}

/// What a dialog is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    Add,
    Update,
    Test,
    Remove,
}

impl DialogKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Update => "update",
            Self::Test => "test",
            Self::Remove => "remove",
        }
    }
}

fn cancelled() -> BridgeError {
    BridgeError::new(
        ExtensionErrorCode::PermissionDenied,
        "cancelled by the user",
    )
}

/// Asks the user over the calling frame and waits for the answer; cancel, the frame's end or
/// [`DIALOG_WAIT`] is 1002. `proposal` and `other_extensions` are shown, never credentials.
pub fn ask(
    ctx: &CallContext,
    kind: DialogKind,
    extension_name: &str,
    proposal: Value,
    other_extensions: &[String],
) -> Result<StorageAnswer, BridgeError> {
    let request_id = token::mint();
    let frame = &ctx.session.frame;
    let (answer, waiting) = mpsc::channel();
    if !ctx.host.storage.open(&request_id, frame, answer) {
        return Err(BridgeError::new(
            ExtensionErrorCode::LimitExceeded,
            "a storage dialog of this frame is already open",
        ));
    }
    ctx.emitter.emit(
        "extension-storage-request",
        json!({
            "requestId": request_id,
            "frame": frame,
            "kind": kind.as_str(),
            "proposal": proposal,
            "extensionName": extension_name,
            "otherExtensions": other_extensions,
        }),
    );
    let answer = waiting.recv_timeout(DIALOG_WAIT);
    // After a timeout the dialog is still registered; a later answer must reach nobody, and
    // holzi's windows for it close.
    ctx.host.storage.resolve(&request_id, StorageAnswer::Cancel);
    ctx.emitter.emit(
        "extension-storage-request-ended",
        json!({ "requestId": request_id }),
    );
    match answer {
        Ok(answer @ StorageAnswer::Confirm { .. }) => Ok(answer),
        Ok(StorageAnswer::Cancel) | Err(_) => Err(cancelled()),
    }
}
