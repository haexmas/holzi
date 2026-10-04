// The tests create extension tables and read registry rows directly.
#![allow(clippy::disallowed_methods)]

use std::path::Path;
use std::sync::Mutex;

use haex_crdt::rusqlite::params;
use serde_json::Value;

use super::*;
use crate::extensions::bridge::events::FRAME_EVENT;
use crate::extensions::ids::{ExtensionName, PublicKey, TablePrefix};
use crate::extensions::permissions::store::{self as permission_store, NewPermission};
use crate::extensions::permissions::{
    Action, GrantScope, Permission, PermissionKind, PermissionStatus, Target, VAULT_WIDE,
};
use crate::extensions::registry::effective::effective_bundle;
use crate::extensions::registry::install::install;
use crate::passwords::test_support::open_test_vault;
use crate::storage::known_devices;
use crate::vault_gate::VaultGate;

fn prefix(key: char, name: &str) -> TablePrefix {
    TablePrefix {
        public_key: PublicKey::parse(&key.to_string().repeat(64)).unwrap(),
        name: ExtensionName::parse(name).unwrap(),
    }
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

#[test]
fn an_extension_hears_of_its_own_and_readable_tables_never_of_core_ones() {
    let notes = prefix('a', "notes");
    let calendar = prefix('b', "cal");
    let mut policy = SqlPolicy::own_only(notes.clone(), Uuid::new_v4());
    let changed = set(&[
        &format!("{notes}pages"),
        &format!("{notes}drafts_no_sync"),
        &format!("{calendar}events"),
        "chat_threads",
        "extension_permissions",
    ]);
    assert_eq!(
        readable(&policy, &changed),
        [format!("{notes}drafts_no_sync"), format!("{notes}pages")]
    );

    policy.grants.push(Permission {
        kind: PermissionKind::Database,
        action: Action::Read,
        target: Target::ExtensionTables(calendar.clone()),
        status: PermissionStatus::Granted,
        scope: GrantScope::Vault,
    });
    assert_eq!(
        readable(&policy, &changed),
        [
            format!("{notes}drafts_no_sync"),
            format!("{notes}pages"),
            format!("{calendar}events")
        ]
    );
}

#[test]
fn created_altered_and_dropped_tables_are_schema_changes() {
    let before: Schema = [
        ("kept".to_owned(), Some("CREATE TABLE kept (a)".to_owned())),
        (
            "altered".to_owned(),
            Some("CREATE TABLE altered (a)".to_owned()),
        ),
        (
            "dropped".to_owned(),
            Some("CREATE TABLE dropped (a)".to_owned()),
        ),
    ]
    .into();
    let after: Schema = [
        ("kept".to_owned(), Some("CREATE TABLE kept (a)".to_owned())),
        (
            "altered".to_owned(),
            Some("CREATE TABLE altered (a, b)".to_owned()),
        ),
        (
            "created".to_owned(),
            Some("CREATE TABLE created (a)".to_owned()),
        ),
    ]
    .into();
    assert_eq!(
        schema_changes(&before, &after),
        set(&["altered", "created", "dropped"])
    );
}

#[derive(Default)]
struct Recorded(Mutex<Vec<Value>>);

impl Emit for Recorded {
    fn emit(&self, event: &str, payload: Value) {
        assert_eq!(event, FRAME_EVENT);
        self.0.lock().unwrap().push(payload);
    }
}

impl Recorded {
    /// The tables each frame heard of, by frame.
    fn heard(&self) -> Vec<(String, Value)> {
        self.0
            .lock()
            .unwrap()
            .drain(..)
            .map(|event| {
                assert_eq!(event["type"], TABLES_UPDATED);
                (
                    event["frame"].as_str().unwrap().to_owned(),
                    event["data"]["tables"].clone(),
                )
            })
            .collect()
    }
}

struct Opened {
    id: Uuid,
    frame: String,
    prefix: String,
}

fn open(vault: &VaultDb, host: &ExtensionHost, name: &str, device: Uuid) -> Opened {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension_bundles")
            .join(name),
    )
    .unwrap();
    let id = install(vault, &bytes, vec![], false, device, 1)
        .unwrap()
        .ids
        .extension_id;
    let bundle = vault
        .read_blocking(move |q| effective_bundle(q, id).map_err(Into::into))
        .unwrap()
        .unwrap()
        .bundle_id;
    let (key, name): (String, String) = vault
        .read_blocking(move |q| {
            q.query_row(
                "SELECT public_key, name FROM extensions WHERE id = ?1",
                params![id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })
        .unwrap()
        .unwrap();
    Opened {
        id,
        frame: host.frames.open(id, bundle, "tab").frame.clone(),
        prefix: format!("{key}__{name}__"),
    }
}

#[test]
fn each_open_extension_hears_what_it_may_read_and_a_migration_its_new_table() {
    let (_dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db.clone())).unwrap();
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .unwrap()[0]
        .vault_device_uuid;
    let host = ExtensionHost::default();
    let notes = open(&vault, &host, "good-notes-like.xt", device);
    let minimal = open(&vault, &host, "good-minimal.xt", device);
    let recorded = Recorded::default();

    let notes_pages = format!("{}pages", notes.prefix);
    let minimal_items = format!("{}items", minimal.prefix);
    notify(
        &vault,
        &host,
        &recorded,
        device,
        &set(&[&notes_pages, &minimal_items, "chat_threads"]),
    );
    let mut heard = recorded.heard();
    heard.sort_by(|a, b| a.0.cmp(&b.0));
    let mut expected = vec![
        (notes.frame.clone(), serde_json::json!([notes_pages])),
        (minimal.frame.clone(), serde_json::json!([minimal_items])),
    ];
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(heard, expected);

    // Reading the other extension's tables, notes hears of them too.
    let minimal_prefix = minimal.prefix.clone();
    let notes_id = notes.id;
    vault
        .write_blocking(move |tx| {
            permission_store::put(
                tx,
                notes_id,
                &NewPermission {
                    kind: "database",
                    action: "read",
                    target: &format!("{minimal_prefix}*"),
                    status: "granted",
                    declared: false,
                    vault_device_uuid: VAULT_WIDE,
                },
                1,
            )
            .map_err(Into::into)
        })
        .unwrap();
    notify(&vault, &host, &recorded, device, &set(&[&minimal_items]));
    assert_eq!(recorded.heard().len(), 2, "both frames hear of it");

    // A migration of minimal: its journal row is all the commit report shows.
    let mut known = vault.read_blocking(|q| schema(q)).unwrap();
    let created = format!("{}later", minimal.prefix);
    let sql = format!("CREATE TABLE \"{created}\" (id TEXT PRIMARY KEY)");
    db.write(|tx| tx.execute(&sql, &[]).map(drop)).unwrap();
    handle(
        &vault,
        &host,
        &recorded,
        device,
        &mut known,
        Batch {
            tables: set(&[MIGRATION_JOURNAL]),
            lagged: false,
        },
    );
    let heard = recorded.heard();
    assert!(
        heard
            .iter()
            .any(|(frame, tables)| *frame == minimal.frame
                && *tables == serde_json::json!([created])),
        "{heard:?}"
    );
    assert!(known.contains_key(&created), "the snapshot follows");
}
