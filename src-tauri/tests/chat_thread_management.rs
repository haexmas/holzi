//! Persistence contract tests for chat thread management (spec 006).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use uuid::Uuid;

use holzi_lib::identity::{holzi_migration_source, installation_id_path, HolziBootstrap};
use holzi_lib::storage::chat_messages::{self, ChatMessage, FinishReason, MessageRole};
use holzi_lib::storage::chat_threads::{self, ChatThread};

const PASSPHRASE: &str = "chat-thread-management";

fn open_vault(dir: &Path) -> Database {
    Database::open(DatabaseConfig {
        path: PathBuf::from(dir).join("vault.db"),
        key: SqlCipherKey::new(PASSPHRASE),
        create_if_missing: true,
        bootstrap: Arc::new(HolziBootstrap::new(installation_id_path(dir)).with_alias("test")),
        signature_provider: Arc::new(NoopSignatureProvider),
        migration_source: holzi_migration_source(),
        trigger_version: haex_crdt::DEFAULT_TRIGGER_VERSION,
    })
    .expect("vault open")
}

fn thread(id: Uuid) -> ChatThread {
    ChatThread {
        id,
        title: "Original title".to_string(),
        last_provider_id: None,
        last_model_id: None,
        created_at: 1_000,
        updated_at: 2_000,
    }
}

fn message(thread_id: Uuid) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4(),
        thread_id,
        parent_id: None,
        role: MessageRole::User,
        content: "message".to_string(),
        provider_id: None,
        model_id: None,
        prompt_tokens: None,
        completion_tokens: None,
        finish_reason: Some(FinishReason::Complete),
        created_at: 1_001,
        idempotency_key: None,
        tool_name: None,
        tool_call_id: None,
        tool_input: None,
        tool_is_error: None,
        tool_source: None,
        autonomy_mode: None,
    }
}

#[test]
fn renaming_changes_only_the_title() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());
    let id = Uuid::new_v4();

    db.with_connection(|conn| {
        chat_threads::insert_thread(conn, &thread(id))?;
        assert_eq!(chat_threads::rename_title(conn, id, "Renamed")?, 1);
        let saved = chat_threads::get_thread(conn, id)?.expect("thread remains");
        assert_eq!(saved.title, "Renamed");
        assert_eq!(saved.created_at, 1_000);
        assert_eq!(saved.updated_at, 2_000);
        Ok(())
    })
    .expect("rename succeeds");
}

#[test]
fn deleting_a_thread_removes_all_messages_as_one_action() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());
    let id = Uuid::new_v4();
    let other_id = Uuid::new_v4();

    db.with_connection(|conn| {
        chat_threads::insert_thread(conn, &thread(id))?;
        chat_messages::insert_message(conn, &message(id))?;
        chat_threads::insert_thread(conn, &thread(other_id))?;
        chat_messages::insert_message(conn, &message(other_id))?;
        assert!(chat_threads::delete_thread_and_messages(conn, id)?);
        assert!(chat_threads::get_thread(conn, id)?.is_none());
        assert!(chat_messages::list_messages(conn, id)?.is_empty());
        assert!(chat_threads::get_thread(conn, other_id)?.is_some());
        assert_eq!(chat_messages::list_messages(conn, other_id)?.len(), 1);
        Ok(())
    })
    .expect("delete succeeds");
}

#[test]
fn renaming_a_missing_thread_is_non_mutating() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());

    db.with_connection(|conn| {
        assert_eq!(chat_threads::rename_title(conn, Uuid::new_v4(), "New")?, 0);
        Ok(())
    })
    .expect("missing rename is handled");
}

#[test]
fn failed_thread_rename_preserves_the_original_title() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());
    let id = Uuid::new_v4();

    db.with_connection(|conn| {
        chat_threads::insert_thread(conn, &thread(id))?;
        conn.execute_batch(
            "CREATE TRIGGER reject_thread_update BEFORE UPDATE ON chat_threads
             BEGIN SELECT RAISE(ABORT, 'injected update failure'); END;",
        )?;

        assert!(chat_threads::rename_title(conn, id, "New").is_err());
        let saved = chat_threads::get_thread(conn, id)?.expect("thread remains");
        assert_eq!(saved.title, "Original title");
        assert_eq!(saved.created_at, 1_000);
        Ok(())
    })
    .expect("rollback preserves the title");
}

#[test]
fn deleting_a_missing_thread_is_non_mutating() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());

    db.with_connection(|conn| {
        assert!(!chat_threads::delete_thread_and_messages(
            conn,
            Uuid::new_v4()
        )?);
        Ok(())
    })
    .expect("missing delete is handled");
}

#[test]
fn failed_thread_delete_rolls_back_message_removal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());
    let id = Uuid::new_v4();

    db.with_connection(|conn| {
        chat_threads::insert_thread(conn, &thread(id))?;
        chat_messages::insert_message(conn, &message(id))?;
        conn.execute_batch(
            "CREATE TRIGGER reject_thread_delete BEFORE DELETE ON chat_threads
             BEGIN SELECT RAISE(ABORT, 'injected delete failure'); END;",
        )?;

        assert!(chat_threads::delete_thread_and_messages(conn, id).is_err());
        assert!(chat_threads::get_thread(conn, id)?.is_some());
        assert_eq!(chat_messages::list_messages(conn, id)?.len(), 1);
        Ok(())
    })
    .expect("rollback preserves the thread");
}
