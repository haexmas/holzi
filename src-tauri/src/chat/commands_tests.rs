//! Unit tests for what remains in `chat/commands.rs`: thread-title
//! validation, model reasoning-capability detection, the operation
//! reservation/abort interaction, retry-budget behavior around the
//! initial request, and tool-permission resolution.

use super::*;
use crate::chat::thread_commands::validate_thread_title;
use crate::storage::chat_threads::ChatThread;

#[test]
fn thread_title_validation_trims_and_enforces_visible_length() {
    assert_eq!(
        validate_thread_title("  A useful title  ").unwrap(),
        "A useful title"
    );
    assert!(matches!(
        validate_thread_title("   "),
        Err(HolziError::InvalidInput { .. })
    ));
    assert!(matches!(
        validate_thread_title(&"x".repeat(121)),
        Err(HolziError::InvalidInput { .. })
    ));
}

#[test]
fn thread_title_validation_counts_grapheme_clusters() {
    let title = "e\u{301}".repeat(120);
    assert_eq!(validate_thread_title(&title).unwrap(), title);
    assert!(matches!(
        validate_thread_title(&"e\u{301}".repeat(121)),
        Err(HolziError::InvalidInput { .. })
    ));
}

#[test]
fn reasoning_capability_is_derived_conservatively_from_the_model_id() {
    assert!(model_supports_reasoning("Qwen/Qwen3-4B-Instruct"));
    assert!(model_supports_reasoning("claude-sonnet-4-20250514"));
    assert!(model_supports_reasoning("claude-haiku-4-5-20251001"));
    assert!(!model_supports_reasoning("Qwen/Qwen2.5-0.5B-Instruct"));
    assert!(!model_supports_reasoning("claude-3-5-sonnet"));
    assert!(!model_supports_reasoning("Qwen/Qwen3-4B-Instruct-2507"));
}

#[test]
fn tool_requests_disable_reasoning_for_qwen_tool_call_compatibility() {
    let tools = vec![ToolSpec {
        name: "run_command".to_string(),
        description: "Runs a shell command.".to_string(),
        input_schema: serde_json::json!({"type": "object"}),
    }];

    assert!(!reasoning_requested_for("Qwen/Qwen3-4B", &tools));
    assert!(reasoning_requested_for("Qwen/Qwen3-4B", &[]));
    assert!(reasoning_requested_for("claude-sonnet-4-20250514", &[]));
}

#[tokio::test]
async fn operation_reservation_survives_transfer_to_the_turn_task() {
    let chat = std::sync::Arc::new(ChatState::new());
    let operation = chat.acquire_operation().unwrap();
    let (finish, wait) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let _operation = operation;
        wait.await.unwrap();
    });
    assert!(
        chat.acquire_operation().is_err(),
        "a second send/load/vault transition must fail"
    );
    abort_turn(&chat).unwrap();
    assert!(
        chat.acquire_operation().is_err(),
        "abort must wait for turn persistence/cleanup"
    );
    finish.send(()).unwrap();
    task.await.unwrap();
    assert!(chat.acquire_operation().is_ok());
}

#[tokio::test]
async fn failed_thread_update_rolls_back_final_assistant_message() {
    let (tmp, db, thread_id) = tokio::task::spawn_blocking(|| {
            let tmp = tempfile::tempdir().unwrap();
            let db = haex_crdt::Database::open(crate::instances::vault_config::vault_config(
                "test-passphrase", &tmp.path().join("vault.db"),
                &crate::identity::installation_id_path(tmp.path()), true,
            )).unwrap();
            let thread_id = Uuid::new_v4();
            db.with_connection(|conn| {
                thread_store::insert_thread(conn, &ChatThread {
                    id: thread_id, title: "Keep me".into(), last_provider_id: None,
                    last_model_id: None, created_at: 0, updated_at: 0,
                })?;
                conn.execute_batch("CREATE TRIGGER reject_thread_update BEFORE UPDATE ON chat_threads BEGIN SELECT RAISE(ABORT, 'injected update failure'); END;")?;
                Ok(())
            }).unwrap();
            (tmp, db, thread_id)
        }).await.unwrap();
    let message = ChatMessage {
        role: MessageRole::Assistant,
        finish_reason: Some(FinishReason::Complete),
        content: "answer".into(),
        ..empty_tool_message(Uuid::new_v4(), thread_id, None)
    };
    assert!(persist_final_message(&db, message, None, "test".into())
        .await
        .is_err());
    tokio::task::spawn_blocking(move || {
        db.with_connection(|conn| {
            assert!(msg_store::list_messages(conn, thread_id)?.is_empty());
            assert_eq!(
                thread_store::get_thread(conn, thread_id)?.unwrap().title,
                "Keep me"
            );
            Ok(())
        })
        .unwrap();
        drop(db);
        drop(tmp);
    })
    .await
    .unwrap();
}
struct StartAdapter {
    attempts: std::sync::atomic::AtomicUsize,
    entered: tokio::sync::Notify,
    error: Option<u16>,
}

#[async_trait::async_trait]
impl crate::adapters::ProviderAdapter for StartAdapter {
    async fn list_models(
        &self,
    ) -> std::result::Result<Vec<crate::adapters::ProviderModel>, crate::adapters::AdapterError>
    {
        Ok(Vec::new())
    }

    async fn stream_chat(
        &self,
        _request: ChatRequest,
    ) -> std::result::Result<crate::adapters::types::AdapterStream, crate::adapters::AdapterError>
    {
        self.attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.entered.notify_one();
        match self.error {
            Some(status) => Err(crate::adapters::AdapterError::Status {
                status,
                body: "test failure".into(),
            }),
            None => std::future::pending().await,
        }
    }
}

#[tokio::test(start_paused = true)]
async fn initial_http_failures_use_the_same_bounded_retry_budget() {
    for status in [429, 503, 401] {
        let adapter = Arc::new(StartAdapter {
            attempts: Default::default(),
            entered: Default::default(),
            error: Some(status),
        });
        let session = ActiveSession {
            model_id: "test".into(),
            provider_id: None,
            adapter: adapter.clone(),
            tokenizer_repo: String::new(),
            context_window: None,
        };
        let request = ChatRequest {
            model_id: "test".into(),
            system_prompt: None,
            messages: Vec::new(),
            reasoning_requested: false,
            max_new_tokens: None,
            tools: Vec::new(),
        };
        let mut attempts = 0;
        let mut events = Vec::new();
        let result = start_step_stream(
            &session,
            &ChatState::new(),
            &request,
            &CancellationToken::new(),
            &mut attempts,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &mut |name, payload| events.push((name, payload)),
        )
        .await;
        assert!(matches!(result, Err(StreamStartError::Failed(_))));
        let retries = if status == 401 { 0 } else { MAX_RETRY_ATTEMPTS };
        assert_eq!(attempts, retries);
        assert_eq!(
            adapter.attempts.load(std::sync::atomic::Ordering::SeqCst),
            retries + 1
        );
        assert_eq!(events.len(), retries);
    }
}

#[tokio::test]
async fn initial_request_can_be_cancelled_before_response_headers() {
    let adapter = Arc::new(StartAdapter {
        attempts: Default::default(),
        entered: Default::default(),
        error: None,
    });
    let session = ActiveSession {
        model_id: "test".into(),
        provider_id: None,
        adapter: adapter.clone(),
        tokenizer_repo: String::new(),
        context_window: None,
    };
    let chat = Arc::new(ChatState::new());
    let cancel = CancellationToken::new();
    *chat.tool_cancellation.lock().unwrap() = Some(cancel.clone());
    let task_chat = chat.clone();
    let task = tokio::spawn(async move {
        let request = ChatRequest {
            model_id: "test".into(),
            system_prompt: None,
            messages: Vec::new(),
            reasoning_requested: false,
            max_new_tokens: None,
            tools: Vec::new(),
        };
        start_step_stream(
            &session,
            &task_chat,
            &request,
            &cancel,
            &mut 0,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &mut |_, _| {},
        )
        .await
    });
    adapter.entered.notified().await;
    abort_turn(&chat).unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(StreamStartError::Cancelled)
    ));
}

#[test]
fn late_permission_reply_after_cancellation_is_a_noop_but_unknown_id_is_rejected() {
    let chat = ChatState::new();
    let request_id = Uuid::new_v4();
    let (sender, mut receiver) = tokio::sync::oneshot::channel();
    chat.pending_tool_approvals
        .lock()
        .unwrap()
        .insert(request_id, sender);
    abort_turn(&chat).unwrap();
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Closed)
    ));
    assert!(resolve_tool_permission(
        &chat,
        RespondToolPermissionArgs {
            request_id,
            decision: ApprovalDecisionWire::Allow,
        }
    )
    .is_ok());
    assert!(matches!(
        resolve_tool_permission(
            &chat,
            RespondToolPermissionArgs {
                request_id: Uuid::new_v4(),
                decision: ApprovalDecisionWire::Allow,
            }
        ),
        Err(HolziError::InvalidInput { .. })
    ));
}
