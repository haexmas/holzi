// The tests write and read BLOB rows directly to set up and observe the clean-up.
#![allow(clippy::disallowed_methods)]

use super::*;
use crate::passwords::test_support::open_test_vault;
use crate::storage::query::{self, Query};

const DAY: i64 = 86_400_000;

fn blob(db: &haex_crdt::Database, hash: &str, orphaned_at: Option<i64>) {
    db.write(|tx| {
        tx.execute(
            "INSERT INTO extension_blobs (hash, data, size, orphaned_at) VALUES (?1, x'00', 1, ?2)",
            params![hash, orphaned_at],
        )
        .map(drop)
    })
    .unwrap();
}

fn orphaned_at(db: &haex_crdt::Database, hash: &str) -> Option<Option<i64>> {
    query::read(db, |r| {
        r.query_row(
            "SELECT orphaned_at FROM extension_blobs WHERE hash = ?1",
            params![hash],
            |row| row.get(0),
        )
    })
    .unwrap()
}

#[test]
fn an_unreferenced_blob_is_marked_then_freed_after_the_grace_period_and_a_used_one_kept() {
    let (_dir, db) = open_test_vault();
    let now = 100 * DAY;
    blob(&db, "fresh", None);
    blob(&db, "expired", Some(now - 7 * DAY));
    blob(&db, "waiting", Some(now - 6 * DAY));
    blob(&db, "used-again", Some(now - 30 * DAY));
    db.write(|tx| {
        tx.execute(
            "INSERT INTO extensions (id, public_key, name, installed_at, updated_at) \
             VALUES ('e', 'k', 'n', 1, 1)",
            &[],
        )?;
        tx.execute(
            "INSERT INTO extension_bundles (id, extension_id, version, manifest_json, \
             signature_json, retired, added_at) VALUES ('b', 'e', '1.0.0', x'', x'', 0, 1)",
            &[],
        )?;
        tx.execute(
            "INSERT INTO extension_bundle_files (id, bundle_id, path, size, sha256) \
             VALUES ('f', 'b', 'index.html', 1, 'used-again')",
            &[],
        )
        .map(drop)
    })
    .unwrap();

    let freed = db.write(|tx| prune_orphans(tx, now)).unwrap();
    assert_eq!(freed, 1);
    assert_eq!(orphaned_at(&db, "expired"), None, "freed after seven days");
    assert_eq!(orphaned_at(&db, "fresh"), Some(Some(now)), "marked now");
    assert_eq!(orphaned_at(&db, "waiting"), Some(Some(now - 6 * DAY)));
    assert_eq!(orphaned_at(&db, "used-again"), Some(None), "a file uses it");
}
