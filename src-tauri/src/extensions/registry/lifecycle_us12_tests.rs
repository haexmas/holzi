//! A signed install that reaches a device with a development version of the same prefix (spec
//! 017, US12, T093, research R16).

// The tests read rows directly to observe each device.
#![allow(clippy::disallowed_methods)]

use super::test_support::*;
use super::*;
use crate::extensions::dev;

/// Registers a development version of the test extension on `node`.
fn develop(node: &Node) -> (tempfile::TempDir, Uuid) {
    let me = node.me;
    node.vault
        .write_blocking(move |tx| dev::set_mode(tx, me, true).map_err(Into::into))
        .expect("mode");
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir(dir.path().join("haextension")).expect("folder");
    std::fs::write(
        dir.path().join("haextension/manifest.json"),
        serde_json::json!({ "name": "tasks", "version": "0.0.1", "publicKey": public_key() })
            .to_string(),
    )
    .expect("manifest");
    let id = dev::confirm(&node.vault, dir.path(), vec![], node.me, now()).expect("confirm");
    (dir, id)
}

#[test]
fn a_synced_install_waits_while_a_development_version_holds_its_prefix() {
    let (a, b) = (Node::new(), Node::new());
    let (_project, draft) = develop(&b);

    let ext = a.install(&bundle("1.0.0", &[INIT]));
    a.follow();
    a.write("INSERT INTO `{t}` (id, label) VALUES ('i1', 'signed')");
    b.pull(&a);
    b.follow();
    assert_eq!(
        b.status(ext).map(|(s, _)| s).as_deref(),
        Some("migration_failed")
    );
    let me = b.me;
    let error = b.count(&format!(
        "SELECT COUNT(*) FROM extension_device_status WHERE extension_id = '{ext}' \
         AND vault_device_uuid = '{me}' AND error = 'dev_prefix_conflict'"
    ));
    assert_eq!(error, Some(1));
    assert!(b.parked() > 0, "its rows wait parked");
    assert!(
        !b.has_items(),
        "nothing reached a table of the development version"
    );

    dev::unload(&b.vault, draft).expect("unload");
    b.follow();
    assert!(ready(&b, ext));
    assert_eq!(b.count("SELECT COUNT(*) FROM `{t}`"), Some(1));
    assert_eq!(b.parked(), 0);
}
