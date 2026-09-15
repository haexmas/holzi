//! `send_message` admission: deriving deterministic message ids from an
//! idempotency key, resolving a call against any prior send sharing that
//! key, and persisting a fresh send transactionally.
//!
//! Split out of `chat/commands.rs` (2026-09-15 review) — third of five
//! steps. See `chat/commands.rs`'s own history for the rest of the split
//! plan.

use uuid::Uuid;

use crate::storage::chat_messages::{self as msg_store, ChatMessage, FinishReason, MessageRole};
use crate::storage::chat_threads::{self as thread_store, ChatThread};

/// Deterministically derives the (user, assistant) message ids for a
/// `send_message` call from its `idempotencyKey`. The same key always
/// yields the same pair, so a retried `invoke()` can be recognised and
/// answered with the original ids without any extra state — no cache,
/// no second column (contract §send_message: "die zugehörigen
/// User-/Assistant-IDs werden aus diesem Send-Vorgang wiederverwendet").
pub fn derive_message_ids(idempotency_key: &str) -> (Uuid, Uuid) {
    let user_message_id = Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("user:{idempotency_key}").as_bytes(),
    );
    let assistant_message_id = Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("assistant:{idempotency_key}").as_bytes(),
    );
    (user_message_id, assistant_message_id)
}

/// Outcome of deduping a `send_message` call against any prior send
/// sharing the same `idempotencyKey`. This only guards against the
/// frontend retrying its own uncertain `invoke()` call — it never
/// resumes or restarts a generation that failed after the user message
/// was accepted (see contracts/tauri-commands.md §send_message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotentSend {
    /// No prior send used this key — proceed with a normal insert
    /// using the given (deterministic) ids.
    Fresh {
        user_message_id: Uuid,
        assistant_message_id: Uuid,
    },
    /// A prior send with this key already exists and matches the
    /// requested thread/content — reuse its ids; insert nothing, start
    /// no new stream.
    Duplicate {
        thread_id: Uuid,
        user_message_id: Uuid,
        assistant_message_id: Uuid,
    },
    /// A prior send with this key exists but its thread or content
    /// differs from this request — the caller must reject with
    /// `HolziError::InvalidInput`.
    Mismatch,
}

/// Resolves a `send_message` call's idempotency against the DB.
/// `requested_thread_id` is the caller's raw `args.thread_id`, not yet
/// resolved to a real thread: `None` means "any thread", since a
/// retried call may not know the thread a prior, possibly
/// unacknowledged, attempt created.
pub fn resolve_idempotent_send(
    conn: &haex_crdt::rusqlite::Connection,
    idempotency_key: &str,
    requested_thread_id: Option<Uuid>,
    content: &str,
) -> haex_crdt::rusqlite::Result<IdempotentSend> {
    let (user_message_id, assistant_message_id) = derive_message_ids(idempotency_key);
    let Some(existing) = msg_store::find_by_idempotency_key(conn, idempotency_key)? else {
        return Ok(IdempotentSend::Fresh {
            user_message_id,
            assistant_message_id,
        });
    };
    let thread_matches = requested_thread_id.map_or(true, |t| t == existing.thread_id);
    if !thread_matches || existing.content != content {
        return Ok(IdempotentSend::Mismatch);
    }
    // Rows written before the role-separated derivation used the raw key for
    // the user id and `{key}:assistant` for the assistant id. Keep returning
    // that pair for those rows so a retry remains byte-for-byte compatible.
    let legacy_user_message_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, idempotency_key.as_bytes());
    let assistant_message_id = if existing.id == legacy_user_message_id {
        Uuid::new_v5(
            &Uuid::NAMESPACE_OID,
            format!("{idempotency_key}:assistant").as_bytes(),
        )
    } else {
        assistant_message_id
    };
    Ok(IdempotentSend::Duplicate {
        thread_id: existing.thread_id,
        user_message_id: existing.id,
        assistant_message_id,
    })
}

/// Result of atomically resolving and persisting a fresh send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedSend {
    Fresh {
        thread_id: Uuid,
        user_message_id: Uuid,
        assistant_message_id: Uuid,
    },
    Duplicate {
        thread_id: Uuid,
        user_message_id: Uuid,
        assistant_message_id: Uuid,
    },
    Mismatch,
    /// The caller named a thread that does not exist. The schema does not
    /// enforce foreign keys, so inserting anyway would strand the message
    /// rows under a `thread_id` `list_threads` can never return.
    UnknownThread {
        thread_id: Uuid,
    },
}

/// Resolves, reserves, and persists a send in one SQLite transaction.
///
/// `BEGIN IMMEDIATE` closes the gap between the idempotency lookup and the
/// unique-key insert. A concurrent loser therefore re-reads the committed
/// winner and receives the same result instead of a primary-key/unique error.
pub fn persist_send_transaction(
    conn: &haex_crdt::rusqlite::Connection,
    idempotency_key: &str,
    requested_thread_id: Option<Uuid>,
    content: &str,
    provider_id: Option<Uuid>,
    model_id: &str,
    now: i64,
) -> haex_crdt::rusqlite::Result<PersistedSend> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let decision =
            resolve_idempotent_send(conn, idempotency_key, requested_thread_id, content)?;
        match decision {
            IdempotentSend::Mismatch => Ok(PersistedSend::Mismatch),
            IdempotentSend::Duplicate {
                thread_id,
                user_message_id,
                assistant_message_id,
            } => Ok(PersistedSend::Duplicate {
                thread_id,
                user_message_id,
                assistant_message_id,
            }),
            IdempotentSend::Fresh {
                user_message_id,
                assistant_message_id,
            } => {
                let thread_id = requested_thread_id.unwrap_or_else(Uuid::new_v4);
                match requested_thread_id {
                    None => thread_store::insert_thread(
                        conn,
                        &ChatThread {
                            id: thread_id,
                            title: default_thread_title(content),
                            last_provider_id: provider_id,
                            last_model_id: Some(model_id.to_string()),
                            created_at: now,
                            updated_at: now,
                        },
                    )?,
                    // A named thread must already exist. `update_thread`
                    // reports zero affected rows for an unknown id, and
                    // without foreign keys the message insert below would
                    // otherwise succeed against a thread nothing can list.
                    Some(_) => {
                        let Some(existing) = thread_store::get_thread(conn, thread_id)? else {
                            return Ok(PersistedSend::UnknownThread { thread_id });
                        };
                        thread_store::update_thread(
                            conn,
                            thread_id,
                            &existing.title,
                            provider_id,
                            Some(model_id),
                            now,
                        )?
                    }
                };
                let parent_id = last_message_id(conn, thread_id)?;
                msg_store::insert_message(
                    conn,
                    &ChatMessage {
                        id: user_message_id,
                        thread_id,
                        parent_id,
                        role: MessageRole::User,
                        content: content.to_string(),
                        provider_id,
                        model_id: Some(model_id.to_string()),
                        prompt_tokens: None,
                        completion_tokens: None,
                        finish_reason: Some(FinishReason::Complete),
                        created_at: now,
                        idempotency_key: Some(idempotency_key.to_string()),
                        tool_name: None,
                        tool_call_id: None,
                        tool_input: None,
                        tool_is_error: None,
                        tool_source: None,
                    },
                )?;
                Ok(PersistedSend::Fresh {
                    thread_id,
                    user_message_id,
                    assistant_message_id,
                })
            }
        }
    })();
    match result {
        Ok(result) => match conn.execute_batch("COMMIT") {
            Ok(()) => Ok(result),
            Err(error) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(error)
            }
        },
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn default_thread_title(first_message: &str) -> String {
    let trimmed = first_message.trim();
    let s: String = trimmed.chars().take(60).collect();
    if s.is_empty() {
        "New chat".to_string()
    } else {
        s
    }
}

fn last_message_id(
    conn: &haex_crdt::rusqlite::Connection,
    thread_id: Uuid,
) -> haex_crdt::rusqlite::Result<Option<Uuid>> {
    let msgs = msg_store::list_messages(conn, thread_id)?;
    Ok(msgs.last().map(|m| m.id))
}

#[cfg(test)]
#[path = "send_admission_tests.rs"]
mod tests;
