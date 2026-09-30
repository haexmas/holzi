use std::collections::BTreeSet;

use haex_crdt::rusqlite::params;
use tokio::io::{duplex, split};

use super::*;
use crate::storage::query::{self, Query};
use crate::sync::content_keys;
use crate::sync::handshake::local_schema;
use crate::sync::link::join::{self, Join};
use crate::sync::test_support::{Device, Member};

/// A fresh installation: an empty vault of its own and device keys, no
/// identity and no data (what the join flow creates).
struct Newcomer {
    device: Device,
    keys: DeviceKeys,
}

fn newcomer() -> Newcomer {
    let device = Device::new();
    let keys = device
        .db()
        .write(|tx| keys::ensure_device_keys(tx, uuid::Uuid::new_v4(), 1))
        .expect("device keys");
    Newcomer { device, keys }
}

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

fn threads(device: &Device) -> Vec<String> {
    query::read(device.db(), |r| {
        r.query_map("SELECT id FROM chat_threads ORDER BY id", &[], |row| {
            row.get(0)
        })
    })
    .expect("read threads")
}

fn effective_devices(device: &Device, vault: &[u8; 32]) -> BTreeSet<([u8; 32], bool)> {
    query::read(device.db(), |r| {
        let valid = device_list::valid_lists(&device_list::load_all(r)?, vault);
        Ok(device_list::effective(&valid)
            .map(|s| {
                s.list
                    .devices
                    .iter()
                    .map(|d| (d.device_pubkey, d.role == ListRole::Main))
                    .collect()
            })
            .unwrap_or_default())
    })
    .expect("read list")
}

fn pending_of(device: &Device) -> Vec<Pending> {
    query::read(device.db(), |r| pending::load_all(r)).expect("read pending")
}

fn vault_secret(device: &Device) -> Option<[u8; 32]> {
    query::read(device.db(), |r| keys::vault_secret(r))
        .expect("read")
        .map(|s| *s)
}

struct Run {
    host: Result<Outcome, LinkError>,
    join: Result<join::Joined, LinkError>,
    shown: Option<String>,
}

/// Runs both sides against each other with the given code and decision.
async fn link(
    main: &Member,
    joiner: &Newcomer,
    host_code: &LinkCode,
    join_code: &LinkCode,
    decision: Decision,
    joiner_schema: SchemaVersion,
) -> Run {
    let (host_side, join_side) = duplex(1 << 22);
    let (host_recv, host_send) = split(host_side);
    let (mut join_recv, mut join_send) = split(join_side);
    let shown = std::sync::Mutex::new(None);
    let host = Host {
        replica: &main.device.replica,
        keys: &main.keys,
        vault: main.vault,
        code: host_code,
        schema: local_schema(),
        now_ms: 1_000,
    };
    let joining = Join {
        replica: &joiner.device.replica,
        keys: &joiner.keys,
        vault_device_uuid: joiner.device.db().device_id(),
        code: join_code,
        name: "new laptop",
        schema: joiner_schema,
        now_ms: 1_000,
    };
    // As on a real connection, the host's end of the stream closes when
    // its side is over, so a joiner waiting on it learns that.
    let host_side = async {
        let (mut send, mut recv) = (host_send, host_recv);
        run(
            &mut send,
            &mut recv,
            joiner.keys.endpoint_id,
            &host,
            |new| *shown.lock().expect("lock") = Some(new.name),
            async move { decision },
        )
        .await
    };
    let (host_result, join_result) = tokio::join!(
        host_side,
        join::run(
            &mut join_send,
            &mut join_recv,
            main.keys.endpoint_id,
            &joining,
            |_| {},
        ),
    );
    Run {
        host: host_result,
        join: join_result,
        shown: shown.into_inner().expect("lock"),
    }
}

#[tokio::test]
async fn a_new_device_links_as_a_linked_device_without_the_vault_secret() {
    let main = Member::genesis();
    write_thread(&main.device, "t1");
    let joiner = newcomer();
    let code = LinkCode::generate();

    let outcome = link(
        &main,
        &joiner,
        &code,
        &code,
        Decision::Accept { as_main: false },
        local_schema(),
    )
    .await;

    assert_eq!(
        outcome.host.expect("linked"),
        Outcome::Linked {
            device: joiner.keys.device_pubkey,
            name: "new laptop".into()
        }
    );
    assert!(!outcome.join.expect("joined").as_main);
    assert_eq!(outcome.shown.as_deref(), Some("new laptop"));

    assert_eq!(threads(&joiner.device), ["t1"], "the data arrived");
    assert_eq!(
        vault_secret(&joiner.device),
        None,
        "SC-009: no vault secret without the role"
    );
    let expected: BTreeSet<_> = [
        (main.keys.device_pubkey, true),
        (joiner.keys.device_pubkey, false),
    ]
    .into();
    assert_eq!(effective_devices(&main.device, &main.vault), expected);
    assert_eq!(effective_devices(&joiner.device, &main.vault), expected);
    assert!(
        pending_of(&main.device).is_empty(),
        "the host's record is gone"
    );
    let waiting = pending_of(&joiner.device);
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].state, State::AwaitingPublication);
    let key = query::read(joiner.device.db(), |r| content_keys::current_key(r, &[])).expect("read");
    assert!(
        key.is_some(),
        "the content key was wrapped for the new device"
    );
}

#[tokio::test]
async fn a_new_device_can_become_a_main_device_and_then_holds_the_vault_secret() {
    let main = Member::genesis();
    let joiner = newcomer();
    let code = LinkCode::generate();

    let outcome = link(
        &main,
        &joiner,
        &code,
        &code,
        Decision::Accept { as_main: true },
        local_schema(),
    )
    .await;

    assert!(outcome.join.expect("joined").as_main);
    outcome.host.expect("linked");
    assert_eq!(
        vault_secret(&joiner.device),
        vault_secret(&main.device),
        "SC-012: the vault secret arrived over the link"
    );
    assert!(
        effective_devices(&main.device, &main.vault).contains(&(joiner.keys.device_pubkey, true))
    );
}

#[tokio::test]
async fn a_wrong_code_transfers_nothing() {
    let main = Member::genesis();
    write_thread(&main.device, "t1");
    let joiner = newcomer();
    let before = effective_devices(&main.device, &main.vault);

    let outcome = link(
        &main,
        &joiner,
        &LinkCode::generate(),
        &LinkCode::generate(),
        Decision::Accept { as_main: false },
        local_schema(),
    )
    .await;

    assert!(matches!(outcome.host, Err(LinkError::BadProof)));
    assert!(outcome.join.is_err());
    assert_eq!(outcome.shown, None, "the host never showed the name");
    assert!(threads(&joiner.device).is_empty());
    assert!(pending_of(&main.device).is_empty());
    assert_eq!(effective_devices(&main.device, &main.vault), before);
}

#[tokio::test]
async fn a_declined_link_gives_the_new_device_nothing() {
    let main = Member::genesis();
    write_thread(&main.device, "t1");
    let joiner = newcomer();
    let code = LinkCode::generate();
    let before = effective_devices(&main.device, &main.vault);

    let outcome = link(
        &main,
        &joiner,
        &code,
        &code,
        Decision::Reject,
        local_schema(),
    )
    .await;

    assert_eq!(outcome.host.expect("declined"), Outcome::Rejected);
    assert!(matches!(outcome.join, Err(LinkError::Declined)));
    assert!(threads(&joiner.device).is_empty());
    assert!(pending_of(&main.device).is_empty());
    assert!(pending_of(&joiner.device).is_empty());
    assert_eq!(effective_devices(&main.device, &main.vault), before);
}

#[tokio::test]
async fn another_schema_version_ends_the_link_before_the_user_is_asked() {
    let main = Member::genesis();
    let joiner = newcomer();
    let code = LinkCode::generate();
    let mut newer = local_schema();
    newer.holzi_migration += 1;

    let outcome = link(
        &main,
        &joiner,
        &code,
        &code,
        Decision::Accept { as_main: false },
        newer,
    )
    .await;

    assert!(matches!(outcome.host, Err(LinkError::Incompatible)));
    assert!(matches!(
        outcome.join,
        Err(LinkError::Aborted(AbortReason::Incompatible))
    ));
    assert_eq!(outcome.shown, None);
    assert!(threads(&joiner.device).is_empty());
}

#[tokio::test]
async fn a_linked_device_cannot_link_another() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    linked.device.pull_from(&main.device);
    let joiner = newcomer();
    let code = LinkCode::generate();

    let outcome = link(
        &linked,
        &joiner,
        &code,
        &code,
        Decision::Accept { as_main: false },
        local_schema(),
    )
    .await;

    assert!(
        matches!(outcome.host, Err(LinkError::NotMainDevice)),
        "{:?}",
        outcome.host
    );
    assert!(outcome.join.is_err());
    assert!(pending_of(&linked.device).is_empty());
    assert!(threads(&joiner.device).is_empty());
}

/// A joiner that speaks the protocol correctly up to the transfer and then
/// disappears, as a crash or a cut connection would.
async fn joiner_that_vanishes(
    main: &Member,
    joiner: &Newcomer,
    code: &LinkCode,
) -> (Result<Outcome, LinkError>, [u8; 32]) {
    use crate::sync::link::code::{Role as CodeRole, Transcript};
    let (host_side, join_side) = duplex(1 << 22);
    let (mut host_recv, mut host_send) = split(host_side);
    let (mut join_recv, mut join_send) = split(join_side);
    let host = Host {
        replica: &main.device.replica,
        keys: &main.keys,
        vault: main.vault,
        code,
        schema: local_schema(),
        now_ms: 1_000,
    };
    let script = async {
        let LinkMessage::Hello {
            nonce_h,
            endpoint_h,
            device_h,
        } = expect_value(&mut join_recv, HANDSHAKE_FRAME_LIMIT)
            .await
            .expect("hello")
        else {
            panic!("hello");
        };
        let nonce_n = [7u8; 32];
        let transcript = Transcript {
            nonce_h,
            nonce_n,
            endpoint_h,
            endpoint_n: joiner.keys.endpoint_id,
            device_h,
            device_n: joiner.keys.device_pubkey,
        };
        let proof = LinkMessage::Proof {
            nonce_n,
            endpoint_n: joiner.keys.endpoint_id,
            device_n: joiner.keys.device_pubkey,
            vault_device_uuid: joiner.device.db().device_id(),
            name: "vanishing".into(),
            schema: local_schema(),
            mac_n: code.proof(CodeRole::Joiner, &transcript),
        };
        write_value(&mut join_send, &proof, HANDSHAKE_FRAME_LIMIT)
            .await
            .expect("proof");
        expect_value::<_, LinkMessage>(&mut join_recv, HANDSHAKE_FRAME_LIMIT)
            .await
            .expect("host proof");
        let link_id = loop {
            match expect_value::<_, LinkMessage>(&mut join_recv, FRAME_LIMIT)
                .await
                .expect("frame")
            {
                LinkMessage::Transfer { link_id, .. } => break link_id,
                _ => continue,
            }
        };
        // Dropping both halves ends the stream without a `Done`.
        drop(join_send);
        drop(join_recv);
        link_id
    };
    let (host_result, link_id) = tokio::join!(
        run(
            &mut host_send,
            &mut host_recv,
            joiner.keys.endpoint_id,
            &host,
            |_| {},
            async { Decision::Accept { as_main: false } },
        ),
        script,
    );
    (host_result, link_id)
}

#[tokio::test]
async fn a_cut_before_done_leaves_no_new_list_on_the_host() {
    let main = Member::genesis();
    let joiner = newcomer();
    let code = LinkCode::generate();
    let before = effective_devices(&main.device, &main.vault);

    let (host, link_id) = joiner_that_vanishes(&main, &joiner, &code).await;

    assert!(host.is_err(), "no Done, no link");
    assert_eq!(
        effective_devices(&main.device, &main.vault),
        before,
        "FR-025: the list is not published without Done"
    );
    let open = pending_of(&main.device);
    assert_eq!(open.len(), 1, "the record stays for a resume");
    assert_eq!(open[0].link_id, link_id);
    assert_eq!(open[0].state, State::Transferring);
    let published =
        finish_pending(&main.device.replica, &main.keys, main.vault, 2_000).expect("finish");
    assert_eq!(
        published, 0,
        "a record that never reached awaiting_publication is not published"
    );
}

#[tokio::test]
async fn a_crash_between_done_and_the_list_commit_is_finished_once() {
    let main = Member::genesis();
    let joiner = newcomer();
    let code = LinkCode::generate();
    let (_, link_id) = joiner_that_vanishes(&main, &joiner, &code).await;
    // The first commit of `publish` happened, the second did not.
    main.device
        .db()
        .write(|tx| pending::set_state(tx, &link_id, State::AwaitingPublication))
        .expect("state");
    assert!(
        !effective_devices(&main.device, &main.vault).contains(&(joiner.keys.device_pubkey, false))
    );

    let first =
        finish_pending(&main.device.replica, &main.keys, main.vault, 2_000).expect("finish");
    let second =
        finish_pending(&main.device.replica, &main.keys, main.vault, 2_000).expect("finish again");

    assert_eq!((first, second), (1, 0), "published exactly once");
    assert!(
        effective_devices(&main.device, &main.vault).contains(&(joiner.keys.device_pubkey, false))
    );
    assert!(pending_of(&main.device).is_empty());
}

#[tokio::test]
async fn a_record_older_than_a_day_is_dropped() {
    let main = Member::genesis();
    let joiner = newcomer();
    let code = LinkCode::generate();
    let _ = joiner_that_vanishes(&main, &joiner, &code).await;

    finish_pending(
        &main.device.replica,
        &main.keys,
        main.vault,
        1_000 + pending::MAX_AGE_MS + 1,
    )
    .expect("finish");

    assert!(pending_of(&main.device).is_empty());
}

#[tokio::test]
async fn the_joiner_drops_its_record_once_the_host_list_names_it() {
    let main = Member::genesis();
    let joiner = newcomer();
    let code = LinkCode::generate();
    link(
        &main,
        &joiner,
        &code,
        &code,
        Decision::Accept { as_main: false },
        local_schema(),
    )
    .await
    .join
    .expect("joined");
    assert_eq!(pending_of(&joiner.device).len(), 1);

    let dropped = join::finish_pending(&joiner.device.replica, 2_000).expect("finish");

    assert_eq!(dropped, 0);
    assert_eq!(pending_of(&joiner.device).len(), 1);

    let dropped = join::finish_pending_after_host_publication(
        &joiner.device.replica,
        &joiner.keys,
        main.vault,
        2_000,
    )
    .expect("finish after publication");

    assert_eq!(dropped, 1);
    assert!(pending_of(&joiner.device).is_empty());
}
