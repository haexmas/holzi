use haex_crdt::rusqlite::params;
use uuid::Uuid;

use super::*;
use crate::storage::query::Query;
use crate::sync::device_list::{DeviceList, RemovedDevice};
use crate::sync::outbound::serve_pull_with_budget;
use crate::sync::test_support::{open_vault_with_limit, Device};

fn write_thread(device: &Device, id: &str, title: &str) {
    device
        .db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO chat_threads (id, title, created_at, updated_at) VALUES (?1, ?2, 1, 1) \
                 ON CONFLICT(id) DO UPDATE SET title = excluded.title",
                params![id, title],
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

fn title(device: &Device, id: &str) -> Option<String> {
    query::read(device.db(), |r| {
        r.query_row(
            "SELECT title FROM chat_threads WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
    })
    .expect("read title")
}

fn progress_of(device: &Device, origin: &Device) -> Option<String> {
    device
        .replica
        .progress()
        .expect("progress")
        .get(&origin.db().device_id())
        .cloned()
}

#[test]
fn created_changed_and_deleted_rows_arrive() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "t1", "first");
    write_thread(&a, "t2", "second");

    let received = b.pull_from(&a);
    assert_eq!(title(&b, "t1").as_deref(), Some("first"));
    assert!(received.last().expect("pages").done);
    assert!(received[0].tables.contains("chat_threads"));

    write_thread(&a, "t1", "renamed");
    delete_thread(&a, "t2");
    let received = b.pull_from(&a);

    assert_eq!(title(&b, "t1").as_deref(), Some("renamed"));
    assert_eq!(title(&b, "t2"), None);
    assert!(
        received[0].tables.contains("chat_threads"),
        "a delete names the table it deletes from"
    );
}

#[test]
fn after_a_pull_neither_side_sees_more_even_after_device_local_writes() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "t1", "first");
    b.pull_from(&a);
    a.pull_from(&b);

    a.db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO holzi_maintenance_no_sync (task) VALUES ('x')",
                &[],
            )?;
            Ok(())
        })
        .expect("device-local write");

    let (pa, pb) = (
        a.replica.progress().expect("a"),
        b.replica.progress().expect("b"),
    );
    assert!(!progress::has_more(&pa, &pb), "b has everything of a");
    assert!(!progress::has_more(&pb, &pa), "a has everything of b");
}

#[test]
fn the_newer_write_to_the_same_field_wins_on_both_devices() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "t1", "base");
    b.pull_from(&a);

    write_thread(&a, "t1", "from a");
    write_thread(&b, "t1", "from b, later");
    b.pull_from(&a);
    a.pull_from(&b);

    assert_eq!(title(&a, "t1").as_deref(), Some("from b, later"));
    assert_eq!(title(&b, "t1").as_deref(), Some("from b, later"));
}

#[test]
fn device_local_rows_never_travel() {
    let (a, b) = (Device::new(), Device::new());
    a.db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO holzi_maintenance_no_sync (task) VALUES ('only-a')",
                &[],
            )?;
            Ok(())
        })
        .expect("device-local write");
    write_thread(&a, "t1", "shared");

    b.pull_from(&a);

    let local: i64 = query::read(b.db(), |r| {
        r.query_row(
            "SELECT COUNT(*) FROM holzi_maintenance_no_sync WHERE task = 'only-a'",
            &[],
            |row| row.get(0),
        )
        .map(|n| n.unwrap_or(0))
    })
    .expect("count");
    assert_eq!(local, 0);
}

#[test]
fn changes_reach_a_third_device_through_an_intermediate_one() {
    let (a, b, c) = (Device::new(), Device::new(), Device::new());
    write_thread(&a, "t1", "from a");
    b.pull_from(&a);
    c.pull_from(&b);
    assert_eq!(title(&c, "t1").as_deref(), Some("from a"));
    assert_eq!(progress_of(&c, &a), progress_of(&b, &a));

    write_thread(&a, "t2", "later from a");
    let received = c.pull_from(&a);
    assert_eq!(title(&c, "t2").as_deref(), Some("later from a"));
    let delivered: usize = received.iter().map(|r| r.tables.len()).sum();
    assert_eq!(delivered, 1, "only the new change travels");
}

#[test]
fn an_open_group_waits_for_its_last_page() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "t1", "one group of several cells");
    let theirs = b.replica.progress().expect("progress");
    let mut outbox = serve_pull_with_budget(&a.replica, &theirs, 120).expect("serve");
    let mut inbox = Inbox::new();

    let first = outbox.next_page().expect("first page");
    assert!(first.group_continues);
    let received = inbox.receive(&b.replica, first).expect("first");
    assert!(received.tables.is_empty());
    assert_eq!(title(&b, "t1"), None);
    assert_eq!(progress_of(&b, &a), None, "no progress for an open group");

    while let Some(page) = outbox.next_page() {
        inbox.receive(&b.replica, page).expect("page");
    }
    assert_eq!(
        title(&b, "t1").as_deref(),
        Some("one group of several cells")
    );
    assert!(progress_of(&b, &a).is_some());
}

#[test]
fn a_broken_pull_resumes_after_the_last_complete_group() {
    let (a, b) = (Device::new(), Device::new());
    for i in 0..4 {
        write_thread(&a, &format!("t{i}"), "x");
    }
    let theirs = b.replica.progress().expect("progress");
    let whole = serve_pull_with_budget(&a.replica, &theirs, usize::MAX)
        .expect("serve")
        .next_page()
        .expect("page");
    let first_hlc = &whole.changes[0].hlc;
    let group: usize = whole
        .changes
        .iter()
        .filter(|c| &c.hlc == first_hlc)
        .map(Change::wire_size)
        .sum();
    // One group per page.
    let mut outbox = serve_pull_with_budget(&a.replica, &theirs, group + group / 2).expect("serve");
    let mut inbox = Inbox::new();
    let first = outbox.next_page().expect("page");
    assert!(first.more && !first.group_continues);
    inbox.receive(&b.replica, first).expect("first page");
    let partial = progress_of(&b, &a).expect("progress after the first page");
    drop(outbox);

    b.pull_from(&a);

    for i in 0..4 {
        assert_eq!(title(&b, &format!("t{i}")).as_deref(), Some("x"));
    }
    assert!(progress::is_beyond(
        &progress_of(&b, &a).expect("progress"),
        Some(&partial)
    ));
}

#[test]
fn a_pull_cannot_apply_an_older_group_after_a_newer_page() {
    let b = Device::new();
    write_thread(&b, "x", "base");
    let (origin, newer) = foreign_change("chat_threads");
    let newer_time = newer
        .hlc
        .split_once('/')
        .and_then(|(time, _)| time.parse::<u64>().ok())
        .expect("timestamp");
    let node = u128::from_le_bytes(*origin.as_bytes());
    let mut older = newer.clone();
    older.hlc = format!("{}/{node:x}", newer_time - 1);

    let mut inbox = Inbox::new();
    inbox
        .receive(
            &b.replica,
            Page {
                changes: vec![newer],
                group_continues: false,
                more: true,
                served: Vector::new(),
            },
        )
        .expect("newer page");
    assert!(matches!(
        inbox.receive(&b.replica, page_with(older)),
        Err(InboundError::Malformed(
            "groups out of HLC order across pages"
        ))
    ));
}

fn page_with(change: Change) -> Page {
    Page {
        changes: vec![change],
        group_continues: false,
        more: false,
        served: Vector::new(),
    }
}

fn foreign_change(table: &str) -> (Uuid, Change) {
    let origin = Uuid::new_v4();
    let node = u128::from_le_bytes(*origin.as_bytes());
    let now = uhlc_now();
    (
        origin,
        Change {
            table: table.to_string(),
            row_pks: r#"{"id":"x"}"#.to_string(),
            column: "title".to_string(),
            hlc: format!("{now}/{node:x}"),
            value: r#""v""#.to_string(),
            continues: false,
        },
    )
}

/// An NTP64 timestamp for now, as haex-crdt's HLCs carry it.
fn uhlc_now() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time");
    haex_crdt::uhlc::NTP64::from(now).as_u64()
}

#[test]
fn a_table_this_device_does_not_sync_aborts_without_progress() {
    let b = Device::new();
    for table in ["no_such_table", "holzi_maintenance_no_sync"] {
        let (origin, change) = foreign_change(table);
        let result = Inbox::new().receive(&b.replica, page_with(change));
        assert!(matches!(result, Err(InboundError::UnknownTable(t)) if t == table));
        assert!(!b
            .replica
            .progress()
            .expect("progress")
            .contains_key(&origin));
    }
}

#[test]
fn a_group_over_the_transaction_limit_aborts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let small = Replica::new(std::sync::Arc::new(open_vault_with_limit(dir.path(), 2)));
    let (origin, mut change) = foreign_change("chat_threads");
    change.value = r#""four""#.to_string();
    let result = Inbox::new().receive(&small, page_with(change));
    assert!(matches!(
        result,
        Err(InboundError::GroupTooLarge { bytes: 4, limit: 2 })
    ));
    assert!(!small.progress().expect("progress").contains_key(&origin));
}

#[test]
fn a_removed_device_is_rejected_beyond_its_limit_and_progress_moves_on() {
    let (a, b) = (Device::new(), Device::new());
    write_thread(&a, "before", "kept");
    let limit = query::read(a.db(), |r| {
        r.query_row(
            "SELECT haex_hlc_no_sync FROM chat_threads WHERE id = 'before'",
            &[],
            |row| row.get::<_, String>(0),
        )
        .map(|hlc| hlc.expect("row"))
    })
    .expect("limit");
    write_thread(&a, "after", "rejected");
    remove_on(&b, a.db().device_id(), &limit);

    let received = b.pull_from(&a);

    assert_eq!(title(&b, "before").as_deref(), Some("kept"));
    assert_eq!(title(&b, "after"), None);
    assert_eq!(received.iter().map(|r| r.rejected_groups).sum::<usize>(), 1);
    assert!(!progress::has_more(
        &a.replica.progress().expect("a"),
        &b.replica.progress().expect("b")
    ));
}

/// Gives `device` a vault identity and a second device list that removes
/// `origin` with `limit`.
fn remove_on(device: &Device, origin: Uuid, limit: &str) {
    crate::sync::genesis::ensure_sync_state(device.db(), Uuid::new_v4(), true).expect("genesis");
    device
        .db()
        .write(|tx| {
            let vault = keys::vault_pubkey(tx)?.expect("identity");
            let secret = keys::vault_secret(tx)?.expect("secret");
            let first = device_list::valid_lists(&device_list::load_all(tx)?, &vault)
                .into_values()
                .next()
                .expect("first list");
            let list = DeviceList {
                generation: 2,
                removed: vec![RemovedDevice {
                    device_pubkey: [7; 32],
                    vault_device_uuid: origin,
                    limit_hlc: limit.to_string(),
                    removed_at: 1,
                }],
                base_list_hash: Some(first.hash),
                ..first.list
            };
            let signed =
                device_list::sign_list(list, &secret).map_err(haex_crdt::Error::consumer)?;
            device_list::insert(tx, &signed)
        })
        .expect("second list");
}

#[test]
fn blob_credentials_arrive_as_blobs() {
    let (a, b) = (Device::new(), Device::new());
    let credentials: Vec<u8> = vec![0, 159, 146, 150, 255];
    a.db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO providers (id, kind, name, credentials, created_at) \
                 VALUES ('p1', 'api_key', 'Key', ?1, 1)",
                params![credentials],
            )?;
            Ok(())
        })
        .expect("provider");

    b.pull_from(&a);

    let (kind, bytes): (String, Vec<u8>) = query::read(b.db(), |r| {
        r.query_row(
            "SELECT typeof(credentials), credentials FROM providers WHERE id = 'p1'",
            &[],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map(|row| row.expect("row"))
    })
    .expect("read provider");
    assert_eq!(kind, "blob");
    assert_eq!(bytes, credentials);
}

#[test]
fn the_device_list_and_key_envelopes_travel_like_any_table() {
    let (a, b) = (Device::new(), Device::new());
    crate::sync::genesis::ensure_sync_state(a.db(), Uuid::new_v4(), true).expect("genesis");

    b.pull_from(&a);

    for table in [
        "vault_identity",
        "device_lists",
        "vault_key_generations",
        "vault_key_envelopes",
    ] {
        let count = |device: &Device| -> i64 {
            query::read(device.db(), |r| {
                r.query_row(&format!("SELECT COUNT(*) FROM {table}"), &[], |row| {
                    row.get(0)
                })
                .map(|n| n.unwrap_or(0))
            })
            .expect("count")
        };
        assert_eq!(count(&b), count(&a), "{table}");
        assert!(count(&b) > 0, "{table}");
    }
    let secrets: i64 = query::read(b.db(), |r| {
        r.query_row(
            "SELECT COUNT(*) FROM vault_identity_secret_no_sync",
            &[],
            |row| row.get(0),
        )
        .map(|n| n.unwrap_or(0))
    })
    .expect("count secrets");
    assert_eq!(secrets, 0, "the vault secret never travels");
}
