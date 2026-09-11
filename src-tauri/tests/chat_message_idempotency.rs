//! Integration coverage for the `idempotency_key` column added to
//! `chat_messages` (spec 002 §send_message, Slice E / T045).
//!
//! Needs a real bootstrapped `Database` for `current_hlc()` and the
//! partial unique index, so this lives in the integration suite rather
//! than a colocated `chat_messages_tests.rs` (see
//! `src-tauri/src/storage/preferences_tests.rs` for the same split).

use std::path::PathBuf;
use std::sync::Arc;

use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use uuid::Uuid;

use holzi_lib::chat::commands::{derive_message_ids, resolve_idempotent_send, IdempotentSend};
use holzi_lib::identity::{
    holzi_migration_source, installation_id_path, HolziBootstrap, HOLZI_TRIGGER_VERSION,
};
use holzi_lib::storage::chat_messages::{self, ChatMessage, FinishReason, MessageRole};

const PASSPHRASE: &str = "chat-message-idempotency";

fn make_config(db_path: PathBuf, installation_id: PathBuf) -> DatabaseConfig {
    DatabaseConfig {
        path: db_path,
        key: SqlCipherKey::new(PASSPHRASE),
        create_if_missing: true,
        bootstrap: Arc::new(HolziBootstrap::new(installation_id).with_alias("test")),
        signature_provider: Arc::new(NoopSignatureProvider),
        migration_source: holzi_migration_source(),
        trigger_version: HOLZI_TRIGGER_VERSION,
    }
}

fn open_db() -> Database {
    let tmp = tempfile::tempdir().expect("tmp dir");
    let db_path = tmp.path().join("vault.db");
    let installation_id_file = installation_id_path(tmp.path());
    let db = Database::open(make_config(db_path, installation_id_file)).expect("genesis open");
    std::mem::forget(tmp); // keep the tempdir alive for the DB's lifetime
    db
}

fn sample_message(
    id: Uuid,
    thread_id: Uuid,
    content: &str,
    idempotency_key: Option<&str>,
) -> ChatMessage {
    ChatMessage {
        id,
        thread_id,
        parent_id: None,
        role: MessageRole::User,
        content: content.to_string(),
        provider_id: None,
        model_id: None,
        prompt_tokens: None,
        completion_tokens: None,
        finish_reason: Some(FinishReason::Complete),
        created_at: 0,
        idempotency_key: idempotency_key.map(|s| s.to_string()),
    }
}

#[test]
fn find_by_idempotency_key_returns_none_when_absent() {
    let db = open_db();
    db.with_connection(|conn| {
        assert!(chat_messages::find_by_idempotency_key(conn, "missing-key")
            .unwrap()
            .is_none());
        Ok(())
    })
    .unwrap();
}

#[test]
fn find_by_idempotency_key_returns_the_matching_row() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let msg_id = Uuid::new_v4();
    db.with_connection(|conn| {
        chat_messages::insert_message(
            conn,
            &sample_message(msg_id, thread_id, "hello", Some("key-1")),
        )
        .unwrap();
        let found = chat_messages::find_by_idempotency_key(conn, "key-1")
            .unwrap()
            .expect("row must be found");
        assert_eq!(found.id, msg_id);
        assert_eq!(found.thread_id, thread_id);
        assert_eq!(found.content, "hello");
        Ok(())
    })
    .unwrap();
}

#[test]
fn reusing_an_idempotency_key_on_a_second_insert_violates_the_unique_index() {
    let db = open_db();
    db.with_connection(|conn| {
        chat_messages::insert_message(
            conn,
            &sample_message(Uuid::new_v4(), Uuid::new_v4(), "first", Some("dup-key")),
        )
        .unwrap();
        let err = chat_messages::insert_message(
            conn,
            &sample_message(Uuid::new_v4(), Uuid::new_v4(), "second", Some("dup-key")),
        )
        .expect_err("second insert with the same idempotency_key must fail");
        assert!(format!("{err}").contains("UNIQUE constraint failed"));
        Ok(())
    })
    .unwrap();
}

#[test]
fn multiple_null_idempotency_keys_are_allowed() {
    let db = open_db();
    db.with_connection(|conn| {
        chat_messages::insert_message(
            conn,
            &sample_message(Uuid::new_v4(), Uuid::new_v4(), "assistant reply 1", None),
        )
        .unwrap();
        chat_messages::insert_message(
            conn,
            &sample_message(Uuid::new_v4(), Uuid::new_v4(), "assistant reply 2", None),
        )
        .unwrap();
        Ok(())
    })
    .unwrap();
}

#[test]
fn resolve_fresh_key_yields_the_deterministic_ids() {
    let db = open_db();
    let (expected_user, expected_assistant) = derive_message_ids("fresh-key");
    db.with_connection(|conn| {
        let decision = resolve_idempotent_send(conn, "fresh-key", None, "hi there").unwrap();
        assert_eq!(
            decision,
            IdempotentSend::Fresh {
                user_message_id: expected_user,
                assistant_message_id: expected_assistant,
            }
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn resolve_duplicate_key_with_matching_thread_and_content_reuses_ids() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let (user_id, assistant_id) = derive_message_ids("retry-key");
    db.with_connection(|conn| {
        chat_messages::insert_message(
            conn,
            &sample_message(user_id, thread_id, "same content", Some("retry-key")),
        )
        .unwrap();

        let decision =
            resolve_idempotent_send(conn, "retry-key", Some(thread_id), "same content").unwrap();
        assert_eq!(
            decision,
            IdempotentSend::Duplicate {
                thread_id,
                user_message_id: user_id,
                assistant_message_id: assistant_id,
            }
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn resolve_duplicate_key_without_a_requested_thread_does_not_mismatch() {
    // The retry-triggering scenario: the frontend never learned the
    // resolved thread id from the first (uncertain) call, so it resends
    // `threadId: None` — that must NOT be treated as a mismatch.
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let (user_id, _assistant_id) = derive_message_ids("no-thread-key");
    db.with_connection(|conn| {
        chat_messages::insert_message(
            conn,
            &sample_message(user_id, thread_id, "content", Some("no-thread-key")),
        )
        .unwrap();

        let decision = resolve_idempotent_send(conn, "no-thread-key", None, "content").unwrap();
        assert!(matches!(decision, IdempotentSend::Duplicate { .. }));
        Ok(())
    })
    .unwrap();
}

#[test]
fn resolve_duplicate_key_with_mismatched_content_is_invalid_input() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let (user_id, _) = derive_message_ids("mismatch-content-key");
    db.with_connection(|conn| {
        chat_messages::insert_message(
            conn,
            &sample_message(user_id, thread_id, "original", Some("mismatch-content-key")),
        )
        .unwrap();

        let decision = resolve_idempotent_send(
            conn,
            "mismatch-content-key",
            Some(thread_id),
            "different content",
        )
        .unwrap();
        assert_eq!(decision, IdempotentSend::Mismatch);
        Ok(())
    })
    .unwrap();
}

#[test]
fn resolve_duplicate_key_with_mismatched_thread_is_invalid_input() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let other_thread_id = Uuid::new_v4();
    let (user_id, _) = derive_message_ids("mismatch-thread-key");
    db.with_connection(|conn| {
        chat_messages::insert_message(
            conn,
            &sample_message(user_id, thread_id, "content", Some("mismatch-thread-key")),
        )
        .unwrap();

        let decision = resolve_idempotent_send(
            conn,
            "mismatch-thread-key",
            Some(other_thread_id),
            "content",
        )
        .unwrap();
        assert_eq!(decision, IdempotentSend::Mismatch);
        Ok(())
    })
    .unwrap();
}
