//! Integration test for the Claude Code delegate adapter (tasks.md T010).
//! Points `CliDelegateAdapter` at a stub script standing in for the real
//! `claude` binary and asserts `stream_chat` yields the expected
//! `Delta`/`Done` sequence — never `ToolCalls` (research.md §4).
//!
//! Unix-only: the stub is a `#!/bin/sh` script invoked directly (no
//! shell wrapper, matching how `claude.rs` spawns the real binary),
//! which needs the execute bit and shebang handling this test sets up
//! via `std::os::unix::fs::PermissionsExt`.

#![cfg(unix)]

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};

use holzi_lib::adapters::cli_delegate::{CliDelegateAdapter, DelegateVendor};
use holzi_lib::adapters::{ChatMessage, ChatRequest, ChatRole, ProviderAdapter, StreamChunk};

/// Canned `stream-json` transcript matching the real event shapes
/// captured live in research.md §1.
const STUB_TRANSCRIPT: &str = r#"{"type":"system","subtype":"init"}
{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}}
{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":" world"}}}
{"type":"result","is_error":false,"subtype":"success","result":"Hello world","usage":{"input_tokens":3,"output_tokens":2}}
"#;

fn write_stub(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("fake-claude");
    let script = format!("#!/bin/sh\ncat <<'EOF'\n{STUB_TRANSCRIPT}EOF\n");
    fs::write(&path, script).expect("write stub script");
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).expect("chmod stub script");
    path
}

fn sample_request() -> ChatRequest {
    ChatRequest {
        model_id: "claude-delegate".to_string(),
        system_prompt: None,
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: "hi".to_string(),
        }],
        reasoning_requested: false,
        max_new_tokens: None,
        tools: Vec::new(),
    }
}

#[tokio::test]
async fn stream_chat_yields_delta_then_done_and_never_tool_calls() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stub = write_stub(dir.path());

    let adapter = CliDelegateAdapter::new(
        DelegateVendor::Claude,
        b"fake-oauth-token".to_vec(),
        stub.to_string_lossy().into_owned(),
        Some(Arc::new(Mutex::new(HashMap::new()))),
        // No real Tauri app needed — a plain closure satisfies
        // `EventEmitter`, matching `tests/common/tool_loop_fixture.rs`'s
        // own emit-closure pattern for the built-in tool loop.
        Some(Arc::new(|_event: &str, _payload: serde_json::Value| {})),
    );

    let mut stream = adapter
        .stream_chat(sample_request())
        .await
        .expect("stream_chat should start");

    let mut deltas = Vec::new();
    let mut saw_done = false;
    while let Some(chunk) = stream.next().await {
        match chunk.expect("no stream error expected") {
            StreamChunk::Delta { content, .. } => deltas.push(content),
            StreamChunk::Done { .. } => {
                saw_done = true;
                break;
            }
            StreamChunk::ToolCalls(_) => panic!("a delegate must never emit ToolCalls"),
        }
    }

    assert_eq!(deltas.join(""), "Hello world");
    assert!(saw_done, "expected a Done chunk to end the stream");
}
