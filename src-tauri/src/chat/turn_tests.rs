use super::*;
use crate::chat::commands::abort_turn;
use crate::storage::chat_threads::ChatThread;

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
