//! Edge cases of parking (spec 017, research R10, R11): a group fetched again, a "keep data"
//! removal after a "delete data" one, and a replay that has to stop.

use super::super::*;
use super::{count, ddl, parked, pull_with, purge_own, write, PAGES};
use crate::storage::query::{self, Query};
use crate::sync::inbound::Inbox;
use crate::sync::test_support::Device;

/// Bytes and HLC of the one parked group.
fn the_parked(device: &Device) -> (i64, String) {
    query::read(device.db(), |r| {
        r.query_row(
            "SELECT bytes, hlc FROM sync_parked_groups_no_sync",
            &[],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    })
    .expect("parked")
    .expect("one parked")
}

#[test]
fn a_group_fetched_again_counts_once_toward_the_limit() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");
    pull_with(&b, &a, Inbox::new());
    let (bytes, hlc) = the_parked(&b);
    let bytes = usize::try_from(bytes).unwrap();

    // Progress stayed below it (as behind a group held at another extension's limit): the group
    // comes again. Room for it once, not twice.
    write(&b, "DELETE FROM sync_progress_no_sync");
    pull_with(&b, &a, Inbox::with_park_limit(bytes + bytes / 2));
    assert_eq!(parked(&b).len(), 1, "stored once");
    let progress = b.replica.progress().expect("progress");
    let reached = progress.get(&a.db().device_id()).expect("progress of A");
    assert_ne!(
        haex_crdt::compare_hlc_strings(reached, &hlc),
        std::cmp::Ordering::Less,
        "not taken for full: progress moved past it"
    );
}

#[test]
fn a_keep_data_removal_after_a_cleared_delete_data_one_still_drops_older_changes() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    ddl(&b, PAGES);
    write(
        &a,
        "INSERT INTO t:pages (id, body) VALUES ('old', 'before')",
    );
    purge_own(&b);
    // B cleared up for that "delete data" removal, then the extension was reinstalled and removed
    // again with "keep data": the registry row names only the later removal now.
    write(
        &b,
        "INSERT INTO extension_purges_applied_no_sync (extension_id, purge_hlc, data_purge_hlc) \
         SELECT id, purge_hlc, purge_hlc FROM extensions WHERE id = 'ext'",
    );
    write(
        &b,
        "UPDATE extensions SET purge_data = 0, purge_hlc = haex_hlc_no_sync WHERE id = 'ext'",
    );
    write(
        &b,
        "UPDATE extension_purges_applied_no_sync SET purge_hlc = \
         (SELECT purge_hlc FROM extensions WHERE id = 'ext')",
    );

    b.pull_from(&a);
    assert_eq!(
        count(&b, "SELECT COUNT(*) FROM t:pages"),
        0,
        "older than the delete-data removal: still dropped"
    );
}

#[test]
fn a_replay_told_to_stop_leaves_the_groups_parked() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");
    b.pull_from(&a);
    ddl(&b, PAGES);

    let replayed = replay_ready(b.db(), &|| true).expect("replay");
    assert_eq!(replayed.groups, 0);
    assert_eq!(parked(&b).len(), 1);
    replay_ready(b.db(), &|| false).expect("replay");
    assert!(parked(&b).is_empty());
}
