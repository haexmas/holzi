//! The action bridge inside the tool loop (spec 032 US1, ADR-0006): a model calls an action as a
//! tool, the call goes out as `action-call-request`, the test answers it the way the webview's
//! `respond_action_call` does, and the persisted rows and the tool result show what the model saw.
//! The bridge itself (timeout, cancellation, ordering) is covered by
//! `src/chat/tools/action_bridge_tests.rs`; this file covers it in the loop.

#[allow(dead_code)]
#[path = "common/tool_loop_fixture.rs"]
mod tool_loop_fixture;
use tool_loop_fixture::*;

#[allow(dead_code)]
#[path = "common/action_fixture.rs"]
mod action_fixture;
use action_fixture::*;

use std::time::Duration;

use holzi_lib::adapters::types::ToolCall as LlmToolCall;
use holzi_lib::adapters::StreamChunk;
use holzi_lib::chat::session::ChatState;
use holzi_lib::chat::tools::action_bridge::{
    ActionBridge, ActionOutcomeWire, ActionReply, ACTION_FAILED_MESSAGE, EVENT_ACTION_CALL_REQUEST,
};
use holzi_lib::storage::chat_messages::{self as msg_store, ChatMessage, MessageRole};
use holzi_lib::storage::query;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

fn tool_call(id: &str, name: &str) -> StreamChunk {
    StreamChunk::ToolCalls(vec![LlmToolCall {
        id: id.to_string(),
        name: name.to_string(),
        input: json!({ "x": 1 }),
    }])
}

fn final_answer() -> Vec<Result<StreamChunk, holzi_lib::adapters::StreamError>> {
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
    ]
}

struct Turn {
    chat_state: Arc<ChatState>,
    db: haex_crdt::Database,
    thread_id: Uuid,
    handle: tokio::task::JoinHandle<()>,
}

/// Starts a turn in which the stub model calls `calls` (one round), then answers.
async fn start_turn(chat_state: ChatState, calls: Vec<StreamChunk>) -> Turn {
    let db = open_db();
    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    seed_thread(&db, thread_id, user_message_id);
    // `Change` and `Safe` run under Auto, which keeps the approval dialog out of these tests.
    set_permission_mode(&db, "auto");
    let chat_state = Arc::new(chat_state);
    let adapter = StubAdapter::new(vec![calls.into_iter().map(Ok).collect(), final_answer()]);
    let session = session_with(adapter).await;
    let request = base_request();
    let stream = session.adapter.stream_chat(request.clone()).await.unwrap();
    let (handle, _events) = spawn_turn(
        db.clone(),
        Arc::clone(&chat_state),
        session,
        thread_id,
        user_message_id,
        Uuid::new_v4(),
        request,
        stream,
    );
    Turn {
        chat_state,
        db,
        thread_id,
        handle,
    }
}

fn rows(turn: &Turn) -> Vec<ChatMessage> {
    query::read(&turn.db, |r| msg_store::list_messages(r, turn.thread_id)).unwrap()
}

fn tool_results(turn: &Turn) -> Vec<ChatMessage> {
    rows(turn)
        .into_iter()
        .filter(|m| m.role == MessageRole::ToolResult)
        .collect()
}

async fn next_call(events: &mut tokio::sync::mpsc::UnboundedReceiver<(String, Value)>) -> Value {
    let (event, payload) = tokio::time::timeout(Duration::from_secs(10), events.recv())
        .await
        .expect("an action call within 10 s")
        .expect("the channel is open");
    assert_eq!(event, EVENT_ACTION_CALL_REQUEST);
    payload
}

#[tokio::test]
async fn an_action_call_round_trips_and_persists_with_the_source_action() {
    let chat_state = ChatState::new();
    register_action(
        &chat_state,
        action_def("wm_state_get", "wm.state.get", "read"),
    );
    let mut events = attach_action_events(&chat_state);
    let mut turn = start_turn(chat_state, vec![tool_call("call-1", "wm_state_get")]).await;

    let payload = next_call(&mut events).await;
    assert_eq!(payload["actionId"], "wm.state.get");
    assert_eq!(payload["input"], json!({ "x": 1 }));
    assert!(answer(
        &turn.chat_state,
        &payload,
        ActionReply::Ok {
            result: json!({ "windows": [] })
        }
    ));
    (&mut turn.handle).await.unwrap();

    let all = rows(&turn);
    let call = all
        .iter()
        .find(|m| m.role == MessageRole::ToolCall)
        .expect("a tool_call row");
    assert_eq!(call.tool_name.as_deref(), Some("wm_state_get"));
    assert_eq!(call.tool_source.as_deref(), Some("action"));
    let result = &tool_results(&turn)[0];
    assert_eq!(result.tool_is_error, Some(false));
    assert!(result.content.contains("windows"), "{}", result.content);
}

#[tokio::test]
async fn a_runner_error_reaches_the_model_with_code_field_and_message() {
    let chat_state = ChatState::new();
    register_action(&chat_state, action_def("chat_x", "chat.x", "write"));
    let mut events = attach_action_events(&chat_state);
    let mut turn = start_turn(chat_state, vec![tool_call("call-1", "chat_x")]).await;

    let payload = next_call(&mut events).await;
    answer(
        &turn.chat_state,
        &payload,
        ActionReply::Err {
            code: "invalid_input".into(),
            field: Some("text".into()),
            message: "text must be a string".into(),
        },
    );
    (&mut turn.handle).await.unwrap();

    let result = &tool_results(&turn)[0];
    assert_eq!(result.tool_is_error, Some(true));
    let parsed: Value = serde_json::from_str(&result.content).expect("the content is JSON");
    assert_eq!(parsed["error"]["code"], "invalid_input");
    assert_eq!(parsed["error"]["field"], "text");
    assert_eq!(parsed["error"]["message"], "text must be a string");
}

#[tokio::test]
async fn a_failed_handler_shows_the_model_a_fixed_text_not_its_message() {
    let chat_state = ChatState::new();
    register_action(&chat_state, action_def("chat_x", "chat.x", "write"));
    let mut events = attach_action_events(&chat_state);
    let mut turn = start_turn(chat_state, vec![tool_call("call-1", "chat_x")]).await;

    let payload = next_call(&mut events).await;
    // The same conversion `respond_action_call` applies to what the webview sends.
    let wire = ActionOutcomeWire {
        ok: false,
        result: None,
        code: Some("failed".into()),
        field: None,
        message: Some("cannot read /home/someone/vault.db".into()),
    };
    answer(&turn.chat_state, &payload, wire.into());
    (&mut turn.handle).await.unwrap();

    let result = &tool_results(&turn)[0];
    assert!(
        result.content.contains(ACTION_FAILED_MESSAGE),
        "{}",
        result.content
    );
    assert!(!result.content.contains("vault.db"), "{}", result.content);
}

#[tokio::test]
async fn no_answer_ends_as_action_timeout() {
    let mut chat_state = ChatState::new();
    chat_state.action_bridge = ActionBridge::with_timeout(Duration::from_millis(60));
    register_action(
        &chat_state,
        action_def("wm_state_get", "wm.state.get", "read"),
    );
    let _events = attach_action_events(&chat_state);
    let mut turn = start_turn(chat_state, vec![tool_call("call-1", "wm_state_get")]).await;
    (&mut turn.handle).await.unwrap();

    let result = &tool_results(&turn)[0];
    assert_eq!(result.tool_is_error, Some(true));
    assert_eq!(result.content, "action_timeout");
}

#[tokio::test]
async fn without_an_emitter_the_call_is_action_unavailable() {
    let chat_state = ChatState::new();
    register_action(
        &chat_state,
        action_def("wm_state_get", "wm.state.get", "read"),
    );
    let mut turn = start_turn(chat_state, vec![tool_call("call-1", "wm_state_get")]).await;
    (&mut turn.handle).await.unwrap();

    let result = &tool_results(&turn)[0];
    assert_eq!(result.tool_is_error, Some(true));
    assert_eq!(result.content, "action_unavailable");
}

#[tokio::test]
async fn closing_the_vault_while_a_call_is_open_cancels_it() {
    let chat_state = ChatState::new();
    register_action(
        &chat_state,
        action_def("wm_state_get", "wm.state.get", "read"),
    );
    let mut events = attach_action_events(&chat_state);
    let mut turn = start_turn(chat_state, vec![tool_call("call-1", "wm_state_get")]).await;

    let payload = next_call(&mut events).await;
    turn.chat_state.reset_for_close();
    (&mut turn.handle).await.unwrap();

    let result = &tool_results(&turn)[0];
    assert_eq!(result.content, "tool_call_cancelled");
    // A late answer to the closed call does nothing.
    assert!(!answer(
        &turn.chat_state,
        &payload,
        ActionReply::Ok {
            result: Value::Null
        }
    ));
}

#[tokio::test]
async fn the_calls_of_one_round_run_one_after_the_other() {
    let chat_state = ChatState::new();
    register_action(&chat_state, action_def("wm_a", "wm.a", "read"));
    register_action(&chat_state, action_def("wm_b", "wm.b", "read"));
    let mut events = attach_action_events(&chat_state);
    let mut turn = start_turn(
        chat_state,
        vec![StreamChunk::ToolCalls(vec![
            LlmToolCall {
                id: "call-a".into(),
                name: "wm_a".into(),
                input: json!({}),
            },
            LlmToolCall {
                id: "call-b".into(),
                name: "wm_b".into(),
                input: json!({}),
            },
        ])],
    )
    .await;

    let first = next_call(&mut events).await;
    // The second call must wait for the first answer.
    assert!(
        tokio::time::timeout(Duration::from_millis(150), events.recv())
            .await
            .is_err(),
        "the second action went out before the first was answered"
    );
    answer(
        &turn.chat_state,
        &first,
        ActionReply::Ok {
            result: Value::Null,
        },
    );
    let second = next_call(&mut events).await;
    assert_ne!(first["actionId"], second["actionId"]);
    answer(
        &turn.chat_state,
        &second,
        ActionReply::Ok {
            result: Value::Null,
        },
    );
    (&mut turn.handle).await.unwrap();
    assert_eq!(tool_results(&turn).len(), 2);
}
