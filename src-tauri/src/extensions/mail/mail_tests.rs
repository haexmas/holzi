//! Mail of extensions (spec 017, US11, T107) through the bridge, against the test server of
//! `test_server.rs` on this device.

// The tests change limits rows directly.
#![allow(clippy::disallowed_methods)]

use std::path::Path;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use haex_crdt::rusqlite::params;
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

struct Silent;

impl Emit for Silent {
    fn emit(&self, _event: &str, _payload: Value) {}
}

struct Setup {
    _dir: tempfile::TempDir,
    rt: tokio::runtime::Runtime,
    ctx: Arc<CallContext>,
    server: Shared,
    imap_port: u16,
    smtp_port: u16,
}

fn setup() -> Setup {
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
            emitter: Arc::new(Silent),
        }),
        server,
        imap_port,
        smtp_port,
    }
}

impl Setup {
    fn call(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        let ctx = Arc::clone(&self.ctx);
        let method = method.to_owned();
        self.rt
            .block_on(self.rt.spawn_blocking(move || call(&ctx, &method, &params)))
            .unwrap()
    }

    fn code(&self, method: &str, params: Value) -> u16 {
        self.call(method, params)
            .map_or_else(|e| e.code.as_u16(), |_| 0)
    }

    fn grant(&self, action: &str, target: &str) {
        set(
            &self.ctx.db,
            self.ctx.device,
            PermissionSetArgs {
                extension_id: self.ctx.session.extension_id.to_string(),
                kind: "mail".into(),
                action: action.into(),
                target: target.into(),
                status: "granted".into(),
                all_devices: false,
                replaces: None,
            },
            2,
        )
        .unwrap();
    }

    fn imap(&self) -> Value {
        json!({ "host": "127.0.0.1", "port": self.imap_port, "security": "none", "username": USER, "password": PASSWORD })
    }

    fn smtp(&self) -> Value {
        json!({ "host": "127.0.0.1", "port": self.smtp_port, "security": "none", "username": USER, "password": PASSWORD })
    }

    fn granted() -> Self {
        let s = setup();
        s.grant("fetch", &format!("127.0.0.1:{}", s.imap_port));
        s.grant("send", &format!("127.0.0.1:{}", s.smtp_port));
        s
    }

    fn uids(&self, mailbox: &str) -> Vec<u32> {
        let state = self.server.lock().unwrap();
        state.boxes[mailbox].iter().map(|m| m.uid).collect()
    }
}

fn outgoing(subject: &str) -> Value {
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

#[test]
fn mailboxes_envelopes_messages_and_attachments_come_from_the_granted_server() {
    let s = Setup::granted();
    let boxes = s
        .call(
            "extension_mail_list_mailboxes",
            json!({ "imap": s.imap(), "includeStatus": true }),
        )
        .unwrap();
    let inbox = boxes
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == "INBOX")
        .unwrap()
        .clone();
    assert_eq!(
        (inbox["exists"].clone(), inbox["unseen"].clone()),
        (json!(2), json!(1))
    );

    let envelopes = s
        .call(
            "extension_mail_fetch_envelopes",
            json!({ "imap": s.imap(), "mailbox": "INBOX", "range": { "type": "latest", "count": 10 } }),
        )
        .unwrap();
    let envelopes = envelopes.as_array().unwrap();
    assert_eq!(envelopes.len(), 2);
    let second = envelopes.iter().find(|e| e["uid"] == 9).unwrap();
    assert_eq!(second["subject"], "second");
    assert_eq!(
        second["from"][0],
        json!({ "name": "Anna", "email": "anna@example.org" })
    );
    assert_eq!(second["references"], json!(["a@x", "b@x"]));
    assert_eq!(second["hasAttachments"], true);
    assert_eq!(
        envelopes.iter().find(|e| e["uid"] == 7).unwrap()["hasAttachments"],
        false
    );

    let message = s
        .call(
            "extension_mail_fetch_message",
            json!({ "imap": s.imap(), "mailbox": "INBOX", "uid": 9 }),
        )
        .unwrap();
    assert_eq!(message["bodyText"].as_str().unwrap().trim(), "Hallo grün");
    assert_eq!(message["attachments"][0]["filename"], "a.pdf");
    assert_eq!(message["attachments"][0]["contentType"], "application/pdf");
    let attachment = s
        .call(
            "extension_mail_fetch_attachment",
            json!({ "imap": s.imap(), "mailbox": "INBOX", "uid": 9, "partIndex": 0 }),
        )
        .unwrap();
    assert_eq!(
        STANDARD.decode(attachment.as_str().unwrap()).unwrap(),
        b"PDF-data"
    );
    assert_eq!(
        s.code(
            "extension_mail_fetch_message",
            json!({ "imap": s.imap(), "mailbox": "INBOX", "uid": 99 })
        ),
        1001
    );
}

#[test]
fn flags_moves_and_appends_change_the_server() {
    let s = Setup::granted();
    s.call(
        "extension_mail_set_flags",
        json!({ "imap": s.imap(), "mailbox": "INBOX", "uids": [9], "flags": ["\\Seen", "$Important"], "add": true }),
    )
    .unwrap();
    {
        let state = s.server.lock().unwrap();
        let flags = &state.boxes["INBOX"]
            .iter()
            .find(|m| m.uid == 9)
            .unwrap()
            .flags;
        assert!(flags.contains(&"\\Seen".to_owned()) && flags.contains(&"$Important".to_owned()));
    }
    assert_eq!(
        s.code(
            "extension_mail_set_flags",
            json!({ "imap": s.imap(), "mailbox": "INBOX", "uids": [9], "flags": ["x)\r\nA9 DELETE INBOX"], "add": true }),
        ),
        3001
    );
    assert!(
        !s.server
            .lock()
            .unwrap()
            .commands
            .iter()
            .any(|c| c.contains("DELETE")),
        "nothing smuggled into the server"
    );

    s.call(
        "extension_mail_move_messages",
        json!({ "imap": s.imap(), "sourceMailbox": "INBOX", "destinationMailbox": "Archive", "uids": [7] }),
    )
    .unwrap();
    assert_eq!(s.uids("INBOX"), [9]);
    assert_eq!(s.uids("Archive"), [7]);

    let draft = s
        .call(
            "extension_mail_build_rfc822",
            json!({ "imapHost": "127.0.0.1", "message": outgoing("draft") }),
        )
        .unwrap();
    s.call(
        "extension_mail_append_message",
        json!({ "imap": s.imap(), "mailbox": "Archive", "rfc822Base64": draft, "flags": ["\\Draft"] }),
    )
    .unwrap();
    let state = s.server.lock().unwrap();
    let appended = state.boxes["Archive"].last().unwrap();
    assert_eq!(appended.flags, ["\\Draft"]);
    assert!(String::from_utf8_lossy(&appended.raw).contains("Subject: draft"));
}

#[test]
fn a_message_is_sent_to_every_recipient_and_built_without_hidden_headers() {
    let s = Setup::granted();
    let id = s
        .call(
            "extension_mail_send_message",
            json!({ "smtp": s.smtp(), "message": outgoing("hello") }),
        )
        .unwrap();
    assert!(!id.as_str().unwrap().is_empty());
    let state = s.server.lock().unwrap();
    let (recipients, data) = state.sent.last().unwrap();
    assert_eq!(recipients, &["ben@example.org", "hidden@example.org"]);
    let data = String::from_utf8_lossy(data);
    assert!(data.contains("Subject: hello"));
    assert!(data.contains("In-Reply-To: <first@example.org>"));
    assert!(data.contains("References: <a@x> <b@x>"));
    assert!(
        !data.contains("hidden@example.org"),
        "Bcc is not in the message"
    );
    assert!(data.contains("filename=\"a.txt\""));
    drop(state);

    let mut injected = outgoing("x");
    injected["subject"] = json!("hi\r\nBcc: victim@example.org");
    assert_eq!(
        s.code(
            "extension_mail_build_rfc822",
            json!({ "imapHost": "x", "message": injected })
        ),
        3001
    );
    let mut injected = outgoing("x");
    injected["inReplyTo"] = json!("a@x>\r\nX-Evil: 1");
    assert_eq!(
        s.code(
            "extension_mail_send_message",
            json!({ "smtp": s.smtp(), "message": injected })
        ),
        3001
    );
}

#[test]
fn each_server_and_port_needs_its_own_permission() {
    let s = setup();
    let asked = s
        .call("extension_mail_list_mailboxes", json!({ "imap": s.imap() }))
        .unwrap_err();
    assert_eq!(asked.code.as_u16(), 1004);
    assert_eq!(
        asked.details,
        Some(
            json!({ "resourceType": "mail", "action": "fetch", "target": format!("127.0.0.1:{}", s.imap_port) })
        )
    );
    s.grant("fetch", &format!("127.0.0.1:{}", s.imap_port + 1));
    assert_eq!(
        s.code("extension_mail_list_mailboxes", json!({ "imap": s.imap() })),
        1004,
        "another port"
    );
    s.grant("fetch", "127.0.0.1");
    assert_eq!(
        s.code("extension_mail_list_mailboxes", json!({ "imap": s.imap() })),
        0,
        "every port of the host"
    );
    assert_eq!(
        s.code(
            "extension_mail_send_message",
            json!({ "smtp": s.smtp(), "message": outgoing("x") })
        ),
        1004,
        "a fetch grant does not send"
    );
    assert!(s.server.lock().unwrap().sent.is_empty());
}

#[test]
fn wrong_credentials_unencrypted_remote_servers_and_large_messages_are_refused() {
    let s = Setup::granted();
    let mut wrong = s.imap();
    wrong["password"] = json!("falsch-123");
    let refused = s
        .call("extension_mail_list_mailboxes", json!({ "imap": wrong }))
        .unwrap_err();
    assert_eq!(refused.code.as_u16(), 2005);
    assert!(!refused.message.contains("falsch-123") && !refused.message.contains(USER));

    s.grant("fetch", "imap.example.org:143");
    assert_eq!(
        s.code(
            "extension_mail_list_mailboxes",
            json!({ "imap": { "host": "imap.example.org", "port": 143, "security": "none", "username": "u", "password": "p" } })
        ),
        3001,
        "no plain text to another device"
    );

    let ext = s.ctx.session.extension_id.to_string();
    s.ctx
        .db
        .write_blocking(move |tx| {
            tx.execute(
                "UPDATE extension_limits SET max_response_bytes = 100 WHERE extension_id = ?1",
                params![ext],
            )
            .map(drop)
        })
        .unwrap();
    assert_eq!(
        s.code(
            "extension_mail_fetch_message",
            json!({ "imap": s.imap(), "mailbox": "INBOX", "uid": 9 })
        ),
        7000
    );
}
