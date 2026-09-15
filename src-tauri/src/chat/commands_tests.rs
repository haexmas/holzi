//! Unit tests for what remains in `chat/commands.rs`: thread-title
//! validation, model reasoning-capability detection, the operation
//! reservation/abort interaction, and tool-permission resolution.

use super::*;
use crate::chat::thread_commands::validate_thread_title;

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
