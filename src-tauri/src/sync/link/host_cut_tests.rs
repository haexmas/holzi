use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{duplex, split, AsyncWrite};

use super::*;
use crate::sync::handshake::local_schema;
use crate::sync::link::join::{self};
use crate::sync::test_support::Member;

use super::tests::{effective_devices, link, newcomer, pending_of, Newcomer};

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

/// A writer that stops working once the joiner has begun to receive the snapshot, so that
/// everything after the last page, the `Done`, never leaves the joiner.
struct CutAfterTransfer<W> {
    inner: W,
    cut: Arc<AtomicBool>,
}

impl<W: AsyncWrite + Unpin> AsyncWrite for CutAfterTransfer<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.cut.load(Ordering::SeqCst) {
            return Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()));
        }
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// A full link in which the joiner stores everything and its `Done` is lost.
async fn link_whose_done_is_lost(
    main: &Member,
    joiner: &Newcomer,
    code: &LinkCode,
) -> (Result<Outcome, LinkError>, Result<join::Joined, LinkError>) {
    let (host_side, join_side) = duplex(1 << 22);
    let (host_recv, host_send) = split(host_side);
    let (join_recv, join_send) = split(join_side);
    let cut = Arc::new(AtomicBool::new(false));
    let host = Host {
        replica: &main.device.replica,
        keys: &main.keys,
        vault: main.vault,
        code,
        schema: local_schema(),
        now_ms: 1_000,
    };
    let joining = join::Join {
        replica: &joiner.device.replica,
        keys: &joiner.keys,
        vault_device_uuid: joiner.device.db().device_id(),
        code,
        name: "new laptop",
        schema: local_schema(),
        now_ms: 1_000,
    };
    let host_side = async {
        let (mut send, mut recv) = (host_send, host_recv);
        run(
            &mut send,
            &mut recv,
            joiner.keys.endpoint_id,
            &host,
            |_| {},
            async { Decision::Accept { as_main: false } },
        )
        .await
    };
    let join_side = async {
        // Both halves go when the join is over, as a connection that is gone would.
        let (mut recv, mut send) = (
            join_recv,
            CutAfterTransfer {
                inner: join_send,
                cut: Arc::clone(&cut),
            },
        );
        join::run(
            &mut send,
            &mut recv,
            main.keys.endpoint_id,
            &joining,
            |_| cut.store(true, Ordering::SeqCst),
        )
        .await
    };
    tokio::join!(host_side, join_side)
}

#[tokio::test]
async fn a_done_that_never_arrives_leaves_the_joiner_linked_and_the_host_waiting() {
    let main = Member::genesis();
    let joiner = newcomer();
    let code = LinkCode::generate();
    let before = effective_devices(&main.device, &main.vault);

    let (host, joined) = link_whose_done_is_lost(&main, &joiner, &code).await;

    joined.expect("the joiner stored everything, so it is linked");
    assert!(host.is_err(), "the host never learned of it");
    assert_eq!(
        effective_devices(&main.device, &main.vault),
        before,
        "FR-025: the host does not publish without Done"
    );
    assert_eq!(pending_of(&main.device).len(), 1);
    assert_eq!(pending_of(&main.device)[0].state, State::Transferring);
    let record = pending_of(&joiner.device);
    assert_eq!(record.len(), 1, "the joiner keeps what it stored");
    assert_eq!(record[0].state, State::AwaitingPublication);
    assert!(
        effective_devices(&joiner.device, &main.vault)
            .contains(&(joiner.keys.device_pubkey, false)),
        "the joiner's own list names it"
    );
}

#[tokio::test]
async fn the_host_drops_its_waiting_record_when_the_new_device_turns_up_on_its_list() {
    let main = Member::genesis();
    let joiner = newcomer();
    let code = LinkCode::generate();
    let _ = link_whose_done_is_lost(&main, &joiner, &code).await;

    assert_eq!(
        drop_listed(&main.device.replica, main.vault).expect("drop"),
        0,
        "not listed yet"
    );

    // The joiner's list reaches the host through their meeting.
    let record = pending_of(&main.device).remove(0);
    main.device
        .db()
        .write(|tx| {
            let signed = device_list::check_pushed(
                &record.list_payload,
                &record.list_signature,
                &main.vault,
            )
            .map_err(haex_crdt::Error::consumer)?;
            device_list::insert(tx, &signed)
        })
        .expect("list arrives");

    assert_eq!(
        drop_listed(&main.device.replica, main.vault).expect("drop"),
        1
    );
    assert!(pending_of(&main.device).is_empty());
}
