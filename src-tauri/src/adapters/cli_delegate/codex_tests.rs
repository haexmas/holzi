//! Unit tests for `codex.rs`'s JSON-RPC line classifier and request
//! builder (tasks.md T012). Event shapes are the real ones captured
//! live in research.md §2, not invented fixtures.

use serde_json::json;

use super::codex::{build_request, categorize_line, Line};

#[test]
fn build_request_matches_the_client_request_envelope() {
    let value = build_request(3, "turn/start", json!({"threadId": "abc"}));
    assert_eq!(value["jsonrpc"], "2.0");
    assert_eq!(value["id"], 3);
    assert_eq!(value["method"], "turn/start");
    assert_eq!(value["params"]["threadId"], "abc");
}

#[test]
fn a_response_to_our_own_request_is_recognized() {
    let line = r#"{"id": 2, "result": {"thread": {"id": "01a0"}}}"#;
    match categorize_line(Some(line)) {
        Line::Response { id, value } => {
            assert_eq!(id, 2);
            assert_eq!(value["result"]["thread"]["id"], "01a0");
        }
        other => panic!("expected Response, got {other:?}"),
    }
}

#[test]
fn a_live_approval_request_is_recognized_as_a_server_request() {
    // Verbatim shape from the research.md §2 live spike.
    let line = r#"{"method": "item/commandExecution/requestApproval", "id": 0, "params": {"threadId": "t", "turnId": "u", "itemId": "exec-1", "startedAtMs": 1, "command": "touch x"}}"#;
    match categorize_line(Some(line)) {
        Line::ServerRequest { id, method, .. } => {
            assert_eq!(id, json!(0));
            assert_eq!(method, "item/commandExecution/requestApproval");
        }
        other => panic!("expected ServerRequest, got {other:?}"),
    }
}

#[test]
fn a_notification_has_no_id() {
    let line = r#"{"method": "turn/completed", "params": {"threadId": "t"}}"#;
    match categorize_line(Some(line)) {
        Line::Notification { method, .. } => assert_eq!(method, "turn/completed"),
        other => panic!("expected Notification, got {other:?}"),
    }
}

#[test]
fn eof_and_blank_and_malformed_lines_do_not_panic() {
    assert!(matches!(categorize_line(None), Line::Eof));
    assert!(matches!(categorize_line(Some("")), Line::Unrecognized));
    assert!(matches!(
        categorize_line(Some("not json")),
        Line::Unrecognized
    ));
}
