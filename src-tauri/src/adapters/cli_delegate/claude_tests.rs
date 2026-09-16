//! Unit tests for `claude.rs`'s NDJSON `stream-json` line parser
//! (tasks.md T011). Event shapes are the real ones captured live in
//! research.md §1, not invented fixtures.

use super::claude::{parse_line, LineOutcome};
use crate::adapters::types::{StreamChunk, StreamError};

#[test]
fn text_delta_becomes_a_content_chunk() {
    let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}}"#;
    match parse_line(line, None, 10) {
        LineOutcome::Chunk(StreamChunk::Delta { content, reasoning }) => {
            assert_eq!(content, "hello");
            assert!(reasoning.is_none());
        }
        _ => panic!("expected a Delta chunk"),
    }
}

#[test]
fn thinking_delta_becomes_a_reasoning_chunk() {
    let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","text":"pondering"}}}"#;
    match parse_line(line, None, 10) {
        LineOutcome::Chunk(StreamChunk::Delta { content, reasoning }) => {
            assert_eq!(content, "");
            assert_eq!(reasoning.as_deref(), Some("pondering"));
        }
        _ => panic!("expected a Delta chunk"),
    }
}

#[test]
fn successful_result_becomes_done() {
    let line = r#"{"is_error":false,"subtype":"success","result":"Done.","usage":{"input_tokens":12,"output_tokens":34}}"#.replace(
        r#""is_error":false"#,
        r#""type":"result","is_error":false"#,
    );
    match parse_line(&line, Some(5), 42) {
        LineOutcome::Chunk(StreamChunk::Done {
            finish_reason,
            prompt_tokens,
            completion_tokens,
            ttft_ms,
            total_ms,
        }) => {
            assert_eq!(finish_reason.as_deref(), Some("complete"));
            assert_eq!(prompt_tokens, Some(12));
            assert_eq!(completion_tokens, Some(34));
            assert_eq!(ttft_ms, Some(5));
            assert_eq!(total_ms, 42);
        }
        _ => panic!("expected a Done chunk"),
    }
}

#[test]
fn errored_result_becomes_a_model_error() {
    let line = r#"{"type":"result","is_error":true,"subtype":"error","result":"something broke"}"#;
    match parse_line(line, None, 10) {
        LineOutcome::Error(StreamError::Model(message)) => {
            assert_eq!(message, "something broke");
        }
        _ => panic!("expected a Model error"),
    }
}

#[test]
fn unrelated_system_events_are_ignored() {
    let line = r#"{"type":"system","subtype":"status","status":"requesting"}"#;
    assert!(matches!(parse_line(line, None, 10), LineOutcome::Ignore));
}

#[test]
fn blank_lines_are_ignored() {
    assert!(matches!(parse_line("", None, 10), LineOutcome::Ignore));
    assert!(matches!(parse_line("   ", None, 10), LineOutcome::Ignore));
}

#[test]
fn malformed_json_surfaces_as_an_internal_stream_error_not_a_panic() {
    let line = r#"{"type":"stream_event", this is not valid json"#;
    match parse_line(line, None, 10) {
        LineOutcome::Error(StreamError::Internal(_)) => {}
        _ => panic!("expected an Internal stream error for malformed JSON"),
    }
}
