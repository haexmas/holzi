//! Integration test that exercises the real mistralrs runtime end-to-end.
//!
//! Skipped by default (marked `#[ignore]`) because it needs:
//!   * a GGUF file on disk pointed to by `HOLZI_TEST_GGUF`
//!   * optionally a tokenizer repo id in `HOLZI_TEST_GGUF_TOKENIZER`
//!     (defaults to `Qwen/Qwen2.5-0.5B-Instruct`, matching the model
//!     that carried Etappe 0's baseline numbers)
//!
//! Run with:
//!   HOLZI_TEST_GGUF=~/Projekte/holzi-etappe0/models/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf \
//!     cargo test --manifest-path src-tauri/Cargo.toml \
//!     --test local_inference -- --ignored
//!
//! For CUDA builds add `--features llm-cuda`.

#![cfg(feature = "llm-cpu")]

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use holzi_lib::adapters::types::{ChatMessage, ChatRequest, ChatRole, StreamChunk};
use holzi_lib::llm::local::LocalModel;

const DEFAULT_TOKENIZER: &str = "Qwen/Qwen2.5-0.5B-Instruct";
const MAX_NEW_TOKENS: usize = 64;
const ABORT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

fn model_path_from_env() -> Option<PathBuf> {
    env::var("HOLZI_TEST_GGUF")
        .ok()
        .map(|s| PathBuf::from(expand_home(&s)))
}

/// Manual `~/` expansion so tests do not need a shellexpand dep.
fn expand_home(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = env::var_os("HOME") {
            let mut p = PathBuf::from(home);
            p.push(rest);
            return p.to_string_lossy().into_owned();
        }
    }
    s.to_string()
}

#[tokio::test]
#[ignore = "requires HOLZI_TEST_GGUF pointing at a real GGUF file"]
async fn load_and_stream_generates_tokens() {
    let path = model_path_from_env().expect("HOLZI_TEST_GGUF is not set");
    let tokenizer =
        env::var("HOLZI_TEST_GGUF_TOKENIZER").unwrap_or_else(|_| DEFAULT_TOKENIZER.to_string());

    let model = LocalModel::load(&path, Some(&tokenizer))
        .await
        .expect("model load");

    let mut handle = model.stream_chat(ChatRequest {
        model_id: String::new(),
        system_prompt: Some("You are a terse assistant.".into()),
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: "Reply with the single word: pong.".into(),
        }],
        max_new_tokens: Some(MAX_NEW_TOKENS),
        tools: Vec::new(),
    });

    let mut total_content = String::new();
    let mut saw_done = false;
    let mut ttft_ms: Option<u64> = None;

    while let Some(item) = handle.next().await {
        match item.expect("stream error") {
            StreamChunk::Delta { content, .. } => total_content.push_str(&content),
            StreamChunk::Done { ttft_ms: t, .. } => {
                saw_done = true;
                ttft_ms = t;
                break;
            }
            StreamChunk::ToolCalls(_) => {
                panic!("this fixture's prompt does not request tool use")
            }
        }
    }

    // Token counts are optional (mistralrs 0.8.1 can close the stream
    // without emitting Done on CUDA; the synthetic Done then reports
    // None). The load-bearing assertions are: we saw a Done frame,
    // we saw non-empty content, and TTFT was observed.
    assert!(saw_done, "stream ended without Done frame");
    assert!(!total_content.is_empty(), "generated content is empty");
    assert!(
        ttft_ms.is_some(),
        "TTFT was never observed — no non-empty delta arrived",
    );
}

#[tokio::test]
#[ignore = "requires HOLZI_TEST_GGUF pointing at a real GGUF file"]
async fn abort_stops_generation_before_completion() {
    let path = model_path_from_env().expect("HOLZI_TEST_GGUF is not set");
    let tokenizer =
        env::var("HOLZI_TEST_GGUF_TOKENIZER").unwrap_or_else(|_| DEFAULT_TOKENIZER.to_string());

    let model = LocalModel::load(&path, Some(&tokenizer))
        .await
        .expect("model load");

    let mut handle = model.stream_chat(ChatRequest {
        model_id: String::new(),
        system_prompt: None,
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: "Count from 1 to 500 in words, one number per line.".into(),
        }],
        max_new_tokens: Some(512),
        tools: Vec::new(),
    });

    // Pull chunks until we get at least one non-empty delta so we know
    // generation has started, then abort.
    let mut got_content = false;
    while let Some(item) = handle.next().await {
        match item.expect("stream error before abort") {
            StreamChunk::Delta { content, .. } if !content.is_empty() => {
                got_content = true;
                break;
            }
            StreamChunk::Done { .. } => {
                panic!("generation completed before abort could take effect");
            }
            _ => continue,
        }
    }
    assert!(got_content, "no delta content arrived before abort");

    handle.abort_handle().abort();

    // After abort, the receiver must return `None` within a bounded
    // window. In-flight chunks may still arrive first; they are fine.
    let drain = async { while handle.next().await.is_some() {} };
    tokio::time::timeout(ABORT_DRAIN_TIMEOUT, drain)
        .await
        .expect("abort did not close the stream within the drain window");
}
