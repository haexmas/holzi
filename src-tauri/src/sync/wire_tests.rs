use tokio::io::{duplex, AsyncWriteExt};
use uuid::Uuid;

use super::*;
use crate::sync::change::Change;

fn samples() -> Vec<Message> {
    let list = ListRef {
        generation: 3,
        list_hash: [9; 32],
    };
    let schema = SchemaVersion {
        protocol: 1,
        holzi_migration: 21,
        crdt_trigger: 13,
    };
    vec![
        Message::Challenge {
            v: PROTOCOL_VERSION,
            nonce_a: [1; 32],
            endpoint_a: [2; 32],
            list,
        },
        Message::Response {
            v: PROTOCOL_VERSION,
            device_d: [3; 32],
            vault: [4; 32],
            nonce_d: [5; 32],
            schema,
            list,
            sig_d: Signature([6; 64]),
        },
        Message::Accept {
            device_a: [7; 32],
            schema,
            list,
            sig_a: Signature([8; 64]),
        },
        Message::Reject {
            code: RejectCode::Removed,
        },
        Message::DeviceListPush {
            payload: vec![1, 2, 3],
            signature: Signature([0; 64]),
        },
        Message::Progress {
            vector: Vector::from([(Uuid::new_v4(), "7/1".to_string())]),
            last_seen: vec![([1; 32], 42)],
        },
        Message::Pull {
            vector: Vector::new(),
            replace: false,
        },
        Message::Page(Page {
            changes: vec![Change {
                table: "chat_threads".to_string(),
                row_pks: r#"{"id":"t"}"#.to_string(),
                column: "title".to_string(),
                hlc: "7/1".to_string(),
                value: r#""x""#.to_string(),
                continues: false,
            }],
            group_continues: false,
            more: false,
            served: Vector::new(),
        }),
        Message::Resync {
            reason: ResyncReason::TombstonesExpired,
        },
    ]
}

#[tokio::test]
async fn every_message_survives_a_frame() {
    let (mut a, mut b) = duplex(1 << 20);
    for message in samples() {
        write_frame(&mut a, &message, FRAME_LIMIT)
            .await
            .expect("write");
        let read = expect_frame(&mut b, FRAME_LIMIT).await.expect("read");
        assert_eq!(read, message);
    }
}

#[tokio::test]
async fn an_oversized_length_is_refused_before_reading_the_body() {
    let (mut a, mut b) = duplex(64);
    // Only the length prefix is sent: the reader must refuse without waiting
    // for, or allocating, the announced body.
    let announced = (HANDSHAKE_FRAME_LIMIT as u32 + 1).to_be_bytes();
    a.write_all(&announced).await.expect("write length");

    let error = read_frame(&mut b, HANDSHAKE_FRAME_LIMIT)
        .await
        .expect_err("too large");
    assert!(matches!(error, WireError::FrameTooLarge { .. }));
    assert_eq!(error.code(), ErrorCode::FrameTooLarge);
}

#[tokio::test]
async fn a_message_over_the_limit_is_not_sent() {
    let (mut a, _b) = duplex(64);
    let message = Message::DeviceListPush {
        payload: vec![0; 100],
        signature: Signature([0; 64]),
    };
    let error = write_frame(&mut a, &message, 64)
        .await
        .expect_err("too large");
    assert!(matches!(error, WireError::FrameTooLarge { .. }));
}

#[tokio::test]
async fn an_unknown_message_is_a_protocol_error() {
    let (mut a, mut b) = duplex(64);
    // Variant index 200 does not exist.
    a.write_all(&[0, 0, 0, 1, 200]).await.expect("write");
    let error = read_frame(&mut b, FRAME_LIMIT).await.expect_err("unknown");
    assert_eq!(error.code(), ErrorCode::Protocol);
}

#[tokio::test]
async fn a_clean_end_before_a_frame_is_no_error() {
    let (a, mut b) = duplex(64);
    drop(a);
    assert!(read_frame(&mut b, FRAME_LIMIT)
        .await
        .expect("end")
        .is_none());
}

#[tokio::test]
async fn an_end_inside_a_frame_is_an_error() {
    let (mut a, mut b) = duplex(64);
    a.write_all(&[0, 0, 0, 10, 1]).await.expect("write");
    drop(a);
    assert!(matches!(
        read_frame(&mut b, FRAME_LIMIT).await,
        Err(WireError::Closed)
    ));
}
