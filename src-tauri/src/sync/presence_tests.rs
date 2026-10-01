use super::*;
use crate::sync::keys::DeviceKeys;

#[test]
fn mailbox_keys_are_deterministic_from_the_same_content_key_and_day() {
    let content_key = [7u8; 32];
    let (secret_a, public_a) = mailbox_keys(&content_key, 42).expect("keys");
    let (secret_b, public_b) = mailbox_keys(&content_key, 42).expect("keys");
    assert_eq!(secret_a.secret_bytes(), secret_b.secret_bytes());
    assert_eq!(public_a, public_b);
}

#[test]
fn mailbox_keys_differ_by_day() {
    let content_key = [7u8; 32];
    let (_, today) = mailbox_keys(&content_key, 42).expect("keys");
    let (_, yesterday) = mailbox_keys(&content_key, 41).expect("keys");
    assert_ne!(today, yesterday);
}

#[test]
fn a_receiver_with_the_same_content_key_opens_the_meeting_and_learns_the_real_sender() {
    let content_key = [3u8; 32];
    let day = day_tag_now();
    let (mb_sk, mb_pk) = mailbox_keys(&content_key, day).expect("mailbox keys");

    let sender = DeviceKeys::generate();
    let content = PresenceContent::own(
        sender.device_pubkey,
        sender.endpoint_id,
        Some("https://relay.example.org".to_string()),
        vec!["203.0.113.5:4433".parse().expect("addr")],
        3,
    );

    let gift_wrap = build(&sender, &content, &mb_pk).expect("build");
    let (opened_sender, opened_content) = open(&gift_wrap, &mb_sk).expect("open");

    assert_eq!(opened_sender, sender.device_pubkey);
    assert_eq!(opened_content, content);
    assert!(opened_content.is_fresh(opened_content.ts));
}

#[test]
fn a_different_day_mailbox_cannot_open_the_meeting() {
    let content_key = [3u8; 32];
    let (_, mb_pk) = mailbox_keys(&content_key, 100).expect("mailbox keys");
    let (wrong_sk, _) = mailbox_keys(&content_key, 101).expect("mailbox keys");

    let sender = DeviceKeys::generate();
    let content = PresenceContent::own(sender.device_pubkey, sender.endpoint_id, None, vec![], 1);
    let gift_wrap = build(&sender, &content, &mb_pk).expect("build");

    assert!(open(&gift_wrap, &wrong_sk).is_err());
}

#[test]
fn a_meeting_is_only_fresh_within_the_bounds() {
    let content = PresenceContent {
        v: 1,
        device: [0; 32],
        endpoint: [0; 32],
        iroh_relay: None,
        addrs: Vec::new(),
        list_generation: 1,
        ts: 1_000_000,
        nonce: [0; 16],
    };
    assert!(content.is_fresh(1_000_000));
    assert!(content.is_fresh(1_000_000 + MAX_AGE_MS));
    assert!(!content.is_fresh(1_000_000 + MAX_AGE_MS + 1));
    assert!(content.is_fresh(1_000_000 - MAX_FUTURE_MS));
    assert!(!content.is_fresh(1_000_000 - MAX_FUTURE_MS - 1));
}

#[test]
fn the_content_survives_the_json_roundtrip_with_hex_fields() {
    let content = PresenceContent::own(
        [9u8; 32],
        [8u8; 32],
        Some("https://relay.example.org".to_string()),
        vec!["203.0.113.5:4433".parse().expect("addr")],
        2,
    );
    let json = serde_json::to_string(&content).expect("encode");
    assert!(json.contains("\"device\":\"09090909"));
    let back: PresenceContent = serde_json::from_str(&json).expect("decode");
    assert_eq!(back, content);
}

#[test]
fn a_crypto_provider_is_installed_for_the_relay_websocket() {
    ensure_crypto_provider();
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
}
