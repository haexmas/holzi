//! Migrations across devices: a confirmed downgrade keeps what was applied, only the verified SQL
//! of the effective bundle runs, and a bundle must belong to the row that chose it (spec 017, R11,
//! FR-003).

// The tests change and read registry rows directly to put a device into each state.
#![allow(clippy::disallowed_methods)]

use super::test_support::*;
use super::*;
use crate::extensions::registry::install::install;

fn confirm_downgrade(node: &Node, bytes: &[u8]) {
    install(&node.vault, bytes, vec![], true, node.me, now()).expect("downgrade");
}

fn shown_status(node: &Node, extension: Uuid) -> Option<String> {
    node.status(extension).map(|(status, _)| status)
}

#[test]
fn a_confirmed_downgrade_keeps_running_on_every_device() {
    let (a, b) = (Node::new(), Node::new());
    let extension = a.install(&bundle("1.0.0", &[INIT]));
    a.install(&bundle("1.1.0", &[INIT, TAG]));
    a.follow();
    assert_eq!(a.journal(), 2);

    confirm_downgrade(&a, &bundle("1.0.0", &[INIT]));
    a.follow();
    assert!(ready(&a, extension), "A: {:?}", a.status(extension));
    assert_eq!(a.journal(), 2, "nothing is taken back on a downgrade");

    b.pull(&a);
    b.follow();
    b.follow();
    assert!(ready(&b, extension), "B: {:?}", b.status(extension));
    assert_eq!(b.journal(), 1, "B runs only the migrations of 1.0.0");
}

#[test]
fn only_the_verified_sql_of_a_migration_runs() {
    let (a, b) = (Node::new(), Node::new());
    let extension = a.install(&bundle("1.0.0", &[INIT]));
    a.install(&bundle("1.1.0", &[INIT, TAG]));
    b.pull(&a);
    // A changed row: other SQL under the name and hash of the second migration.
    b.write(
        "UPDATE extension_migrations SET sql = 'CREATE TABLE `{t}_other` (id TEXT PRIMARY KEY)' \
         WHERE name = '0001_step'",
    );

    b.follow();
    assert!(ready(&b, extension), "B: {:?}", b.status(extension));
    assert_eq!(
        b.count("SELECT COUNT(*) FROM sqlite_master WHERE name = '{t}_other'"),
        Some(0),
        "the changed SQL did not run"
    );
    assert_eq!(
        b.count("SELECT COUNT(*) FROM sqlite_master WHERE name = '{t}' AND sql LIKE '%`tag`%'"),
        Some(1),
        "the signed migration ran instead"
    );
}

#[test]
fn a_bundle_its_row_names_with_another_version_does_not_start() {
    let a = Node::new();
    let extension = a.install(&bundle("1.0.0", &[INIT]));
    a.write("UPDATE extension_bundles SET version = '9.0.0'");

    a.follow();
    assert_eq!(
        shown_status(&a, extension).as_deref(),
        Some("signature_failed")
    );
    assert!(!a.has_items(), "nothing of the bundle ran");
}
