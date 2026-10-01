use haex_crdt::rusqlite::params;
use haex_crdt::RetentionPolicy;
use uuid::Uuid;

use super::*;
use crate::storage::query::Query;
use crate::sync::change::PAGE_BUDGET;
use crate::sync::inbound::Inbox;
use crate::sync::outbound::{serve, serve_pull_with_budget, Served};
use crate::sync::test_support::Device;

fn write_thread(device: &Device, id: &str) {
    device
        .db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO chat_threads (id, title, created_at, updated_at) VALUES (?1, ?1, 1, 1)",
                params![id],
            )?;
            Ok(())
        })
        .expect("write thread");
}

fn delete_thread(device: &Device, id: &str) {
    device
        .db()
        .write(|tx| {
            tx.execute("DELETE FROM chat_threads WHERE id = ?1", params![id])?;
            Ok(())
        })
        .expect("delete thread");
}

fn threads(device: &Device) -> Vec<String> {
    query::read(device.db(), |r| {
        r.query_map("SELECT id FROM chat_threads ORDER BY id", &[], |row| {
            row.get(0)
        })
    })
    .expect("read threads")
}

/// An HLC of `origin`, `days` before now.
fn hlc_days_ago(days: u64, origin: Uuid) -> String {
    let at = SystemTime::now() - Duration::from_secs(days * 24 * 60 * 60);
    let time =
        haex_crdt::uhlc::NTP64::from(at.duration_since(SystemTime::UNIX_EPOCH).expect("time"))
            .as_u64();
    let node = u128::from_le_bytes(*origin.as_bytes());
    format!("{time}/{node:x}")
}

/// What `into` does with a snapshot of `from`: applies every page, then
/// prunes what the snapshot did not carry.
fn replace_from(from: &Device, into: &Device) -> Vec<String> {
    let Served::Pages(mut outbox) = serve(&from.replica, &Vector::new(), true).expect("serve")
    else {
        panic!("a snapshot request is always served");
    };
    let mut inbox = Inbox::for_snapshot();
    while let Some(page) = outbox.next_page() {
        inbox.receive(&into.replica, page).expect("receive page");
    }
    let (kept, served) = inbox.into_snapshot().expect("a finished snapshot");
    prune_absent_and_advance(&into.replica, &kept, &served).expect("prune")
}

fn purge_delete_markers(device: &Device) {
    device
        .db()
        .cleanup_deleted_rows(RetentionPolicy::All, |_, _| Ok(()))
        .expect("purge markers");
}

#[test]
fn a_cursor_before_the_horizon_is_stale_only_when_the_puller_is_behind() {
    let origin = Uuid::new_v4();
    let old = hlc_days_ago(100, origin);
    let newer = hlc_days_ago(1, origin);
    let now = SystemTime::now();

    let theirs = Vector::from([(origin, old.clone())]);
    let behind = Vector::from([(origin, newer)]);
    let level = Vector::from([(origin, old)]);

    assert!(is_stale(&theirs, &behind, now), "old and behind");
    assert!(
        !is_stale(&theirs, &level, now),
        "an origin quiet for 100 days is no reason: nothing was missed"
    );
}

#[test]
fn a_recent_cursor_is_not_stale_and_an_unknown_origin_is_no_reason() {
    let origin = Uuid::new_v4();
    let now = SystemTime::now();
    let theirs = Vector::from([(origin, hlc_days_ago(10, origin))]);
    let served = Vector::from([(origin, hlc_days_ago(1, origin))]);

    assert!(!is_stale(&theirs, &served, now));
    assert!(
        !is_stale(&Vector::new(), &served, now),
        "a puller without the origin holds nothing a lost deletion could keep"
    );
}

#[test]
fn a_sender_answers_a_stale_pull_with_resync_and_a_snapshot_request_with_pages() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "t1");
    let origin = a.db().device_id();
    let stale = Vector::from([(origin, hlc_days_ago(100, origin))]);

    assert!(matches!(
        serve(&a.replica, &stale, false).expect("serve"),
        Served::Resync
    ));
    assert!(matches!(
        serve(&a.replica, &stale, true).expect("serve"),
        Served::Pages(_)
    ));
    assert!(matches!(
        serve(&a.replica, &b.replica.progress().expect("progress"), false).expect("serve"),
        Served::Pages(_)
    ));
}

#[test]
fn a_snapshot_removes_rows_deleted_while_their_markers_were_pruned() {
    let (a, b) = (Device::new(), Device::new());
    for id in ["t1", "t2", "t3"] {
        write_thread(&a, id);
    }
    b.pull_from(&a);
    delete_thread(&a, "t2");
    delete_thread(&a, "t3");
    purge_delete_markers(&a);

    // Without the markers an ordinary pull cannot tell b about the deletions.
    b.pull_from(&a);
    assert_eq!(threads(&b), ["t1", "t2", "t3"], "the deleted rows linger");

    let touched = replace_from(&a, &b);

    assert_eq!(threads(&b), ["t1"]);
    assert_eq!(touched, ["chat_threads"]);
}

#[test]
fn a_snapshot_does_not_advance_progress_until_pruning_finishes() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "t1");
    let origin = a.db().device_id();
    let served = a.replica.progress().expect("sender progress");
    let mut inbox = Inbox::for_snapshot();
    inbox
        .receive(
            &b.replica,
            crate::sync::change::Page {
                changes: Vec::new(),
                group_continues: false,
                more: false,
                served: served.clone(),
            },
        )
        .expect("receive final page");

    assert!(
        progress::has_more(&served, &b.replica.progress().expect("receiver progress")),
        "the final snapshot page must not publish served progress before pruning"
    );
    let (kept, served) = inbox.into_snapshot().expect("a finished snapshot");
    prune_absent_and_advance(&b.replica, &kept, &served).expect("prune");
    assert!(!progress::has_more(
        &a.replica.progress().expect("sender progress"),
        &b.replica.progress().expect("receiver progress")
    ));
    assert!(b
        .replica
        .progress()
        .expect("receiver progress")
        .contains_key(&origin));
}

#[test]
fn prune_rechecks_a_candidate_after_a_local_write() {
    let device = Device::new();
    write_thread(&device, "t1");
    let served = device.replica.progress().expect("progress after insert");

    // The row was covered when a snapshot scan would have selected it. A local
    // update after that scan gets a newer cell HLC and must make the candidate
    // ineligible when the delete transaction rechecks it.
    device
        .db()
        .write(|tx| {
            tx.execute(
                "UPDATE chat_threads SET title = 'newer' WHERE id = 't1'",
                &[],
            )?;
            Ok(())
        })
        .expect("update thread");

    device
        .db()
        .write(|tx| {
            assert!(!row_is_still_covered(
                tx,
                "chat_threads",
                r#"{"id":"t1"}"#,
                &served,
            )?);
            Ok(())
        })
        .expect("recheck candidate");
}

#[test]
fn a_snapshot_keeps_what_the_sender_had_not_seen() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "t1");
    write_thread(&a, "t2");
    b.pull_from(&a);
    delete_thread(&a, "t2");
    purge_delete_markers(&a);
    // b wrote after it was last in contact; a has never seen this.
    write_thread(&b, "own");

    replace_from(&a, &b);

    assert_eq!(threads(&b), ["own", "t1"]);
    // And what b kept reaches a the ordinary way.
    a.pull_from(&b);
    assert_eq!(threads(&a), ["own", "t1"]);
}

#[test]
fn after_a_snapshot_neither_side_sees_more() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "t1");
    write_thread(&a, "t2");
    b.pull_from(&a);
    delete_thread(&a, "t2");
    purge_delete_markers(&a);

    replace_from(&a, &b);

    let (pa, pb) = (
        a.replica.progress().expect("a"),
        b.replica.progress().expect("b"),
    );
    assert!(!progress::has_more(&pa, &pb), "b has everything of a");
}

#[test]
fn a_snapshot_is_a_pull_from_nothing() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "t1");
    b.pull_from(&a);
    // Even a puller that already has everything is served whole.
    let Served::Pages(mut outbox) = serve(&a.replica, &Vector::new(), true).expect("serve") else {
        panic!("pages");
    };
    let from_nothing = serve_pull_with_budget(&a.replica, &Vector::new(), PAGE_BUDGET)
        .expect("serve from nothing");
    let (mut ours, mut theirs) = (Vec::new(), Vec::new());
    let mut other = from_nothing;
    while let Some(page) = outbox.next_page() {
        ours.push(page);
    }
    while let Some(page) = other.next_page() {
        theirs.push(page);
    }
    assert_eq!(ours, theirs);
}

#[test]
fn row_keys_convert_to_sql_values() {
    use serde_json::json;
    assert_eq!(
        sql_value(&json!("abc")).expect("text"),
        Value::Text("abc".into())
    );
    assert_eq!(sql_value(&json!(7)).expect("int"), Value::Integer(7));
    assert_eq!(
        sql_value(&json!({"$blob_hex": "00ff"})).expect("blob"),
        Value::Blob(vec![0, 255])
    );
    assert!(sql_value(&json!(null)).is_err());
    assert!(sql_value(&json!({"other": 1})).is_err());
}
