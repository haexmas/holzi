use tokio::io::{duplex, split};

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
