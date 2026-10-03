//! Parking groups for extension tables (spec 017, T075, research R10, R11).

use std::sync::Arc;

use haex_crdt::rusqlite::params;
use haex_crdt::{AuthContext, Authorization, GuardedWriteOptions, SqlGuard};

use super::*;
use crate::extensions::sql::test_support::{own, t};
use crate::sync::change::PAGE_BUDGET;
use crate::sync::inbound::tests::{foreign_change, page_with};
use crate::sync::inbound::Inbox;
use crate::sync::outbound::serve_pull_with_budget;
use crate::sync::test_support::Device;

/// Runs `sql` (with `t:` for the own prefix) as a migration would: guarded, in schema mode.
fn ddl(device: &Device, sql: &str) {
    let guard = SqlGuard {
        authorizer: Arc::new(|_: &AuthContext<'_>| Authorization::Allow),
        progress: None,
        max_value_bytes: None,
    };
    let sql = t(sql);
    device
        .db()
        .write_guarded_with(
            &guard,
            GuardedWriteOptions {
                schema_mode: true,
                local: false,
            },
            |tx| tx.execute(&sql, &[]).map(drop),
        )
        .expect("ddl");
}

fn write(device: &Device, sql: &str) {
    let sql = t(sql);
    device
        .db()
        .write(|tx| tx.execute(&sql, &[]).map(drop))
        .expect("write");
}

fn count(device: &Device, sql: &str) -> i64 {
    let sql = t(sql);
    query::read(device.db(), |r| r.query_row(&sql, &[], |row| row.get(0)))
        .expect("count")
        .unwrap_or(0)
}

fn parked(device: &Device) -> Vec<(String, String)> {
    query::read(device.db(), |r| {
        r.query_map(
            "SELECT extension_prefix, reason FROM sync_parked_groups_no_sync ORDER BY id",
            &[],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    })
    .expect("parked")
}

fn write_thread(device: &Device, id: &str) {
    device
        .db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO chat_threads (id, title, created_at, updated_at) VALUES (?1, 't', 1, 1)",
                params![id],
            )
            .map(drop)
        })
        .expect("thread");
}

fn threads(device: &Device) -> i64 {
    count(device, "SELECT COUNT(*) FROM chat_threads")
}

const PAGES: &str = "CREATE TABLE t:pages (id TEXT PRIMARY KEY, body TEXT)";

#[test]
fn rows_before_their_tables_are_parked_while_core_data_arrives() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");
    write_thread(&a, "core");

    let received = b.pull_from(&a);
    assert_eq!(threads(&b), 1, "core data in the same pull is applied");
    assert_eq!(
        parked(&b),
        vec![(own().to_string(), MISSING_TABLE.to_owned())]
    );
    assert_eq!(received.iter().map(|r| r.parked_groups).sum::<usize>(), 1);
    assert!(b
        .replica
        .progress()
        .expect("progress")
        .contains_key(&a.db().device_id()));

    // Progress moved past the group: pulling again brings nothing new and parks nothing twice.
    b.pull_from(&a);
    assert_eq!(parked(&b).len(), 1);

    ddl(&b, PAGES);
    let replayed = replay_ready(b.db()).expect("replay");
    assert_eq!(replayed.groups, 1);
    assert_eq!(
        count(&b, "SELECT COUNT(*) FROM t:pages WHERE body = 'one'"),
        1
    );
    assert!(parked(&b).is_empty(), "replayed groups are deleted");
}

#[test]
fn a_column_a_later_migration_adds_is_parked_and_nothing_is_skipped() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    ddl(&b, PAGES);
    ddl(&a, "ALTER TABLE t:pages ADD COLUMN tag TEXT");
    write(
        &a,
        "INSERT INTO t:pages (id, body, tag) VALUES ('p1', 'one', 'red')",
    );

    let received = b.pull_from(&a);
    assert_eq!(
        parked(&b),
        vec![(own().to_string(), MISSING_COLUMN.to_owned())]
    );
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 0);
    assert!(received.iter().all(|r| r.skipped_unknown_columns == 0));

    ddl(&b, "ALTER TABLE t:pages ADD COLUMN tag TEXT");
    replay_ready(b.db()).expect("replay");
    assert_eq!(
        count(&b, "SELECT COUNT(*) FROM t:pages WHERE tag = 'red'"),
        1
    );
}

#[test]
fn a_delete_marker_for_a_missing_table_is_parked_and_wins_after_replay() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");
    b.pull_from(&a);
    write(&a, "DELETE FROM t:pages WHERE id = 'p1'");

    b.pull_from(&a);
    let reasons: Vec<String> = parked(&b).into_iter().map(|(_, reason)| reason).collect();
    assert_eq!(reasons, vec![MISSING_TABLE, MISSING_TABLE]);

    ddl(&b, PAGES);
    replay_ready(b.db()).expect("replay");
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 0);
    assert!(parked(&b).is_empty());
}

#[test]
fn later_groups_of_an_extension_queue_behind_a_parked_one() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    ddl(&a, "ALTER TABLE t:pages ADD COLUMN tag TEXT");
    ddl(&b, PAGES);
    write(
        &a,
        "INSERT INTO t:pages (id, body, tag) VALUES ('p1', 'one', 'red')",
    );
    b.pull_from(&a);
    // A later change without the new column must not overtake the parked insert.
    write(&a, "UPDATE t:pages SET body = 'two' WHERE id = 'p1'");
    b.pull_from(&a);
    assert_eq!(
        parked(&b),
        vec![
            (own().to_string(), MISSING_COLUMN.to_owned()),
            (own().to_string(), AFTER_PARKED.to_owned()),
        ]
    );

    ddl(&b, "ALTER TABLE t:pages ADD COLUMN tag TEXT");
    let replayed = replay_ready(b.db()).expect("replay");
    assert_eq!(replayed.groups, 2);
    assert_eq!(
        count(
            &b,
            "SELECT COUNT(*) FROM t:pages WHERE body = 'two' AND tag = 'red'"
        ),
        1
    );
}

#[test]
fn a_pull_replays_what_became_ready_meanwhile() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");
    b.pull_from(&a);
    ddl(&b, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p2', 'two')");

    b.pull_from(&a);
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 2);
    assert!(parked(&b).is_empty());
}

#[test]
fn an_unknown_table_without_extension_prefix_still_aborts() {
    let b = Device::new();
    let (origin, change) = foreign_change("no_prefix_here");
    let result = Inbox::new().receive(&b.replica, page_with(change));
    assert!(matches!(result, Err(InboundError::UnknownTable(_))));
    assert!(!b
        .replica
        .progress()
        .expect("progress")
        .contains_key(&origin));
    assert!(parked(&b).is_empty());
}

#[test]
fn a_device_local_extension_table_on_the_wire_is_a_protocol_error() {
    let b = Device::new();
    let table = t("t:cache_no_sync");
    let (origin, change) = foreign_change(&table);
    let result = Inbox::new().receive(&b.replica, page_with(change));
    assert!(matches!(result, Err(InboundError::DeviceLocalTable(name)) if name == table));
    assert!(!b
        .replica
        .progress()
        .expect("progress")
        .contains_key(&origin));
}

/// Marks the own extension as removed with "delete data" on `device`, at the HLC of that write.
fn purge_own(device: &Device) {
    let prefix = own();
    device
        .db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO extensions (id, public_key, name, installed_at, updated_at) \
                 VALUES ('ext', ?1, ?2, 1, 1)",
                params![prefix.public_key.as_str(), prefix.name.as_str()],
            )?;
            tx.execute(
                "UPDATE extensions SET state = 'removed', purge_data = 1, \
                 purge_hlc = haex_hlc_no_sync WHERE id = 'ext'",
                &[],
            )
            .map(drop)
        })
        .expect("purge");
}

#[test]
fn changes_older_than_a_purge_are_dropped_and_newer_ones_apply() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    ddl(&b, PAGES);
    write(
        &a,
        "INSERT INTO t:pages (id, body) VALUES ('old', 'before')",
    );
    purge_own(&b);

    b.pull_from(&a);
    assert_eq!(
        count(&b, "SELECT COUNT(*) FROM t:pages"),
        0,
        "older than the purge"
    );
    assert!(parked(&b).is_empty(), "dropped, not parked");

    write(&a, "INSERT INTO t:pages (id, body) VALUES ('new', 'after')");
    b.pull_from(&a);
    assert_eq!(
        count(&b, "SELECT COUNT(*) FROM t:pages WHERE id = 'new'"),
        1
    );
}

fn pull_with(into: &Device, from: &Device, mut inbox: Inbox) {
    let theirs = into.replica.progress().expect("progress");
    let mut outbox = serve_pull_with_budget(&from.replica, &theirs, PAGE_BUDGET).expect("serve");
    while let Some(page) = outbox.next_page() {
        inbox.receive(&into.replica, page).expect("receive");
    }
}

#[test]
fn at_the_parking_limit_progress_waits_instead_of_dropping() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p2', 'two')");
    let first = {
        let probe = Device::new();
        pull_with(&probe, &a, Inbox::new());
        query::read(probe.db(), |r| {
            r.query_row(
                "SELECT bytes FROM sync_parked_groups_no_sync ORDER BY id LIMIT 1",
                &[],
                |row| row.get::<_, i64>(0),
            )
        })
        .expect("bytes")
        .expect("one parked")
    };

    // Room for one group only.
    pull_with(
        &b,
        &a,
        Inbox::with_park_limit(usize::try_from(first).unwrap()),
    );
    assert_eq!(parked(&b).len(), 1);
    let stopped = b.replica.progress().expect("progress");

    // The second group comes again; with room it is parked, the first not twice.
    pull_with(&b, &a, Inbox::new());
    assert_eq!(parked(&b).len(), 2);
    assert_ne!(stopped, b.replica.progress().expect("progress"));

    ddl(&b, PAGES);
    replay_ready(b.db()).expect("replay");
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 2);
}

#[test]
fn a_snapshot_counts_parked_rows_as_carried() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");
    let theirs = b.replica.progress().expect("progress");
    let mut outbox = serve_pull_with_budget(&a.replica, &theirs, PAGE_BUDGET).expect("serve");
    let mut inbox = Inbox::for_snapshot();
    while let Some(page) = outbox.next_page() {
        inbox.receive(&b.replica, page).expect("receive");
    }
    let (rows, _) = inbox.into_snapshot().expect("snapshot");
    assert!(rows.iter().any(|(table, _)| *table == t("t:pages")));
}
