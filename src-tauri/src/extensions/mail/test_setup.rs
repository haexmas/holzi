//! The setup of the mail tests: an installed extension with a frame, the test server of
//! `test_server.rs` on this device, and the events the frame would hear.

use std::path::Path;
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};

use super::test_server::{self, Shared, PASSWORD, USER};
use crate::extensions::bridge::dispatch::{call, CallContext, Emit};
use crate::extensions::commands::permissions::{set, PermissionSetArgs};
use crate::extensions::error::BridgeError;
use crate::extensions::host::ExtensionHost;
use crate::extensions::registry::effective::effective_bundle;
use crate::extensions::registry::install::install;
use crate::passwords::test_support::open_test_vault;
use crate::storage::known_devices;
use crate::vault_gate::VaultGate;

#[derive(Default)]
pub(super) struct Recorded(pub(super) Mutex<Vec<(String, Value)>>);

impl Emit for Recorded {
    fn emit(&self, event: &str, payload: Value) {
        self.0.lock().unwrap().push((event.to_owned(), payload));
    }
}

impl Recorded {
    /// The `data` of every frame event of `kind`.
    pub(super) fn heard(&self, kind: &str) -> Vec<Value> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, p)| {
                e == crate::extensions::bridge::events::FRAME_EVENT && p["type"] == kind
            })
            .map(|(_, p)| p["data"].clone())
            .collect()
    }
}

pub(super) struct Setup {
    pub(super) _dir: tempfile::TempDir,
    pub(super) rt: tokio::runtime::Runtime,
    pub(super) ctx: Arc<CallContext>,
    pub(super) server: Shared,
    pub(super) imap_port: u16,
    pub(super) smtp_port: u16,
    pub(super) recorded: Arc<Recorded>,
}

pub(super) fn setup() -> Setup {
    let (dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db)).unwrap();
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .unwrap()[0]
        .vault_device_uuid;
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension_bundles/good-minimal.xt"),
    )
    .unwrap();
    let extension = install(&vault, &bytes, vec![], false, device, 1)
        .unwrap()
        .ids
        .extension_id;
    let bundle = vault
        .read_blocking(move |q| effective_bundle(q, extension).map_err(Into::into))
        .unwrap()
        .unwrap()
        .bundle_id;
    let host = Arc::new(ExtensionHost::default());
    let recorded = Arc::new(Recorded::default());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = test_server::state();
    let (imap_port, smtp_port) = test_server::start(&rt, Arc::clone(&server));
    Setup {
        _dir: dir,
        rt,
        ctx: Arc::new(CallContext {
            db: vault,
            session: host.frames.open(extension, bundle, "tab"),
            host,
            device,
            emitter: Arc::clone(&recorded) as Arc<dyn Emit>,
        }),
        server,
        imap_port,
        smtp_port,
        recorded,
    }
}

impl Setup {
    pub(super) fn call(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        let ctx = Arc::clone(&self.ctx);
        let method = method.to_owned();
        self.rt
            .block_on(self.rt.spawn_blocking(move || call(&ctx, &method, &params)))
            .unwrap()
    }

    pub(super) fn code(&self, method: &str, params: Value) -> u16 {
        self.call(method, params)
            .map_or_else(|e| e.code.as_u16(), |_| 0)
    }

    pub(super) fn grant(&self, action: &str, target: &str) {
        set(
            &self.ctx.db,
            self.ctx.device,
            PermissionSetArgs {
                extension_id: self.ctx.session.extension_id.to_string(),
                kind: "mail".into(),
                action: action.into(),
                target: target.into(),
                status: "granted".into(),
                replaces: None,
            },
            2,
        )
        .unwrap();
    }

    pub(super) fn imap(&self) -> Value {
        json!({ "host": "127.0.0.1", "port": self.imap_port, "security": "none", "username": USER, "password": PASSWORD })
    }

    pub(super) fn smtp(&self) -> Value {
        json!({ "host": "127.0.0.1", "port": self.smtp_port, "security": "none", "username": USER, "password": PASSWORD })
    }

    pub(super) fn granted() -> Self {
        let s = setup();
        s.grant("fetch", &format!("127.0.0.1:{}", s.imap_port));
        s.grant("send", &format!("127.0.0.1:{}", s.smtp_port));
        s
    }

    pub(super) fn uids(&self, mailbox: &str) -> Vec<u32> {
        let state = self.server.lock().unwrap();
        state.boxes[mailbox].iter().map(|m| m.uid).collect()
    }
}

pub(super) fn outgoing(subject: &str) -> Value {
    json!({
        "from": { "name": "Anna", "email": "anna@example.org" },
        "to": [{ "email": "ben@example.org" }],
        "bcc": [{ "email": "hidden@example.org" }],
        "subject": subject,
        "bodyText": "Hallo",
        "bodyHtml": "<p>Hallo</p>",
        "attachments": [{ "filename": "a.txt", "contentType": "text/plain", "data": STANDARD.encode("anhang") }],
        "inReplyTo": "first@example.org",
        "references": ["a@x", "<b@x>"],
    })
}
