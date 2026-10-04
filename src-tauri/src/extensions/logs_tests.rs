// The tests count log rows directly.
#![allow(clippy::disallowed_methods)]

use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::extensions::bridge::dispatch::{call, Emit};
use crate::extensions::host::ExtensionHost;
use crate::extensions::registry::effective::effective_bundle;
use crate::extensions::registry::install::install;
use crate::passwords::test_support::open_test_vault;
use crate::storage::known_devices;
use crate::vault_gate::{VaultDb, VaultGate};

struct Silent;

impl Emit for Silent {
    fn emit(&self, _event: &str, _payload: Value) {}
}

fn context(vault: &VaultDb, host: &Arc<ExtensionHost>, name: &str, device: Uuid) -> CallContext {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension_bundles")
            .join(name),
    )
    .unwrap();
    let extension = install(vault, &bytes, vec![], false, device, 1)
        .unwrap()
        .ids
        .extension_id;
    let bundle = vault
        .read_blocking(move |q| effective_bundle(q, extension).map_err(Into::into))
        .unwrap()
        .unwrap()
        .bundle_id;
    CallContext {
        db: vault.clone(),
        host: Arc::clone(host),
        session: host.frames.open(extension, bundle, "tab"),
        device,
        emitter: Arc::new(Silent),
    }
}

fn two_extensions() -> (tempfile::TempDir, CallContext, CallContext) {
    let (dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db)).unwrap();
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .unwrap()[0]
        .vault_device_uuid;
    let host = Arc::new(ExtensionHost::default());
    let notes = context(&vault, &host, "good-notes-like.xt", device);
    let minimal = context(&vault, &host, "good-minimal.xt", device);
    (dir, notes, minimal)
}

fn write(ctx: &CallContext, level: &str, message: &str) -> Result<Value, BridgeError> {
    call(
        ctx,
        "extension_logging_write",
        &json!({ "level": level, "message": message }),
    )
}

fn messages(answer: Value) -> Vec<String> {
    answer
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn an_extension_reads_its_own_entries_newest_first() {
    let (_dir, notes, minimal) = two_extensions();
    write(&notes, "info", "started").unwrap();
    call(
        &notes,
        "extension_logging_write",
        &json!({ "level": "error", "message": "failed", "metadata": { "code": 7 } }),
    )
    .unwrap();
    write(&minimal, "warn", "not for notes").unwrap();

    let all = call(&notes, "extension_logging_read", &json!({})).unwrap();
    assert_eq!(messages(all.clone()), ["failed", "started"]);
    assert_eq!(all[0]["metadata"], json!("{\"code\":7}"));
    let errors = call(
        &notes,
        "extension_logging_read",
        &json!({ "level": "error" }),
    )
    .unwrap();
    assert_eq!(messages(errors), ["failed"]);
    let paged = call(
        &notes,
        "extension_logging_read",
        &json!({ "limit": 1, "offset": 1 }),
    )
    .unwrap();
    assert_eq!(messages(paged), ["started"]);
}

#[test]
fn levels_sizes_and_paging_values_are_checked() {
    let (_dir, notes, _) = two_extensions();
    let code = |outcome: Result<Value, BridgeError>| outcome.unwrap_err().code.as_u16();
    assert_eq!(code(write(&notes, "fatal", "x")), 3001);
    assert_eq!(
        code(write(&notes, "info", &"m".repeat(MAX_MESSAGE_BYTES + 1))),
        7000
    );
    assert_eq!(
        code(call(
            &notes,
            "extension_logging_write",
            &json!({ "level": "info", "message": "m", "metadata": "x".repeat(MAX_METADATA_BYTES) })
        )),
        7000
    );
    assert_eq!(
        code(call(
            &notes,
            "extension_logging_read",
            &json!({ "limit": -1 })
        )),
        3001
    );
}

#[test]
fn only_the_newest_entries_of_an_extension_stay() {
    let (_dir, notes, minimal) = two_extensions();
    write(&minimal, "info", "kept").unwrap();
    let ext = notes.session.extension_id.to_string();
    let device = notes.device.to_string();
    // An entry under another device id, as in a vault file copied from elsewhere.
    let elsewhere = ext.clone();
    notes
        .db
        .write_blocking(move |tx| {
            tx.execute(
                "INSERT INTO extension_logs_no_sync \
                 (extension_id, vault_device_uuid, level, message, created_at) \
                 VALUES (?1, ?2, 'info', 'from elsewhere', 0)",
                params![elsewhere, Uuid::new_v4().to_string()],
            )?;
            Ok(())
        })
        .unwrap();
    // Fill up to the limit directly; the next write through the bridge drops the oldest.
    notes
        .db
        .write_blocking(move |tx| {
            for i in 0..MAX_ENTRIES {
                tx.execute(
                    "INSERT INTO extension_logs_no_sync \
                     (extension_id, vault_device_uuid, level, message, created_at) \
                     VALUES (?1, ?2, 'debug', ?3, 1)",
                    params![ext, device, format!("old {i}")],
                )?;
            }
            Ok(())
        })
        .unwrap();
    write(&notes, "info", "newest").unwrap();

    let ext = notes.session.extension_id;
    let device = notes.device;
    let count = |extension: Uuid| {
        notes
            .db
            .read_blocking(move |q| {
                q.query_row(
                    "SELECT COUNT(*) FROM extension_logs_no_sync WHERE extension_id = ?1",
                    params![extension.to_string()],
                    |r| r.get::<_, i64>(0),
                )
            })
            .unwrap()
            .unwrap_or(0)
    };
    assert_eq!(
        count(ext),
        MAX_ENTRIES + 1,
        "the other device's entry stays"
    );
    assert_eq!(
        count(minimal.session.extension_id),
        1,
        "other extensions keep theirs"
    );
    let query = LogQuery {
        level: None,
        limit: 1,
        offset: 0,
        before: None,
    };
    let newest = notes
        .db
        .read_blocking(move |q| read(q, ext, device, &query))
        .unwrap();
    assert_eq!(newest[0].message, "newest");
    let oldest_left = notes
        .db
        .read_blocking(move |q| {
            q.query_row(
                "SELECT message FROM extension_logs_no_sync \
                 WHERE extension_id = ?1 AND vault_device_uuid = ?2 ORDER BY id LIMIT 1",
                params![ext.to_string(), device.to_string()],
                |r| r.get::<_, String>(0),
            )
        })
        .unwrap();
    assert_eq!(oldest_left.as_deref(), Some("old 1"), "the oldest one went");
}
