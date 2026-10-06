//! Remote storage through the bridge against the fake provider (spec 038 US2, T035, T037): the
//! list by name only, permissions per storage, the extension's own area, limits, the mapping of
//! the provider's errors, and that no answer of any of the nine methods carries a credential, the
//! endpoint or the region (SC-002).

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::extensions::bridge::blocking::block_on;
use crate::extensions::bridge::dispatch::{call as bridge_call, CallContext, Emit};
use crate::extensions::commands::permissions::{set, PermissionSetArgs};
use crate::extensions::error::BridgeError;
use crate::extensions::host::ExtensionHost;
use crate::extensions::ids::storage_vault_id;
use crate::extensions::registry::effective::effective_bundle;
use crate::extensions::registry::install::install;
use crate::extensions::remote_storage_dialog::StorageAnswer;
use crate::passwords::service::PasswordsService;
use crate::remote_storage::model::{ConnectionInput, CredentialsInput, StorageInput};
use crate::remote_storage::service::StorageService;
use crate::remote_storage::test_support::{resolver, vault, FakeStore, Op};
use crate::remote_storage::{Addressing, ProviderKind, StorageError};
use crate::storage::known_devices;
use crate::storage::query::Query;
use crate::vault_gate::VaultDb;

pub(crate) const ACCESS_KEY: &str = "AKIDLEAKMARKER";
pub(crate) const SECRET: &str = "secret-leak-marker-5f1e";
pub(crate) const ENDPOINT: &str = "http://127.0.0.1:9000";
pub(crate) const REGION: &str = "region-leak-marker";

/// Records events and answers storage dialogs with the answer set for the test.
pub(crate) struct Events {
    pub host: Arc<ExtensionHost>,
    pub seen: Mutex<Vec<(String, Value)>>,
    pub answer: Mutex<Option<StorageAnswer>>,
    /// Close the frame instead of answering.
    pub close_frame: Mutex<bool>,
}

impl Emit for Events {
    fn emit(&self, event: &str, payload: Value) {
        self.seen
            .lock()
            .expect("lock")
            .push((event.to_owned(), payload.clone()));
        if event != "extension-storage-request" {
            return;
        }
        let request_id = payload["requestId"].as_str().unwrap_or_default().to_owned();
        if *self.close_frame.lock().expect("lock") {
            self.host
                .drop_dialogs_of(payload["frame"].as_str().unwrap_or_default());
            return;
        }
        let answer = self
            .answer
            .lock()
            .expect("lock")
            .clone()
            .unwrap_or(StorageAnswer::Cancel);
        self.host.storage.resolve(&request_id, answer);
    }
}

impl Events {
    pub fn dialogs(&self) -> Vec<Value> {
        self.seen
            .lock()
            .expect("lock")
            .iter()
            .filter(|(event, _)| event == "extension-storage-request")
            .map(|(_, payload)| payload.clone())
            .collect()
    }

    pub fn answer_with(&self, answer: StorageAnswer) {
        *self.answer.lock().expect("lock") = Some(answer);
    }
}

pub(crate) struct Setup {
    _dir: tempfile::TempDir,
    pub vault: VaultDb,
    pub fake: Arc<FakeStore>,
    pub events: Arc<Events>,
    pub ctx: CallContext,
    /// Two storages the user made in the settings, on one connection.
    pub photos: String,
    pub notes: String,
}

pub(crate) fn credentials() -> CredentialsInput {
    CredentialsInput {
        access_key_id: ACCESS_KEY.to_owned(),
        secret_access_key: Zeroizing::new(SECRET.to_owned()),
        session_token: None,
    }
}

pub(crate) fn setup() -> Setup {
    let (dir, vault) = vault();
    vault
        .write_blocking(|tx| crate::sync::keys::ensure_vault_identity(tx, true).map(|_| ()))
        .expect("vault identity");
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .expect("devices")[0]
        .vault_device_uuid;
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension_bundles/good-minimal.xt"),
    )
    .expect("fixture");
    let extension = install(&vault, &bytes, vec![], false, device, 1)
        .expect("install")
        .ids
        .extension_id;
    let bundle = vault
        .read_blocking(move |q| effective_bundle(q, extension).map_err(Into::into))
        .expect("bundle")
        .expect("effective")
        .bundle_id;
    let fake = Arc::new(FakeStore::new());
    let host = Arc::new(ExtensionHost::default());
    host.storage.set(fake.clone(), resolver(&[]));
    let events = Arc::new(Events {
        host: Arc::clone(&host),
        seen: Mutex::new(Vec::new()),
        answer: Mutex::new(None),
        close_frame: Mutex::new(false),
    });
    let service = StorageService::new(
        vault.clone(),
        PasswordsService::new(vault.clone()),
        fake.clone(),
        resolver(&[]),
    );
    let connection = block_on(service.save_connection(ConnectionInput {
        id: None,
        provider_name: "RustFS".to_owned(),
        provider_kind: ProviderKind::Rustfs,
        endpoint: Some(ENDPOINT.to_owned()),
        region: REGION.to_owned(),
        addressing: Addressing::Path,
        credentials: Some(credentials()),
        bucket_for_test: "photos".to_owned(),
    }))
    .expect("connection");
    let storage = |name: &str, bucket: &str| {
        block_on(service.save_storage(StorageInput {
            id: None,
            connection_id: connection.id.clone(),
            name: name.to_owned(),
            bucket: bucket.to_owned(),
        }))
        .expect("storage")
        .id
    };
    let photos = storage("Fotos", "photos");
    let notes = storage("Notizen", "notes");
    Setup {
        _dir: dir,
        ctx: CallContext {
            db: vault.clone(),
            session: host.frames.open(extension, bundle, "tab"),
            host,
            device,
            emitter: events.clone(),
        },
        vault,
        fake,
        events,
        photos,
        notes,
    }
}

impl Setup {
    pub fn call(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        bridge_call(&self.ctx, method, &params)
    }

    pub fn code(&self, method: &str, params: Value) -> u16 {
        self.call(method, params)
            .map_or_else(|e| e.code.as_u16(), |_| 0)
    }

    pub fn permit(&self, action: &str, target: &str, status: &str) {
        set(
            &self.vault,
            self.ctx.device,
            PermissionSetArgs {
                extension_id: self.ctx.session.extension_id.to_string(),
                kind: "remoteStorage".into(),
                action: action.into(),
                target: target.into(),
                status: status.into(),
                all_devices: true,
                replaces: None,
            },
            0,
        )
        .expect("permit");
    }

    /// The extension's area in every bucket.
    pub fn prefix(&self) -> String {
        let pubkey = self
            .vault
            .read_blocking(|q| crate::sync::keys::vault_pubkey(q))
            .expect("read")
            .expect("identity");
        format!(
            "holzi-ext/{}/{}/",
            storage_vault_id(&pubkey),
            self.ctx.session.extension_id
        )
    }

    pub fn count(&self, sql: &str) -> i64 {
        let sql = sql.to_owned();
        self.vault
            .read_blocking(move |q| Ok(q.query_row(&sql, &[], |r| r.get(0))?.unwrap_or(0)))
            .expect("count")
    }
}

fn b64(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn object(storage: &str, key: &str) -> Value {
    json!({ "request": { "backendId": storage, "key": key } })
}

#[test]
fn the_list_names_only_the_storages_a_read_permission_covers() {
    let s = setup();
    assert_eq!(
        s.code("extension_remote_storage_list_backends", json!({})),
        1004,
        "without any read permission the extension is asked for one"
    );
    s.permit("read", &s.photos, "granted");
    let listed = s
        .call("extension_remote_storage_list_backends", json!({}))
        .expect("list");
    assert_eq!(
        listed,
        json!([{
            "id": s.photos,
            "type": "s3",
            "name": "Fotos",
            "providerName": "RustFS",
            "bucket": "photos",
        }]),
        "names only (FR-009a)"
    );
}

#[test]
fn objects_go_to_the_extensions_area_and_come_back_without_it() {
    let s = setup();
    s.permit("readWrite", &s.photos, "granted");
    let upload =
        json!({ "request": { "backendId": s.photos, "key": "a/b.txt", "data": b64(b"hello") } });
    s.call("extension_remote_storage_upload", upload)
        .expect("upload");
    assert_eq!(s.fake.keys("photos"), [format!("{}a/b.txt", s.prefix())]);

    let downloaded = s
        .call(
            "extension_remote_storage_download",
            object(&s.photos, "a/b.txt"),
        )
        .expect("download");
    assert_eq!(downloaded, json!(b64(b"hello")));

    let listed = s
        .call(
            "extension_remote_storage_list",
            json!({ "request": { "backendId": s.photos, "prefix": "a/" } }),
        )
        .expect("list");
    assert_eq!(listed[0]["key"], json!("a/b.txt"));
    assert_eq!(listed.as_array().map(Vec::len), Some(1));

    s.call(
        "extension_remote_storage_delete",
        object(&s.photos, "a/b.txt"),
    )
    .expect("delete");
    assert!(s.fake.keys("photos").is_empty());
}

#[test]
fn read_alone_does_not_write_and_an_uncovered_storage_stays_unknown() {
    let s = setup();
    s.permit("read", &s.photos, "granted");
    let puts = || {
        s.fake
            .calls()
            .iter()
            .filter(|(op, _)| *op == Op::Put)
            .count()
    };
    let before = puts();
    let upload = json!({ "request": { "backendId": s.photos, "key": "x", "data": b64(b"x") } });
    assert_eq!(s.code("extension_remote_storage_upload", upload), 1004);
    assert_eq!(
        s.code("extension_remote_storage_download", object(&s.notes, "x")),
        1004,
        "another storage needs its own permission"
    );
    assert_eq!(
        s.code(
            "extension_remote_storage_download",
            object("no-such-storage", "x")
        ),
        1004,
        "without a permission the extension does not learn that a storage is missing"
    );
    s.permit("read", "*", "granted");
    assert_eq!(
        s.code(
            "extension_remote_storage_download",
            object("no-such-storage", "x")
        ),
        1001
    );
    s.permit("read", &s.notes, "denied");
    assert_eq!(
        s.code("extension_remote_storage_download", object(&s.notes, "x")),
        1002
    );
    assert_eq!(puts(), before, "nothing was written");
}

#[test]
fn keys_that_leave_the_area_are_refused_before_the_provider() {
    let s = setup();
    s.permit("readWrite", &s.photos, "granted");
    let before = s.fake.calls().len();
    for key in ["../x", "/abs", "a//b", "a\\b", "a/./b", ""] {
        assert_eq!(
            s.code("extension_remote_storage_download", object(&s.photos, key)),
            3001,
            "{key:?}"
        );
    }
    assert_eq!(
        s.code(
            "extension_remote_storage_list",
            json!({ "request": { "backendId": s.photos, "prefix": "../" } })
        ),
        3001
    );
    assert_eq!(s.fake.calls().len(), before, "nothing reached the provider");
}

#[test]
fn keys_of_the_provider_outside_the_area_are_not_listed() {
    let s = setup();
    s.permit("readWrite", &s.photos, "granted");
    let access = block_on(
        StorageService::new(
            s.vault.clone(),
            PasswordsService::new(s.vault.clone()),
            s.fake.clone(),
            resolver(&[]),
        )
        .access_of(&s.photos),
    )
    .expect("access");
    let soon = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    use crate::remote_storage::RemoteStore;
    block_on(
        s.fake
            .put(&access, "elsewhere/secret.txt", b"x".to_vec(), soon),
    )
    .expect("put");
    let other_area = format!("holzi-ext/{}/{}/x", Uuid::nil(), Uuid::nil());
    block_on(s.fake.put(&access, &other_area, b"x".to_vec(), soon)).expect("put");
    let listed = s
        .call(
            "extension_remote_storage_list",
            json!({ "request": { "backendId": s.photos } }),
        )
        .expect("list");
    assert_eq!(listed, json!([]));
}

#[test]
fn limits_of_the_extension_hold_for_storage_calls() {
    let s = setup();
    s.permit("readWrite", &s.photos, "granted");
    let id = s.ctx.session.extension_id.to_string();
    s.vault
        .write_blocking(move |tx| {
            tx.execute(
                "UPDATE extension_limits SET max_response_bytes = 8, max_rows = 1 \
                 WHERE extension_id = ?1",
                &[&id],
            )?;
            Ok(())
        })
        .expect("limits");
    let big = json!({ "request": { "backendId": s.photos, "key": "big", "data": b64(&[1; 12]) } });
    assert_eq!(s.code("extension_remote_storage_upload", big), 7000);
    let small = json!({ "request": { "backendId": s.photos, "key": "a", "data": b64(&[1; 6]) } });
    s.call("extension_remote_storage_upload", small)
        .expect("fits");
    assert_eq!(
        s.code("extension_remote_storage_download", object(&s.photos, "a")),
        0,
        "6 bytes are 8 in base64"
    );
    let second = json!({ "request": { "backendId": s.photos, "key": "b", "data": b64(&[1; 3]) } });
    s.call("extension_remote_storage_upload", second)
        .expect("fits");
    let listed = s.call(
        "extension_remote_storage_list",
        json!({ "request": { "backendId": s.photos } }),
    );
    assert_eq!(
        listed.map_err(|e| e.code.as_u16()),
        Err(7000),
        "more than max_rows"
    );
}

#[test]
fn errors_of_the_provider_reach_the_extension_as_kinds() {
    let s = setup();
    s.permit("readWrite", &s.photos, "granted");
    let kind = |error: BridgeError| (error.code.as_u16(), error.details);
    s.fake.fail(Op::Get, StorageError::NotFound);
    assert_eq!(
        s.code("extension_remote_storage_download", object(&s.photos, "x")),
        1001
    );
    s.fake.fail(Op::Get, StorageError::Network);
    assert_eq!(
        kind(
            s.call("extension_remote_storage_download", object(&s.photos, "x"))
                .unwrap_err()
        ),
        (2002, Some(json!({ "kind": "network" })))
    );
    s.fake.fail(Op::Get, StorageError::AccessDenied);
    assert_eq!(
        kind(
            s.call("extension_remote_storage_download", object(&s.photos, "x"))
                .unwrap_err()
        ),
        (2002, Some(json!({ "kind": "accessDenied" })))
    );
    assert_eq!(
        s.count("SELECT COUNT(*) FROM storage_tests_no_sync WHERE outcome = 'accessDenied'"),
        1,
        "the settings show the hint"
    );
}

#[test]
fn deleted_credentials_are_access_denied_for_the_extension() {
    let s = setup();
    s.permit("readWrite", &s.photos, "granted");
    s.vault
        .write_blocking(|tx| {
            tx.execute("DELETE FROM haex_passwords_item_details", &[])?;
            Ok(())
        })
        .expect("the user deletes the entry");
    let error = s
        .call("extension_remote_storage_download", object(&s.photos, "x"))
        .unwrap_err();
    assert_eq!(
        (error.code.as_u16(), error.details),
        (2002, Some(json!({ "kind": "accessDenied" })))
    );
}

/// SC-002: no answer or error of the nine methods carries a credential, the endpoint or the
/// region, also when the provider fails.
#[test]
fn no_answer_carries_credentials_endpoint_or_region() {
    let s = setup();
    s.permit("readWrite", "*", "granted");
    s.permit("add", "*", "granted");
    s.events.answer_with(StorageAnswer::Confirm {
        connection_id: None,
        credentials: None,
        name: None,
        bucket: None,
    });
    let calls = [
        ("extension_remote_storage_list_backends", json!({})),
        (
            "extension_remote_storage_upload",
            json!({ "request": { "backendId": s.photos, "key": "k", "data": b64(b"v") } }),
        ),
        ("extension_remote_storage_download", object(&s.photos, "k")),
        (
            "extension_remote_storage_list",
            json!({ "request": { "backendId": s.photos } }),
        ),
        ("extension_remote_storage_delete", object(&s.photos, "k")),
        (
            "extension_remote_storage_add_backend",
            json!({ "request": { "name": "Neu", "type": "s3", "sameProviderAs": s.photos,
                "config": { "bucket": "fresh" } } }),
        ),
        (
            "extension_remote_storage_update_backend",
            json!({ "request": { "backendId": s.notes, "name": "Umbenannt" } }),
        ),
        (
            "extension_remote_storage_test_backend",
            json!({ "backendId": s.notes }),
        ),
        (
            "extension_remote_storage_remove_backend",
            json!({ "backendId": s.notes }),
        ),
    ];
    let markers = [ACCESS_KEY, SECRET, "127.0.0.1", REGION];
    for failing in [
        None,
        Some(StorageError::AccessDenied),
        Some(StorageError::Network),
    ] {
        if let Some(error) = failing {
            for op in [Op::Put, Op::Get, Op::List, Op::Delete] {
                s.fake.fail(op, error);
            }
        }
        for (method, params) in &calls {
            let answer = match s.call(method, params.clone()) {
                Ok(value) => value.to_string(),
                Err(error) => serde_json::to_string(&error).expect("json"),
            };
            for marker in markers {
                assert!(
                    !answer.contains(marker),
                    "{method} ({failing:?}) leaks {marker}: {answer}"
                );
            }
        }
    }
}
