use std::time::Duration;

use haex_crdt::rusqlite::params;
use tokio::sync::Notify;

use super::*;
use crate::storage::query::{self, Query};
use crate::sync::test_support::Member;

/// How long a test waits for an expected event.
const EVENT_LIMIT: Duration = Duration::from_secs(20);

struct Node {
    node: SyncNode,
    applied: Arc<Notify>,
}

async fn start(member: &Member) -> Node {
    let applied = Arc::new(Notify::new());
    let signal = Arc::clone(&applied);
    let node = SyncNode::bind(
        Arc::clone(&member.device.replica),
        member.keys.clone(),
        member.vault,
        NodeConfig {
            relay_mode: RelayMode::Disabled,
            bind_addr: Some((std::net::Ipv4Addr::LOCALHOST, 0).into()),
        },
        Arc::new(move |_tables| signal.notify_one()),
    )
    .await
    .expect("bind");
    Node { node, applied }
}

fn write_thread(member: &Member, id: &str, title: &str) {
    member
        .device
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

fn title(member: &Member, id: &str) -> Option<String> {
    query::read(member.device.db(), |r| {
        r.query_row(
            "SELECT title FROM chat_threads WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
    })
    .expect("read title")
}

/// Waits for `applied` until `done` holds, at most [`EVENT_LIMIT`].
async fn wait_for(applied: &Notify, done: impl Fn() -> bool) {
    tokio::time::timeout(EVENT_LIMIT, async {
        while !done() {
            applied.notified().await;
        }
    })
    .await
    .expect("the change arrives in time");
}

fn pair() -> (Member, Member) {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    (main, linked)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_devices_exchange_changes_in_both_directions() {
    let (main, linked) = pair();
    let a = start(&main).await;
    let b = start(&linked).await;
    b.node.connect(a.node.addr()).await.expect("connect");

    write_thread(&main, "t1", "from main");
    a.node.local_changed();
    wait_for(&b.applied, || {
        title(&linked, "t1").as_deref() == Some("from main")
    })
    .await;

    write_thread(&linked, "t2", "from linked");
    b.node.local_changed();
    wait_for(&a.applied, || {
        title(&main, "t2").as_deref() == Some("from linked")
    })
    .await;

    tokio::time::timeout(Duration::from_secs(5), async {
        a.node.shutdown().await;
        b.node.shutdown().await;
    })
    .await
    .expect("the nodes shut down within the session-end limits");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changes_made_before_connecting_arrive_on_connect() {
    let (main, linked) = pair();
    write_thread(&main, "offline", "written while apart");
    let a = start(&main).await;
    let b = start(&linked).await;

    b.node.connect(a.node.addr()).await.expect("connect");

    wait_for(&b.applied, || title(&linked, "offline").is_some()).await;
    a.node.shutdown().await;
    b.node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_devices_dialing_each_other_keep_one_connection() {
    let (main, linked) = pair();
    let a = start(&main).await;
    let b = start(&linked).await;

    let (ab, ba) = tokio::join!(a.node.connect(b.node.addr()), b.node.connect(a.node.addr()));
    ab.expect("a dials");
    ba.expect("b dials");
    write_thread(&main, "t1", "x");
    a.node.local_changed();
    wait_for(&b.applied, || title(&linked, "t1").is_some()).await;
    write_thread(&linked, "t2", "y");
    b.node.local_changed();
    wait_for(&a.applied, || title(&main, "t2").is_some()).await;

    assert_eq!(a.node.connected(), vec![linked.keys.device_pubkey]);
    assert_eq!(b.node.connected(), vec![main.keys.device_pubkey]);
    a.node.shutdown().await;
    b.node.shutdown().await;
}
