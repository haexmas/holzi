//! Unit tests for `claude.rs`'s NDJSON `stream-json` line parser
//! (tasks.md T011). Event shapes are the real ones captured live in
//! research.md §1, not invented fixtures.

use super::autonomy::AutonomyMode;
use super::claude::{build_command, parse_line, LineOutcome};
use super::DelegateVendor;
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

fn model_args(model: &str) -> Vec<String> {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let cmd = build_command("claude", model, None, None, &tmp, AutonomyMode::Ungated);
    cmd.as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_real_model_id_is_passed_through_as_the_model_flag() {
    let args = model_args("claude-opus-5");
    let flag = args.iter().position(|a| a == "--model");
    assert_eq!(
        flag.and_then(|i| args.get(i + 1)).map(String::as_str),
        Some("claude-opus-5"),
    );
}

#[test]
fn an_empty_model_id_omits_the_model_flag() {
    assert!(!model_args("").contains(&"--model".to_string()));
}

#[test]
fn the_legacy_synthetic_vendor_id_omits_the_model_flag() {
    // Pre-refresh sessions persisted `DelegateVendor::Claude.as_str()`
    // ("claude") as their model id, back when `list_models` returned one
    // synthetic entry instead of real Anthropic model ids — that id isn't
    // a real `--model` value the CLI accepts, so it must fall back to the
    // CLI's own default exactly like an empty id does.
    assert!(!model_args(DelegateVendor::Claude.as_str()).contains(&"--model".to_string()));
}
