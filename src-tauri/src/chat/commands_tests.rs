//! Unit tests for the idempotency id-derivation. Pure function, no
//! I/O — the DB-facing dedup decision (`resolve_idempotent_send`) is
//! covered by the integration suite at
//! `tests/chat_message_idempotency.rs` because it needs a real
//! `chat_messages` table.

use super::commands::derive_message_ids;

#[test]
fn same_key_yields_the_same_ids_every_time() {
    let first = derive_message_ids("abc-123");
    let second = derive_message_ids("abc-123");
    assert_eq!(first, second);
}

#[test]
fn different_keys_yield_different_ids() {
    let (user_a, assistant_a) = derive_message_ids("key-a");
    let (user_b, assistant_b) = derive_message_ids("key-b");
    assert_ne!(user_a, user_b);
    assert_ne!(assistant_a, assistant_b);
}

#[test]
fn user_and_assistant_ids_never_collide_for_the_same_key() {
    let (user_id, assistant_id) = derive_message_ids("same-key");
    assert_ne!(user_id, assistant_id);
}

#[test]
fn role_namespaces_prevent_nested_key_collisions() {
    let (_, assistant_id) = derive_message_ids("key");
    let (nested_user_id, _) = derive_message_ids("key:assistant");
    assert_ne!(assistant_id, nested_user_id);
}
