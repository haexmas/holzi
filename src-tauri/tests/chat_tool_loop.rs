//! Integration coverage for the turn/step loop (`run_turn` in
//! `chat/commands.rs`) — User Story 1 (spec.md Acceptance Scenarios 1-3).
//!
//! Uses a scripted stub [`ProviderAdapter`] instead of a real model, and a
//! real bootstrapped `Database` (needed for `current_hlc()`, same
//! justification as `tests/chat_message_idempotency.rs`) so persisted rows
//! and their `parent_id` chain can be asserted directly.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use holzi_lib::adapters::types::{ChatRequest, ToolCall as LlmToolCall, ToolSpec};
use holzi_lib::adapters::{AdapterError, AdapterStream, ProviderAdapter, ProviderModel, StreamChunk, StreamError};
use holzi_lib::chat::commands::{abort_turn, run_turn, MAX_RETRY_ATTEMPTS, MAX_TOOL_ROUNDS};
use holzi_lib::chat::session::{ActiveSession, ChatState};
use holzi_lib::chat::tools::{ApprovalDecision, RiskClass, Tool, ToolResult as ToolExecResult};
use holzi_lib::identity::{holzi_migration_source, installation_id_path, HolziBootstrap, HOLZI_TRIGGER_VERSION};
use holzi_lib::storage::chat_messages::{self as msg_store, ChatMessage, FinishReason, MessageRole};
use holzi_lib::storage::chat_threads::{self as thread_store, ChatThread};
use holzi_lib::storage::preferences::{self, PrefScope};

/// Matches the private `chat.permission_mode` key in `chat/commands.rs`
/// (data-model.md) — there is no dedicated get/set command, only the
/// generic preferences store, so tests write it directly.
const PREF_PERMISSION_MODE: &str = "chat.permission_mode";

fn set_permission_mode(db: &Database, mode: &str) {
    let this_device = db.device_id();
    db.with_connection(|conn| {
        preferences::insert_or_update(conn, PrefScope::Device(this_device), PREF_PERMISSION_MODE, mode)
            .map(|_| ())
            .map_err(haex_crdt::Error::from)
    })
    .unwrap();
}

const PASSPHRASE: &str = "chat-tool-loop";

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
    std::mem::forget(tmp);
    db
}

/// Seeds a thread with a single persisted `user` message and returns its id.
fn seed_thread(db: &Database, thread_id: Uuid, user_message_id: Uuid) {
    db.with_connection(|conn| {
        thread_store::insert_thread(
            conn,
            &ChatThread {
                id: thread_id,
                title: "test thread".to_string(),
                last_provider_id: None,
                last_model_id: None,
                created_at: 0,
                updated_at: 0,
            },
        )?;
        msg_store::insert_message(
            conn,
            &ChatMessage {
                id: user_message_id,
                thread_id,
                parent_id: None,
                role: MessageRole::User,
                content: "hi".to_string(),
                provider_id: None,
                model_id: None,
                prompt_tokens: None,
                completion_tokens: None,
                finish_reason: Some(FinishReason::Complete),
                created_at: 0,
                idempotency_key: None,
                tool_name: None,
                tool_call_id: None,
                tool_input: None,
                tool_is_error: None,
                tool_source: None,
            },
        )?;
        Ok(())
    })
    .unwrap();
}

/// Scripted adapter: each `stream_chat` call pops the next `Vec` of
/// pre-baked chunks off the front and streams them in order. Panics if
/// asked for more steps than were scripted — that is a test-authoring bug,
/// not a case to handle gracefully.
struct StubAdapter {
    steps: StdMutex<VecDeque<Vec<Result<StreamChunk, StreamError>>>>,
}

impl StubAdapter {
    fn new(steps: Vec<Vec<Result<StreamChunk, StreamError>>>) -> Self {
        Self {
            steps: StdMutex::new(steps.into()),
        }
    }

    /// An adapter that always answers with the same tool call, forever —
    /// used to exercise the round limit (T011).
    fn always_calls_tool(name: &str) -> Self {
        let mut steps = Vec::new();
        for _ in 0..(MAX_TOOL_ROUNDS + 2) {
            steps.push(vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
                id: Uuid::new_v4().to_string(),
                name: name.to_string(),
                input: serde_json::json!({}),
            }]))]);
        }
        Self::new(steps)
    }
}

#[async_trait]
impl ProviderAdapter for StubAdapter {
    async fn list_models(&self) -> Result<Vec<ProviderModel>, AdapterError> {
        Ok(Vec::new())
    }

    async fn stream_chat(&self, _req: ChatRequest) -> Result<AdapterStream, AdapterError> {
        let chunks = self
            .steps
            .lock()
            .unwrap()
            .pop_front()
            .expect("stub adapter ran out of scripted steps");
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            for c in chunks {
                if tx.send(c).is_err() {
                    return;
                }
            }
        });
        Ok(AdapterStream::new(rx, task.abort_handle()))
    }
}

/// A trivial in-test tool. `behavior` controls whether it succeeds or
/// reports a tool-level error, without ever panicking either way.
struct ScriptedTool {
    name: &'static str,
    risk_class: RiskClass,
    fails: bool,
}

#[async_trait]
impl Tool for ScriptedTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        "test-only scripted tool"
    }

    fn source(&self) -> &'static str {
        "cli"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({ "type": "object" })
    }

    fn risk_class(&self) -> RiskClass {
        self.risk_class
    }

    async fn execute(&self, input: Value, _cancel: CancellationToken) -> ToolExecResult {
        if self.fails {
            ToolExecResult::error("scripted failure")
        } else {
            ToolExecResult::ok(format!("scripted result for {input}"))
        }
    }
}

fn base_request() -> ChatRequest {
    ChatRequest {
        model_id: "stub-model".to_string(),
        system_prompt: None,
        messages: Vec::new(),
        max_new_tokens: None,
        tools: vec![ToolSpec {
            name: "echo".to_string(),
            description: "echoes".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
        }],
    }
}

async fn session_with(adapter: StubAdapter) -> ActiveSession {
    ActiveSession {
        model_id: "stub-model".to_string(),
        provider_id: None,
        adapter: Arc::new(adapter),
        tokenizer_repo: String::new(),
        context_window: None,
    }
}

/// Spawns `run_turn` on its own task so the caller can respond to
/// `tool-permission-request` events while it is still awaiting them, and
/// returns every emitted `(event, payload)` in arrival order via a channel
/// instead of a `Vec` (unlike [`run_scripted_turn`], nothing here can wait
/// for the whole turn to finish before observing events). Stashes a fresh
/// `CancellationToken` into `chat_state.tool_cancellation` first, mirroring
/// what `send_message` does in production, so a test can call
/// `abort_turn(&chat_state)` exactly like a real `abort_current_generation`
/// invocation (T032).
#[allow(clippy::too_many_arguments)]
fn spawn_turn(
    db: Database,
    chat_state: Arc<ChatState>,
    session: ActiveSession,
    thread_id: Uuid,
    user_message_id: Uuid,
    assistant_message_id: Uuid,
    request: ChatRequest,
    stream: AdapterStream,
) -> (
    tokio::task::JoinHandle<()>,
    tokio::sync::mpsc::UnboundedReceiver<(String, Value)>,
) {
    let cancel_token = CancellationToken::new();
    *chat_state.tool_cancellation.lock().unwrap() = Some(cancel_token.clone());

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = tokio::spawn(async move {
        let mut emit = move |name: &'static str, payload: Value| {
            let _ = tx.send((name.to_string(), payload));
        };
        run_turn(
            &db,
            &chat_state,
            &session,
            thread_id,
            user_message_id,
            assistant_message_id,
            request,
            stream,
            cancel_token,
            &mut emit,
        )
        .await;
    });
    (handle, rx)
}

fn extract_request_id(payload: &Value) -> Uuid {
    payload["requestId"]
        .as_str()
        .expect("requestId is a string")
        .parse()
        .expect("requestId is a uuid")
}

/// Resolves a pending approval exactly like the `respond_tool_permission`
/// Tauri command would — that command is not itself callable without a
/// live `AppHandle`/`State`, so tests exercise the same underlying
/// `ChatState.pending_tool_approvals` mechanism directly.
fn respond(chat_state: &ChatState, request_id: Uuid, decision: ApprovalDecision) {
    let sender = chat_state
        .pending_tool_approvals
        .lock()
        .unwrap()
        .remove(&request_id)
        .expect("pending request must exist");
    let _ = sender.send(decision);
}

/// Runs a turn end-to-end against a scripted adapter/tool and returns the
/// persisted rows (oldest first) plus every emitted `(event, payload)`.
async fn run_scripted_turn(
    db: &Database,
    chat_state: &ChatState,
    session: &ActiveSession,
    thread_id: Uuid,
    user_message_id: Uuid,
    assistant_message_id: Uuid,
) -> (Vec<ChatMessage>, Vec<(String, Value)>) {
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();

    let mut events: Vec<(String, Value)> = Vec::new();
    let mut emit = |name: &'static str, payload: Value| {
        events.push((name.to_string(), payload));
    };

    run_turn(
        db,
        chat_state,
        session,
        thread_id,
        user_message_id,
        assistant_message_id,
        request,
        stream,
        CancellationToken::new(),
        &mut emit,
    )
    .await;

    let rows = db
        .with_connection(|conn| {
            msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from)
        })
        .unwrap();
    (rows, events)
}

#[tokio::test]
async fn tool_call_then_final_answer_persists_the_full_ordered_chain() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    // US1 fixtures predate the permission gate (Phase 4) and are not
    // testing it — `auto` lets the `Safe` scripted tool execute
    // immediately, same as before the gate existed.
    set_permission_mode(&db, "auto");

    let chat_state = ChatState::new();
    chat_state
        .tool_registry
        .lock()
        .unwrap()
        .register(Arc::new(ScriptedTool {
            name: "echo",
            risk_class: RiskClass::Safe,
            fails: false,
        }));

    let adapter = StubAdapter::new(vec![
        vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
            id: "call-1".to_string(),
            name: "echo".to_string(),
            input: serde_json::json!({ "x": 1 }),
        }]))],
        vec![
            Ok(StreamChunk::Delta {
                content: "final answer".to_string(),
                reasoning: None,
            }),
            Ok(StreamChunk::Done {
                finish_reason: Some("end_turn".to_string()),
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                ttft_ms: Some(3),
                total_ms: 20,
            }),
        ],
    ]);
    let session = session_with(adapter).await;

    let (rows, events) = run_scripted_turn(
        &db,
        &chat_state,
        &session,
        thread_id,
        user_message_id,
        assistant_message_id,
    )
    .await;

    // user -> tool_call -> tool_result -> assistant(final), in that order.
    assert_eq!(rows.len(), 4, "{rows:#?}");
    assert_eq!(rows[0].id, user_message_id);
    assert_eq!(rows[1].role, MessageRole::ToolCall);
    assert_eq!(rows[1].parent_id, Some(user_message_id));
    assert_eq!(rows[1].tool_name.as_deref(), Some("echo"));
    assert_eq!(rows[1].tool_call_id.as_deref(), Some("call-1"));
    assert_eq!(rows[2].role, MessageRole::ToolResult);
    assert_eq!(rows[2].parent_id, Some(rows[1].id));
    assert_eq!(rows[2].tool_call_id.as_deref(), Some("call-1"));
    assert_eq!(rows[2].tool_is_error, Some(false));
    assert_eq!(rows[3].role, MessageRole::Assistant);
    assert_eq!(rows[3].id, assistant_message_id);
    assert_eq!(rows[3].parent_id, Some(rows[2].id));
    assert_eq!(rows[3].content, "final answer");
    assert_eq!(rows[3].finish_reason, Some(FinishReason::Complete));

    let event_names: Vec<&str> = events.iter().map(|(n, _)| n.as_str()).collect();
    assert!(event_names.contains(&"chat-tool-call"));
    assert!(event_names.contains(&"chat-tool-result"));
    assert!(event_names.contains(&"chat-message-complete"));
    let (last_name, last_payload) = events.last().expect("at least one event");
    assert_eq!(last_name, "chat-turn-complete");
    assert_eq!(last_payload["finishReason"], "complete");
    assert_eq!(last_payload["assistantMessageId"], assistant_message_id.to_string());
}

#[tokio::test]
async fn a_tool_error_does_not_end_the_turn() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    // US1 fixtures predate the permission gate (Phase 4) and are not
    // testing it — `auto` lets the `Safe` scripted tool execute
    // immediately, same as before the gate existed.
    set_permission_mode(&db, "auto");

    let chat_state = ChatState::new();
    chat_state
        .tool_registry
        .lock()
        .unwrap()
        .register(Arc::new(ScriptedTool {
            name: "echo",
            risk_class: RiskClass::Safe,
            fails: true,
        }));

    let adapter = StubAdapter::new(vec![
        vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
            id: "call-1".to_string(),
            name: "echo".to_string(),
            input: serde_json::json!({}),
        }]))],
        vec![Ok(StreamChunk::Done {
            finish_reason: Some("end_turn".to_string()),
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            ttft_ms: Some(1),
            total_ms: 1,
        })],
    ]);
    let session = session_with(adapter).await;

    let (rows, _events) = run_scripted_turn(
        &db,
        &chat_state,
        &session,
        thread_id,
        user_message_id,
        assistant_message_id,
    )
    .await;

    assert_eq!(rows.len(), 4, "{rows:#?}");
    assert_eq!(rows[2].role, MessageRole::ToolResult);
    assert_eq!(rows[2].tool_is_error, Some(true));
    assert_eq!(rows[2].content, "scripted failure");
    // The turn must still reach a normal completion, not FinishReason::Error.
    assert_eq!(rows[3].role, MessageRole::Assistant);
    assert_eq!(rows[3].finish_reason, Some(FinishReason::Complete));
}

#[tokio::test]
async fn exceeding_the_round_limit_stops_with_tool_limit_reached() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    // US1 fixtures predate the permission gate (Phase 4) and are not
    // testing it — `auto` lets the `Safe` scripted tool execute
    // immediately, same as before the gate existed.
    set_permission_mode(&db, "auto");

    let chat_state = ChatState::new();
    chat_state
        .tool_registry
        .lock()
        .unwrap()
        .register(Arc::new(ScriptedTool {
            name: "echo",
            risk_class: RiskClass::Safe,
            fails: false,
        }));

    let adapter = StubAdapter::always_calls_tool("echo");
    let session = session_with(adapter).await;

    let (rows, events) = run_scripted_turn(
        &db,
        &chat_state,
        &session,
        thread_id,
        user_message_id,
        assistant_message_id,
    )
    .await;

    let final_row = rows
        .iter()
        .find(|m| m.id == assistant_message_id)
        .expect("terminal assistant row must be persisted");
    assert_eq!(final_row.finish_reason, Some(FinishReason::ToolLimitReached));

    // Exactly MAX_TOOL_ROUNDS tool_call rows — no further step ran once
    // the cap was reached.
    let tool_call_rows = rows
        .iter()
        .filter(|m| m.role == MessageRole::ToolCall)
        .count();
    assert_eq!(tool_call_rows, MAX_TOOL_ROUNDS);

    let (last_name, last_payload) = events.last().expect("at least one event");
    assert_eq!(last_name, "chat-turn-complete");
    assert_eq!(last_payload["finishReason"], "tool_limit_reached");
}

#[tokio::test]
async fn manual_mode_emits_a_permission_request_for_a_safe_tool() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    set_permission_mode(&db, "manual");

    let chat_state = Arc::new(ChatState::new());
    chat_state
        .tool_registry
        .lock()
        .unwrap()
        .register(Arc::new(ScriptedTool {
            name: "echo",
            risk_class: RiskClass::Safe,
            fails: false,
        }));

    let adapter = StubAdapter::new(vec![
        vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
            id: "call-1".to_string(),
            name: "echo".to_string(),
            input: serde_json::json!({}),
        }]))],
        vec![Ok(StreamChunk::Done {
            finish_reason: Some("end_turn".to_string()),
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            ttft_ms: Some(1),
            total_ms: 1,
        })],
    ]);
    let session = session_with(adapter).await;
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();

    let (handle, mut rx) = spawn_turn(
        db.clone(),
        chat_state.clone(),
        session,
        thread_id,
        user_message_id,
        assistant_message_id,
        request,
        stream,
    );

    let (name, payload) = rx.recv().await.expect("an event must arrive");
    assert_eq!(name, "tool-permission-request");
    assert_eq!(payload["riskClass"], "safe");
    assert_eq!(payload["toolName"], "echo");

    respond(&chat_state, extract_request_id(&payload), ApprovalDecision::Allow);
    handle.await.unwrap();

    let rows = db
        .with_connection(|conn| msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from))
        .unwrap();
    let tool_result = rows
        .iter()
        .find(|m| m.role == MessageRole::ToolResult)
        .expect("tool_result row");
    assert_eq!(tool_result.tool_is_error, Some(false));
}

#[tokio::test]
async fn manual_mode_emits_a_permission_request_for_the_risky_cli_tool() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    set_permission_mode(&db, "manual");

    // `ChatState::new()` registers the host-CLI tool unconditionally.
    let chat_state = Arc::new(ChatState::new());

    let adapter = StubAdapter::new(vec![
        vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
            id: "call-1".to_string(),
            name: "run_command".to_string(),
            input: serde_json::json!({ "command": "echo hi" }),
        }]))],
        vec![Ok(StreamChunk::Done {
            finish_reason: Some("end_turn".to_string()),
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            ttft_ms: Some(1),
            total_ms: 1,
        })],
    ]);
    let session = session_with(adapter).await;
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();

    let (handle, mut rx) = spawn_turn(
        db.clone(),
        chat_state.clone(),
        session,
        thread_id,
        user_message_id,
        assistant_message_id,
        request,
        stream,
    );

    let (name, payload) = rx.recv().await.expect("an event must arrive");
    assert_eq!(name, "tool-permission-request");
    assert_eq!(payload["riskClass"], "risky");
    assert_eq!(payload["toolName"], "run_command");

    respond(&chat_state, extract_request_id(&payload), ApprovalDecision::Allow);
    handle.await.unwrap();

    let rows = db
        .with_connection(|conn| msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from))
        .unwrap();
    let tool_result = rows
        .iter()
        .find(|m| m.role == MessageRole::ToolResult)
        .expect("tool_result row");
    assert_eq!(tool_result.tool_is_error, Some(false));
    assert_eq!(tool_result.content.trim(), "hi");
}

#[tokio::test]
async fn denying_a_request_produces_an_error_result_and_the_turn_continues() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    set_permission_mode(&db, "manual");

    let chat_state = Arc::new(ChatState::new());
    chat_state
        .tool_registry
        .lock()
        .unwrap()
        .register(Arc::new(ScriptedTool {
            name: "echo",
            risk_class: RiskClass::Safe,
            fails: false,
        }));

    let adapter = StubAdapter::new(vec![
        vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
            id: "call-1".to_string(),
            name: "echo".to_string(),
            input: serde_json::json!({}),
        }]))],
        vec![Ok(StreamChunk::Done {
            finish_reason: Some("end_turn".to_string()),
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            ttft_ms: Some(1),
            total_ms: 1,
        })],
    ]);
    let session = session_with(adapter).await;
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();

    let (handle, mut rx) = spawn_turn(
        db.clone(),
        chat_state.clone(),
        session,
        thread_id,
        user_message_id,
        assistant_message_id,
        request,
        stream,
    );

    let (_name, payload) = rx.recv().await.expect("an event must arrive");
    respond(&chat_state, extract_request_id(&payload), ApprovalDecision::Deny);
    handle.await.unwrap();

    let rows = db
        .with_connection(|conn| msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from))
        .unwrap();
    let tool_result = rows
        .iter()
        .find(|m| m.role == MessageRole::ToolResult)
        .expect("tool_result row");
    assert_eq!(tool_result.tool_is_error, Some(true));
    let final_row = rows
        .iter()
        .find(|m| m.id == assistant_message_id)
        .expect("terminal assistant row");
    assert_eq!(final_row.finish_reason, Some(FinishReason::Complete));
}

#[tokio::test]
async fn an_unanswered_request_leaves_the_turn_waiting_indefinitely() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    set_permission_mode(&db, "manual");

    let chat_state = Arc::new(ChatState::new());
    chat_state
        .tool_registry
        .lock()
        .unwrap()
        .register(Arc::new(ScriptedTool {
            name: "echo",
            risk_class: RiskClass::Safe,
            fails: false,
        }));

    let adapter = StubAdapter::new(vec![vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
        id: "call-1".to_string(),
        name: "echo".to_string(),
        input: serde_json::json!({}),
    }]))]]);
    let session = session_with(adapter).await;
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();

    let (mut handle, mut rx) = spawn_turn(
        db.clone(),
        chat_state.clone(),
        session,
        thread_id,
        user_message_id,
        assistant_message_id,
        request,
        stream,
    );

    let (name, _payload) = rx.recv().await.expect("an event must arrive");
    assert_eq!(name, "tool-permission-request");

    // Never respond — the turn must still be running after a bounded
    // wait, not auto-decided one way or the other (spec.md Acceptance
    // Scenario 5).
    let outcome = tokio::time::timeout(std::time::Duration::from_millis(200), &mut handle).await;
    assert!(
        outcome.is_err(),
        "an unanswered request must not be auto-decided"
    );
    handle.abort();
}

#[tokio::test]
async fn plan_mode_blocks_a_risky_tool_without_any_prompt() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    set_permission_mode(&db, "plan");

    // `ChatState::new()` registers the host-CLI tool, always `Risky`.
    let chat_state = ChatState::new();

    let adapter = StubAdapter::new(vec![
        vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
            id: "call-1".to_string(),
            name: "run_command".to_string(),
            input: serde_json::json!({ "command": "echo hi" }),
        }]))],
        vec![Ok(StreamChunk::Done {
            finish_reason: Some("end_turn".to_string()),
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            ttft_ms: Some(1),
            total_ms: 1,
        })],
    ]);
    let session = session_with(adapter).await;

    let (rows, events) = run_scripted_turn(
        &db,
        &chat_state,
        &session,
        thread_id,
        user_message_id,
        assistant_message_id,
    )
    .await;

    assert!(
        !events.iter().any(|(n, _)| n == "tool-permission-request"),
        "plan mode must never prompt for a blocked risky action"
    );
    let tool_result = rows
        .iter()
        .find(|m| m.role == MessageRole::ToolResult)
        .expect("tool_result row");
    assert_eq!(tool_result.tool_is_error, Some(true));
    assert_eq!(tool_result.content, "blocked_by_plan_mode");
    let final_row = rows
        .iter()
        .find(|m| m.id == assistant_message_id)
        .expect("terminal assistant row");
    assert_eq!(final_row.finish_reason, Some(FinishReason::Complete));
}

/// T030 (US3): aborting while the host-CLI tool is actually executing (as
/// opposed to waiting on approval) kills the OS process and ends the turn
/// as `Cancelled`, with no rows persisted for the interrupted round. Only
/// one step is scripted — if the loop incorrectly proceeded to a further
/// step after cancellation (T033), `StubAdapter` would panic on running out
/// of scripted steps, and `handle.await.unwrap()` below would fail.
#[tokio::test]
async fn aborting_during_tool_execution_kills_the_process_and_ends_the_turn() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    // `auto` still asks for a `Risky` call (the CLI tool is always Risky,
    // spec.md FR-015) — this only avoids a second, irrelevant prompt for
    // the mode switcher itself; `manual` would behave identically here.
    set_permission_mode(&db, "auto");

    let chat_state = Arc::new(ChatState::new());

    let adapter = StubAdapter::new(vec![vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
        id: "call-1".to_string(),
        name: "run_command".to_string(),
        input: serde_json::json!({ "command": "sleep 5" }),
    }]))]]);
    let session = session_with(adapter).await;
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();

    let (handle, mut rx) = spawn_turn(
        db.clone(),
        chat_state.clone(),
        session,
        thread_id,
        user_message_id,
        assistant_message_id,
        request,
        stream,
    );

    let (name, payload) = rx.recv().await.expect("an event must arrive");
    assert_eq!(name, "tool-permission-request");
    respond(&chat_state, extract_request_id(&payload), ApprovalDecision::Allow);

    // Give the approved call a moment to actually spawn `sleep 5` before
    // aborting, so this exercises "kill a running process", not "cancel
    // before it ever started".
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let started = std::time::Instant::now();
    abort_turn(&chat_state).unwrap();

    handle.await.unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "abort must kill the sleeping process rather than waiting it out: took {elapsed:?}"
    );

    let rows = db
        .with_connection(|conn| msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from))
        .unwrap();
    assert!(
        rows.iter().all(|m| m.role != MessageRole::ToolCall && m.role != MessageRole::ToolResult),
        "an interrupted round must leave no tool_call/tool_result rows"
    );
    let final_row = rows
        .iter()
        .find(|m| m.id == assistant_message_id)
        .expect("terminal assistant row");
    assert_eq!(final_row.finish_reason, Some(FinishReason::Cancelled));
    assert!(
        !events_after(&mut rx).iter().any(|(n, _)| n == "chat-tool-call" || n == "chat-tool-result"),
        "no tool-call/tool-result event for an interrupted round"
    );
}

/// T031 (US3): aborting while a `tool-permission-request` is still pending
/// (never approved or denied) resolves the wait as cancelled, distinct
/// from a user `Deny` — no `tool_result` row with a denial reason, and the
/// turn ends as `Cancelled` rather than continuing (spec.md Acceptance
/// Scenario, US3).
#[tokio::test]
async fn aborting_a_pending_permission_request_cancels_the_turn_not_denies_it() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    set_permission_mode(&db, "manual");

    let chat_state = Arc::new(ChatState::new());
    chat_state
        .tool_registry
        .lock()
        .unwrap()
        .register(Arc::new(ScriptedTool {
            name: "echo",
            risk_class: RiskClass::Safe,
            fails: false,
        }));

    // Only one step scripted — see the T030 test above for why that also
    // covers T033 (no further step after cancellation).
    let adapter = StubAdapter::new(vec![vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
        id: "call-1".to_string(),
        name: "echo".to_string(),
        input: serde_json::json!({}),
    }]))]]);
    let session = session_with(adapter).await;
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();

    let (handle, mut rx) = spawn_turn(
        db.clone(),
        chat_state.clone(),
        session,
        thread_id,
        user_message_id,
        assistant_message_id,
        request,
        stream,
    );

    let (name, _payload) = rx.recv().await.expect("an event must arrive");
    assert_eq!(name, "tool-permission-request");

    // Never respond — abort instead.
    abort_turn(&chat_state).unwrap();
    handle.await.unwrap();

    let rows = db
        .with_connection(|conn| msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from))
        .unwrap();
    assert!(
        rows.iter().all(|m| m.role != MessageRole::ToolCall && m.role != MessageRole::ToolResult),
        "an interrupted round must leave no tool_call/tool_result rows — no denial row either"
    );
    let final_row = rows
        .iter()
        .find(|m| m.id == assistant_message_id)
        .expect("terminal assistant row");
    assert_eq!(final_row.finish_reason, Some(FinishReason::Cancelled));
}

/// Drains whatever is left in `rx` right now without blocking further —
/// used after a turn has already finished to inspect the full event tail.
fn events_after(rx: &mut tokio::sync::mpsc::UnboundedReceiver<(String, Value)>) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
    out
}

#[tokio::test]
async fn two_independent_risky_calls_each_get_their_own_pending_request() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    set_permission_mode(&db, "manual");

    let chat_state = Arc::new(ChatState::new());
    {
        let mut registry = chat_state.tool_registry.lock().unwrap();
        registry.register(Arc::new(ScriptedTool {
            name: "tool_a",
            risk_class: RiskClass::Risky,
            fails: false,
        }));
        registry.register(Arc::new(ScriptedTool {
            name: "tool_b",
            risk_class: RiskClass::Risky,
            fails: false,
        }));
    }

    let adapter = StubAdapter::new(vec![
        vec![Ok(StreamChunk::ToolCalls(vec![
            LlmToolCall {
                id: "call-a".to_string(),
                name: "tool_a".to_string(),
                input: serde_json::json!({}),
            },
            LlmToolCall {
                id: "call-b".to_string(),
                name: "tool_b".to_string(),
                input: serde_json::json!({}),
            },
        ]))],
        vec![Ok(StreamChunk::Done {
            finish_reason: Some("end_turn".to_string()),
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            ttft_ms: Some(1),
            total_ms: 1,
        })],
    ]);
    let session = session_with(adapter).await;
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();

    let (mut handle, mut rx) = spawn_turn(
        db.clone(),
        chat_state.clone(),
        session,
        thread_id,
        user_message_id,
        assistant_message_id,
        request,
        stream,
    );

    let (name1, payload1) = rx.recv().await.expect("first request");
    assert_eq!(name1, "tool-permission-request");
    let (name2, payload2) = rx.recv().await.expect("second request");
    assert_eq!(name2, "tool-permission-request");

    let id1 = extract_request_id(&payload1);
    let id2 = extract_request_id(&payload2);
    assert_ne!(id1, id2, "each call must mint its own request id");

    respond(&chat_state, id1, ApprovalDecision::Allow);
    let outcome = tokio::time::timeout(std::time::Duration::from_millis(150), &mut handle).await;
    assert!(
        outcome.is_err(),
        "approving one request must not resolve or affect the other"
    );

    respond(&chat_state, id2, ApprovalDecision::Deny);
    handle.await.unwrap();

    let rows = db
        .with_connection(|conn| msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from))
        .unwrap();
    let result_a = rows
        .iter()
        .find(|m| m.role == MessageRole::ToolResult && m.tool_call_id.as_deref() == Some("call-a"))
        .expect("result for call-a");
    let result_b = rows
        .iter()
        .find(|m| m.role == MessageRole::ToolResult && m.tool_call_id.as_deref() == Some("call-b"))
        .expect("result for call-b");
    assert_eq!(result_a.tool_is_error, Some(false));
    assert_eq!(result_b.tool_is_error, Some(true));
}

#[tokio::test]
async fn a_mode_change_while_pending_only_affects_the_next_tool_use() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    set_permission_mode(&db, "manual");

    let chat_state = Arc::new(ChatState::new());
    chat_state
        .tool_registry
        .lock()
        .unwrap()
        .register(Arc::new(ScriptedTool {
            name: "echo",
            risk_class: RiskClass::Safe,
            fails: false,
        }));

    // Round 1 (Manual -> Ask) then round 2 (mode flips to Auto in
    // between -> must not ask again for the same Safe tool).
    let adapter = StubAdapter::new(vec![
        vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
            id: "call-1".to_string(),
            name: "echo".to_string(),
            input: serde_json::json!({}),
        }]))],
        vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
            id: "call-2".to_string(),
            name: "echo".to_string(),
            input: serde_json::json!({}),
        }]))],
        vec![Ok(StreamChunk::Done {
            finish_reason: Some("end_turn".to_string()),
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            ttft_ms: Some(1),
            total_ms: 1,
        })],
    ]);
    let session = session_with(adapter).await;
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();

    let (handle, mut rx) = spawn_turn(
        db.clone(),
        chat_state.clone(),
        session,
        thread_id,
        user_message_id,
        assistant_message_id,
        request,
        stream,
    );

    let (name1, payload1) = rx.recv().await.expect("round 1's request");
    assert_eq!(name1, "tool-permission-request");
    let id1 = extract_request_id(&payload1);

    // Flip the mode while round 1's request is still pending.
    set_permission_mode(&db, "auto");
    respond(&chat_state, id1, ApprovalDecision::Allow);

    handle.await.unwrap();
    rx.close();
    let mut further_requests = 0;
    while let Ok((name, _)) = rx.try_recv() {
        if name == "tool-permission-request" {
            further_requests += 1;
        }
    }
    assert_eq!(
        further_requests, 0,
        "round 2 must observe the new mode and execute without asking"
    );

    let rows = db
        .with_connection(|conn| msg_store::list_messages(conn, thread_id).map_err(haex_crdt::Error::from))
        .unwrap();
    let tool_call_rows = rows.iter().filter(|m| m.role == MessageRole::ToolCall).count();
    assert_eq!(tool_call_rows, 2, "both rounds must have executed");
}

/// T035 (US4): a transient failure retries automatically and only the
/// final, successful answer is ever persisted — no trace of the failed
/// attempt (spec.md Acceptance Scenario 1). `tokio::time::pause` makes the
/// retry backoff instant instead of a real 500ms wait.
#[tokio::test]
async fn a_transient_failure_retries_and_only_the_final_answer_persists() {
    tokio::time::pause();
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);

    let chat_state = ChatState::new();
    let adapter = StubAdapter::new(vec![
        vec![Err(StreamError::Transient("hiccup".to_string()))],
        vec![
            Ok(StreamChunk::Delta {
                content: "hi there".to_string(),
                reasoning: None,
            }),
            Ok(StreamChunk::Done {
                finish_reason: Some("end_turn".to_string()),
                prompt_tokens: Some(1),
                completion_tokens: Some(1),
                ttft_ms: Some(1),
                total_ms: 1,
            }),
        ],
    ]);
    let session = session_with(adapter).await;

    let (rows, events) = run_scripted_turn(
        &db,
        &chat_state,
        &session,
        thread_id,
        user_message_id,
        assistant_message_id,
    )
    .await;

    assert_eq!(rows.len(), 2, "{rows:#?}");
    let final_row = rows
        .iter()
        .find(|m| m.id == assistant_message_id)
        .expect("terminal assistant row");
    assert_eq!(final_row.finish_reason, Some(FinishReason::Complete));
    assert_eq!(final_row.content, "hi there");

    let retry_events: Vec<_> = events.iter().filter(|(n, _)| n == "chat-retry").collect();
    assert_eq!(retry_events.len(), 1, "{events:#?}");
    assert_eq!(retry_events[0].1["attempt"], 1);
}

/// T036 (US4): a transient failure that keeps recurring past the retry
/// budget ends the turn with `FinishReason::Error`, distinguishable from
/// `Cancelled` and `ToolLimitReached` (spec.md Acceptance Scenario 2).
#[tokio::test]
async fn a_transient_failure_past_the_retry_limit_ends_with_error() {
    tokio::time::pause();
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);

    let chat_state = ChatState::new();
    // The original attempt plus every retry all fail transiently,
    // exhausting the bounded budget.
    let steps = (0..=MAX_RETRY_ATTEMPTS)
        .map(|_| vec![Err(StreamError::Transient("still down".to_string()))])
        .collect();
    let adapter = StubAdapter::new(steps);
    let session = session_with(adapter).await;

    let (rows, events) = run_scripted_turn(
        &db,
        &chat_state,
        &session,
        thread_id,
        user_message_id,
        assistant_message_id,
    )
    .await;

    let final_row = rows
        .iter()
        .find(|m| m.id == assistant_message_id)
        .expect("terminal assistant row");
    assert_eq!(final_row.finish_reason, Some(FinishReason::Error));
    assert_ne!(final_row.finish_reason, Some(FinishReason::Cancelled));
    assert_ne!(final_row.finish_reason, Some(FinishReason::ToolLimitReached));

    let retry_events: Vec<_> = events.iter().filter(|(n, _)| n == "chat-retry").collect();
    assert_eq!(retry_events.len(), MAX_RETRY_ATTEMPTS, "{events:#?}");
}

/// A terminal (non-transient) `StreamError` must never be retried, even
/// with retry budget remaining — it would just reproduce deterministically.
#[tokio::test]
async fn a_terminal_stream_error_is_not_retried() {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);

    let chat_state = ChatState::new();
    let adapter = StubAdapter::new(vec![vec![Err(StreamError::Model(
        "invalid request".to_string(),
    ))]]);
    let session = session_with(adapter).await;

    let (rows, events) = run_scripted_turn(
        &db,
        &chat_state,
        &session,
        thread_id,
        user_message_id,
        assistant_message_id,
    )
    .await;

    let final_row = rows
        .iter()
        .find(|m| m.id == assistant_message_id)
        .expect("terminal assistant row");
    assert_eq!(final_row.finish_reason, Some(FinishReason::Error));
    assert!(!events.iter().any(|(n, _)| n == "chat-retry"));
}
