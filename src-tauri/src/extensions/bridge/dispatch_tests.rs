// These tests switch an extension off directly in the registry.
#![allow(clippy::disallowed_methods)]

use std::path::Path;
use std::sync::Mutex;

use haex_crdt::Database;

use super::*;
use crate::extensions::bridge::events::{emit_to_frames, FRAME_EVENT};
use crate::extensions::registry::effective::effective_bundle;
use crate::extensions::registry::install::install;
use crate::passwords::test_support::open_test_vault;
use crate::storage::known_devices;
use crate::vault_gate::VaultGate;

#[derive(Default)]
struct Recorded(Mutex<Vec<(String, Value)>>);

impl Emit for Recorded {
    fn emit(&self, event: &str, payload: Value) {
        self.0.lock().unwrap().push((event.to_owned(), payload));
    }
}

struct Setup {
    _dir: tempfile::TempDir,
    db: Database,
    ctx: CallContext,
    recorded: Arc<Recorded>,
}

fn setup() -> Setup {
    let (dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db.clone())).unwrap();
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .unwrap()[0]
        .vault_device_uuid;
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension_bundles/good-notes-like.xt"),
    )
    .unwrap();
    let extension = install(&vault, &bytes, vec![], false, device, 1)
        .unwrap()
        .ids
        .extension_id;
    let bundle_id = vault
        .read_blocking(move |q| effective_bundle(q, extension).map_err(Into::into))
        .unwrap()
        .unwrap()
        .bundle_id;
    let host = Arc::new(ExtensionHost::default());
    let session = host.frames.open(extension, bundle_id, "tab-1");
    let recorded = Arc::new(Recorded::default());
    let ctx = CallContext {
        db: vault,
        host,
        session,
        device,
        emitter: Arc::clone(&recorded) as Arc<dyn Emit>,
    };
    Setup {
        _dir: dir,
        db,
        ctx,
        recorded,
    }
}

fn code(outcome: Result<Value, BridgeError>) -> u16 {
    outcome.unwrap_err().code.as_u16()
}

#[test]
fn context_and_info_describe_holzi_and_the_calling_extension_only() {
    let s = setup();
    s.ctx.host.set_context("dark", "en");
    let context = call(&s.ctx, "extension_context_get", &Value::Null).unwrap();
    assert_eq!(context["theme"], "dark");
    assert_eq!(context["locale"], "en");
    assert_eq!(context["deviceId"], s.ctx.device.to_string());
    assert_eq!(context["platform"], std::env::consts::OS);

    s.ctx.host.set_context("sepia", "de");
    let context = call(&s.ctx, "extension_context_get", &Value::Null).unwrap();
    assert_eq!(context["theme"], "system");

    let info = call(&s.ctx, "extension_get_info", &json!({"name": "other"})).unwrap();
    assert_eq!(info["name"], "notes-like");
    assert_eq!(info["displayName"], "Notes");
    assert_eq!(info["version"], "1.2.0");
}

#[test]
fn tab_attention_marks_only_the_calling_frame() {
    let s = setup();
    call(&s.ctx, "extension_tab_attention", &json!({"active": true})).unwrap();
    let recorded = s.recorded.0.lock().unwrap().clone();
    assert_eq!(
        recorded,
        vec![(
            "extension-tab-attention".to_owned(),
            json!({"frame": s.ctx.session.frame, "active": true})
        )]
    );
    assert_eq!(
        code(call(
            &s.ctx,
            "extension_tab_attention",
            &json!({"active": "yes"})
        )),
        3001
    );
}

#[test]
fn unlisted_methods_are_not_supported_and_later_ones_not_available() {
    let s = setup();
    for method in [
        "extension_space_list",
        "set_auth_token",
        "extension_context_set",
        "extension_signal_ready",
        "extension_permissions_grant",
        "extension_webview_broadcast",
        "__proto__",
        "",
    ] {
        assert_eq!(code(call(&s.ctx, method, &Value::Null)), 8000, "{method}");
    }
    for method in [
        "extension_filesystem_read_file",
        "extension_web_fetch",
        "extension_shell_create",
        "extension_mail_list_accounts",
    ] {
        assert_eq!(code(call(&s.ctx, method, &Value::Null)), 8001, "{method}");
    }
}

#[test]
fn every_call_of_a_disabled_extension_answers_disabled() {
    let s = setup();
    s.db.write(|tx| {
        tx.execute("UPDATE extensions SET enabled = 0", &[])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        code(call(&s.ctx, "extension_context_get", &Value::Null)),
        8002
    );
    assert_eq!(code(call(&s.ctx, "nope", &Value::Null)), 8002);
}

#[test]
fn answers_have_the_sdk_form() {
    assert_eq!(
        answer(json!("1"), Ok(json!(5))),
        json!({"id": "1", "result": 5})
    );
    assert_eq!(
        answer(json!(2), Err(BridgeError::not_supported())),
        json!({"id": 2, "error": {"code": 8000, "message": "not supported"}})
    );
}

#[test]
fn frame_events_reach_only_the_frames_of_that_extension() {
    let s = setup();
    let other = s
        .ctx
        .host
        .frames
        .open(Uuid::new_v4(), Uuid::new_v4(), "tab-2");
    emit_to_frames(
        &*s.recorded,
        &s.ctx.host,
        s.ctx.session.extension_id,
        "extension:permission-resolved",
        &json!({"decision": "granted"}),
    );
    let recorded = s.recorded.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, FRAME_EVENT);
    assert_eq!(recorded[0].1["frame"], s.ctx.session.frame);
    assert_ne!(recorded[0].1["frame"], json!(other.frame));
    assert_eq!(recorded[0].1["type"], "extension:permission-resolved");
}
