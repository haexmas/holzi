//! The life cycle of an extension across devices (spec 017, T076, research R11): two or three
//! devices on the real pull path of `sync::test_support`, each following the registry with
//! [`reconcile`].

// The tests read and change rows directly to observe and disturb each device.
#![allow(clippy::disallowed_methods)]

use std::sync::Arc;

use ed25519_dalek::SigningKey;
use haex_bundle::jcs::parse_restricted;
use haex_bundle::{build_archive, Entry};
use haex_crdt::rusqlite::params;
use sha2::{Digest, Sha256};

use super::*;
use crate::extensions::host::ExtensionHost;
use crate::extensions::permissions::store::{self as permission_store, NewPermission};
use crate::extensions::permissions::PermissionKind;
use crate::extensions::registry::install::install;
use crate::extensions::registry::remove::remove;
use crate::storage::known_devices;
use crate::storage::query;
use crate::sync::test_support::Device;
use crate::vault_gate::VaultGate;

/// Signs the bundles of these tests only; derived from a public seed, never trusted elsewhere.
fn key() -> SigningKey {
    let seed = Sha256::digest(b"holzi spec 017 lifecycle tests only, never use for signing");
    SigningKey::from_bytes(&seed.into())
}

fn public_key() -> String {
    key()
        .verifying_key()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The table of the test extension.
fn items() -> String {
    format!("{}__tasks__items", public_key())
}

const INIT: &str = "CREATE TABLE `{t}` (`id` text PRIMARY KEY NOT NULL, `label` text);";
const TAG: &str = "ALTER TABLE `{t}` ADD `tag` text;";

/// The extension `tasks` at `version` with `migrations` (`{t}` stands for its table).
fn bundle(version: &str, migrations: &[&str]) -> Vec<u8> {
    let manifest = parse_restricted(&format!(
        r#"{{"name":"tasks","version":"{version}","displayName":"Tasks","migrationsDir":"db"}}"#
    ))
    .expect("manifest");
    let mut files = vec![Entry {
        path: "index.html".into(),
        data: format!("<!doctype html><title>Tasks {version}</title>").into_bytes(),
    }];
    for (i, sql) in migrations.iter().enumerate() {
        files.push(Entry {
            path: format!("db/{i:04}_step.sql"),
            data: sql.replace("{t}", &items()).into_bytes(),
        });
    }
    build_archive(files, manifest, &key()).expect("bundle")
}

/// One device with what the extension host keeps for it.
struct Node {
    device: Device,
    vault: VaultDb,
    host: ExtensionHost,
    me: Uuid,
}

impl Node {
    fn new() -> Self {
        let device = Device::new();
        let vault = VaultGate::new()
            .vault_db(Arc::new(device.db().clone()))
            .expect("vault");
        let me = vault
            .read_blocking(|q| known_devices::list_devices(q))
            .expect("devices")[0]
            .vault_device_uuid;
        Self {
            device,
            vault,
            host: ExtensionHost::default(),
            me,
        }
    }

    fn install(&self, bytes: &[u8]) -> Uuid {
        install(&self.vault, bytes, vec![], false, self.me, now())
            .expect("install")
            .ids
            .extension_id
    }

    fn follow(&self) -> Vec<ExtensionStatusChanged> {
        reconcile(&self.vault, &self.host, self.me, now()).expect("reconcile")
    }

    fn pull(&self, from: &Node) {
        self.device.pull_from(&from.device);
    }

    fn status(&self, extension: Uuid) -> Option<(String, Option<String>)> {
        let id = crate::extensions::ids::device_status_id(extension, self.me).to_string();
        query::read(self.device.db(), |r| {
            r.query_row(
                "SELECT status, bundle_id FROM extension_device_status WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
        })
        .expect("status")
    }

    fn write(&self, sql: &str) {
        let sql = sql.replace("{t}", &items());
        self.device
            .db()
            .write(|tx| tx.execute(&sql, &[]).map(drop))
            .expect("write");
    }

    fn count(&self, sql: &str) -> Option<i64> {
        let sql = sql.replace("{t}", &items());
        query::read(self.device.db(), |r| {
            r.query_row(&sql, &[], |row| row.get(0))
        })
        .ok()?
    }

    fn has_items(&self) -> bool {
        let table = items();
        query::read(self.device.db(), |r| {
            r.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
                params![table],
                |row| row.get::<_, i64>(0),
            )
        })
        .expect("schema")
            == Some(1)
    }

    fn journal(&self) -> i64 {
        self.count("SELECT COUNT(*) FROM extension_migrations_applied_no_sync")
            .unwrap_or(0)
    }

    fn parked(&self) -> i64 {
        self.count("SELECT COUNT(*) FROM sync_parked_groups_no_sync")
            .unwrap_or(0)
    }
}

fn now() -> i64 {
    crate::passwords::clock::unix_millis(std::time::SystemTime::now())
}

fn ready(node: &Node, extension: Uuid) -> bool {
    node.status(extension).is_some_and(|(s, _)| s == "ready")
}

#[test]
fn an_install_on_a_reaches_b_which_verifies_and_gets_the_data() {
    let (a, b) = (Node::new(), Node::new());
    let ext = a.install(&bundle("1.0.0", &[INIT]));
    a.follow();
    a.write("INSERT INTO `{t}` (id, label) VALUES ('i1', 'first')");

    b.pull(&a);
    let changed = b.follow();
    assert!(ready(&b, ext));
    assert!(changed.iter().any(|c| c.status == "ready" && !c.reload));
    assert_eq!(b.count("SELECT COUNT(*) FROM `{t}`"), Some(1));
    assert_eq!(b.parked(), 0);
}

#[test]
fn a_bundle_corrupted_on_b_fails_there_while_other_data_keeps_syncing() {
    let (a, b) = (Node::new(), Node::new());
    let ext = a.install(&bundle("1.0.0", &[INIT]));
    a.follow();
    b.pull(&a);
    b.write("UPDATE extension_blobs SET data = x'00'");

    b.follow();
    assert_eq!(
        b.status(ext).map(|(s, _)| s).as_deref(),
        Some("signature_failed")
    );
    a.write("INSERT INTO chat_threads (id, title, created_at, updated_at) VALUES ('t', 't', 1, 1)");
    b.pull(&a);
    assert_eq!(b.count("SELECT COUNT(*) FROM chat_threads"), Some(1));
}

#[test]
fn an_update_on_a_applies_the_new_migrations_on_b_and_reloads_its_tabs() {
    let (a, b) = (Node::new(), Node::new());
    let ext = a.install(&bundle("1.0.0", &[INIT]));
    a.follow();
    b.pull(&a);
    b.follow();
    let first = b.status(ext).and_then(|(_, bundle)| bundle);

    a.install(&bundle("1.1.0", &[INIT, TAG]));
    a.follow();
    a.write("INSERT INTO `{t}` (id, label, tag) VALUES ('i1', 'first', 'red')");
    b.pull(&a);
    let changed = b.follow();

    let second = b.status(ext).and_then(|(_, bundle)| bundle);
    assert_ne!(first, second, "B switched to the new bundle");
    assert!(changed.iter().any(|c| c.reload), "open tabs reload");
    assert_eq!(
        b.count("SELECT COUNT(*) FROM `{t}` WHERE tag = 'red'"),
        Some(1)
    );
}

#[test]
fn concurrent_installs_of_two_versions_end_on_the_higher_one_everywhere() {
    let (a, b) = (Node::new(), Node::new());
    let ext = a.install(&bundle("1.2.0", &[INIT]));
    b.install(&bundle("1.3.0", &[INIT]));
    a.pull(&b);
    b.pull(&a);
    a.follow();
    b.follow();
    let on_a = a.status(ext).and_then(|(_, bundle)| bundle);
    let on_b = b.status(ext).and_then(|(_, bundle)| bundle);
    assert!(on_a.is_some());
    assert_eq!(on_a, on_b);
    let version: String = query::read(a.device.db(), |r| {
        r.query_row(
            "SELECT version FROM extension_bundles WHERE id = ?1",
            params![on_a.clone().unwrap()],
            |row| row.get(0),
        )
    })
    .expect("version")
    .expect("bundle");
    assert_eq!(version, "1.3.0");
}

#[test]
fn a_device_scoped_permission_from_a_does_not_hold_on_b() {
    let (a, b) = (Node::new(), Node::new());
    let ext = a.install(&bundle("1.0.0", &[INIT]));
    a.vault
        .write_blocking(move |tx| {
            permission_store::put(
                tx,
                ext,
                &NewPermission {
                    kind: "filesystem",
                    action: "read",
                    target: "/tmp/notes",
                    status: "granted",
                    declared: false,
                    vault_device_uuid: a.me,
                },
                1,
            )
            .map(drop)
            .map_err(Into::into)
        })
        .expect("grant");
    b.pull(&a);
    let on = |node: &Node| {
        let me = node.me;
        node.vault
            .read_blocking(move |q| {
                permission_store::candidates(q, ext, PermissionKind::Filesystem, me)
                    .map_err(Into::into)
            })
            .expect("candidates")
            .len()
    };
    assert_eq!(on(&a), 1);
    assert_eq!(on(&b), 0);
}

#[test]
fn delete_data_drops_the_tables_on_b_and_late_groups_from_before_are_discarded() {
    let (a, b, c) = (Node::new(), Node::new(), Node::new());
    let ext = a.install(&bundle("1.0.0", &[INIT]));
    a.follow();
    a.write("INSERT INTO `{t}` (id, label) VALUES ('i1', 'first')");
    b.pull(&a);
    b.follow();
    c.pull(&a);
    c.follow();
    // Written on C before the removal, reaching B only after it.
    c.write("INSERT INTO `{t}` (id, label) VALUES ('late', 'from c')");

    remove(&a.vault, ext, true, now()).expect("remove");
    a.follow();
    assert!(!a.has_items(), "the removing device clears up too");
    b.pull(&a);
    b.follow();
    assert!(!b.has_items());
    assert_eq!(b.journal(), 0);

    b.pull(&c);
    assert!(!b.has_items());
    assert_eq!(b.parked(), 0, "older than the purge: dropped, not parked");
}

#[test]
fn a_reinstall_after_removal_starts_fresh_with_newer_data() {
    let (a, b) = (Node::new(), Node::new());
    let v1 = bundle("1.0.0", &[INIT]);
    let ext = a.install(&v1);
    a.follow();
    a.write("INSERT INTO `{t}` (id, label) VALUES ('old', 'before')");
    b.pull(&a);
    b.follow();
    remove(&a.vault, ext, true, now()).expect("remove");
    a.follow();
    b.pull(&a);
    b.follow();

    a.install(&v1);
    a.follow();
    a.write("INSERT INTO `{t}` (id, label) VALUES ('new', 'after')");
    b.pull(&a);
    b.follow();
    assert!(ready(&b, ext));
    assert_eq!(
        b.count("SELECT COUNT(*) FROM `{t}` WHERE id = 'new'"),
        Some(1)
    );
    assert_eq!(
        b.count("SELECT COUNT(*) FROM `{t}` WHERE id = 'old'"),
        Some(0)
    );
}

#[test]
fn a_device_offline_during_removal_and_reinstall_still_clears_up() {
    let (a, c) = (Node::new(), Node::new());
    let v1 = bundle("1.0.0", &[INIT]);
    let ext = a.install(&v1);
    a.follow();
    a.write("INSERT INTO `{t}` (id, label) VALUES ('old', 'before')");
    c.pull(&a);
    c.follow();

    // C is offline while A removes with "delete data" and installs again.
    remove(&a.vault, ext, true, now()).expect("remove");
    a.follow();
    a.install(&v1);
    a.follow();

    c.pull(&a);
    c.follow();
    assert!(ready(&c, ext), "C sees state = installed only");
    assert_eq!(
        c.count("SELECT COUNT(*) FROM `{t}` WHERE id = 'old'"),
        Some(0)
    );
}

#[test]
fn keep_data_keeps_tables_journal_and_parked_groups_and_a_reinstall_does_not_migrate_again() {
    let (a, b) = (Node::new(), Node::new());
    let v1 = bundle("1.0.0", &[INIT]);
    let ext = a.install(&v1);
    a.follow();
    a.write("INSERT INTO `{t}` (id, label) VALUES ('i1', 'kept')");
    b.pull(&a);
    b.follow();
    // A group for a table of the extension B does not have, waiting.
    let waiting = serde_json::json!([{
        "tableName": format!("{}__tasks__later", public_key()),
        "rowPks": "{\"id\":\"x\"}",
        "columnName": "label",
        "hlcTimestamp": "h",
        "value": "v",
        "deviceId": "d",
    }])
    .to_string();
    b.device
        .db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO sync_parked_groups_no_sync (origin, hlc, extension_prefix, tables, \
                 group_blob, bytes, reason, parked_at) VALUES ('o', 'h', ?1, '[]', ?2, 2, \
                 'missing_table', 1)",
                params![format!("{}__tasks__", public_key()), waiting.into_bytes()],
            )
            .map(drop)
        })
        .expect("park");

    remove(&a.vault, ext, false, now()).expect("remove");
    b.pull(&a);
    b.follow();
    assert!(b.has_items());
    assert_eq!(b.count("SELECT COUNT(*) FROM `{t}`"), Some(1));
    assert_eq!(b.journal(), 1);
    assert_eq!(b.parked(), 1);

    a.install(&v1);
    a.follow();
    b.pull(&a);
    b.follow();
    assert!(ready(&b, ext));
    assert_eq!(b.journal(), 1, "no migration ran again");
    assert_eq!(b.count("SELECT COUNT(*) FROM `{t}`"), Some(1));
}
