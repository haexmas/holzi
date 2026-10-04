//! Parking groups for extension tables (spec 017, T075, research R10, R11).

use std::sync::Arc;

use haex_crdt::rusqlite::params;
use haex_crdt::{AuthContext, Authorization, GuardedWriteOptions, SqlGuard};

use super::*;
use crate::extensions::registry::status::{self, DeviceStatus, PARKED_LIMIT};
use crate::extensions::sql::test_support::{own, t};
use crate::storage::query;
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

/// Records on `device` that it cleared up for the removal of `purge_own`, as the clear-up does.
fn cleared_up(device: &Device) {
    write(
        device,
        "INSERT INTO extension_purges_applied_no_sync (extension_id, purge_hlc) \
         SELECT id, purge_hlc FROM extensions WHERE id = 'ext'",
    );
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

    // Newer than the removal, before B cleared up for it: the clear-up must not drop it.
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('new', 'after')");
    b.pull_from(&a);
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 0);
    assert_eq!(
        parked(&b),
        vec![(own().to_string(), AWAITING_PURGE.to_owned())]
    );

    cleared_up(&b);
    let replayed = replay_ready(b.db()).expect("replay");
    assert_eq!(replayed.groups, 1);
    assert_eq!(
        count(&b, "SELECT COUNT(*) FROM t:pages WHERE id = 'new'"),
        1
    );

    write(
        &a,
        "INSERT INTO t:pages (id, body) VALUES ('later', 'after')",
    );
    b.pull_from(&a);
    assert_eq!(
        count(&b, "SELECT COUNT(*) FROM t:pages WHERE id = 'later'"),
        1,
        "after the clear-up newer changes apply directly"
    );
}

#[test]
fn a_removal_earlier_in_the_same_pull_parks_the_newer_rows_of_its_extension() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    ddl(&b, PAGES);
    let prefix = own();
    write(
        &a,
        &format!(
            "INSERT INTO extensions (id, public_key, name, installed_at, updated_at) \
             VALUES ('ext', '{}', '{}', 1, 1)",
            prefix.public_key.as_str(),
            prefix.name.as_str()
        ),
    );
    b.pull_from(&a);

    // One pull carries the removal and rows written after it (a reinstall elsewhere).
    write(
        &a,
        "UPDATE extensions SET state = 'removed', purge_data = 1, \
         purge_hlc = haex_hlc_no_sync WHERE id = 'ext'",
    );
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('new', 'after')");
    b.pull_from(&a);
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 0);
    assert_eq!(
        parked(&b),
        vec![(own().to_string(), AWAITING_PURGE.to_owned())]
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

/// The state of extension `id` on `device`: status and error.
fn state_of(device: &Device, id: &str) -> Option<(String, Option<String>)> {
    let row = crate::extensions::ids::device_status_id(
        uuid::Uuid::parse_str(id).unwrap(),
        device.db().device_id(),
    )
    .to_string();
    query::read(device.db(), |r| {
        r.query_row(
            "SELECT status, error FROM extension_device_status WHERE id = ?1",
            params![row],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    })
    .expect("state")
}

#[test]
fn at_the_parking_limit_the_state_here_says_so_until_the_extension_starts() {
    const EXT: &str = "0b1e7c2a-63d4-4f7a-9d4e-6a0c2f5e8a11";
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");
    let prefix = own();
    b.db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO extensions (id, public_key, name, installed_at, updated_at) \
                 VALUES (?1, ?2, ?3, 1, 1)",
                params![EXT, prefix.public_key.as_str(), prefix.name.as_str()],
            )
            .map(drop)
        })
        .expect("registered");

    // No room at all: the group is not parked, and the state here tells why.
    pull_with(&b, &a, Inbox::with_park_limit(0));
    assert!(parked(&b).is_empty());
    assert_eq!(
        state_of(&b, EXT),
        Some(("transferring".to_owned(), Some(PARKED_LIMIT.to_owned())))
    );

    let ext = uuid::Uuid::parse_str(EXT).unwrap();
    let me = b.db().device_id();
    let set = |state: DeviceStatus| {
        b.db()
            .write(|tx| status::set(tx, ext, me, state, None, None, 2).map_err(Into::into))
            .expect("set")
    };
    set(DeviceStatus::Transferring);
    assert_eq!(
        state_of(&b, EXT).and_then(|(_, error)| error).as_deref(),
        Some(PARKED_LIMIT),
        "still transferring: the reason stays"
    );
    set(DeviceStatus::Ready);
    assert_eq!(state_of(&b, EXT), Some(("ready".to_owned(), None)));
}

#[test]
fn a_registry_arriving_after_the_limit_page_still_gets_the_limit_error() {
    const EXT: &str = "0b1e7c2a-63d4-4f7a-9d4e-6a0c2f5e8a11";
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");

    // Fill the receiver before it knows the extension. The first group's stored size gives a
    // budget that keeps the later registry write on a separate page.
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
    pull_with(&b, &a, Inbox::with_park_limit(0));
    assert!(state_of(&b, EXT).is_none());

    let prefix = own();
    a.db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO extensions (id, public_key, name, installed_at, updated_at) \
                 VALUES (?1, ?2, ?3, 1, 1)",
                params![EXT, prefix.public_key.as_str(), prefix.name.as_str()],
            )
            .map(drop)
        })
        .expect("registered");

    let theirs = b.replica.progress().expect("progress");
    let mut outbox = serve_pull_with_budget(
        &a.replica,
        &theirs,
        usize::try_from(first).unwrap() + usize::try_from(first).unwrap() / 2,
    )
    .expect("serve");
    let mut inbox = Inbox::with_park_limit(0);
    let first_page = outbox.next_page().expect("data page");
    assert!(first_page.more, "the registry must be on a later page");
    inbox.receive(&b.replica, first_page).expect("data page");
    assert!(state_of(&b, EXT).is_none());
    while let Some(page) = outbox.next_page() {
        inbox.receive(&b.replica, page).expect("registry page");
    }

    assert_eq!(
        state_of(&b, EXT),
        Some(("transferring".to_owned(), Some(PARKED_LIMIT.to_owned())))
    );
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
