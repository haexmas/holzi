//! `claude -p` process driver (tasks.md T013, T015). Drives one
//! `stream_chat` call to completion as a subprocess and translates its
//! `--output-format stream-json` events into [`StreamChunk`]s.
//!
//! Phase 3 (US1) scope only: the command built here runs with
//! `--permission-prompts none` — a safe, fail-closed default (anything
//! needing approval is denied, the process never hangs waiting for a
//! host that doesn't exist yet) — and no `--mcp-config`/
//! `--permission-prompt-tool`. Phase 5 (US3, tasks.md T034/T035) edits
//! [`build_command`] in place to remove `--permission-prompts none` and
//! add the live approval bridge instead.

use std::process::Stdio;
use std::time::Instant;

use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::adapters::types::{StreamChunk, StreamError};
use crate::adapters::{AdapterError, AdapterStream, ChatRequest};

use super::build_transcript_prompt;

/// Caps how much of a `claude` stderr we keep around for an error
/// message — mirrors `chat/tools/cli.rs`'s own output cap, just applied
/// to diagnostics rather than command output.
const MAX_STDERR_BYTES: usize = 64 * 1024;

/// What one `stream-json` line means for the stream. `pub(super)` so
/// `claude_tests.rs` can exercise it directly without spawning a real
/// process (tasks.md T011).
pub(super) enum LineOutcome {
    /// Nothing worth surfacing (a system/status event, a tool-related
    /// event this feature doesn't model, an empty line).
    Ignore,
    Chunk(StreamChunk),
    /// Either a malformed/unparseable line (a well-formed `claude` never
    /// emits one, so seeing one means something genuinely went wrong —
    /// a truncated write, a version mismatch) or a `result` line
    /// reporting `is_error: true`.
    Error(StreamError),
}

/// Pure NDJSON-line parser — no I/O, so it's directly unit-testable
/// (tasks.md T011) against the real event shapes captured in
/// research.md §1. `ttft_ms` is `Some` only the first time a `Delta` is
/// produced (time-to-first-token), computed by the caller from `start`.
pub(super) fn parse_line(line: &str, ttft_ms: Option<u64>, total_ms: u64) -> LineOutcome {
    if line.trim().is_empty() {
        return LineOutcome::Ignore;
    }
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return LineOutcome::Error(StreamError::Internal(format!("invalid JSON line: {e}")))
        }
    };
    match value.get("type").and_then(|v| v.as_str()) {
        Some("stream_event") => {
            let Some(delta) = value
                .get("event")
                .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("content_block_delta"))
                .and_then(|e| e.get("delta"))
            else {
                return LineOutcome::Ignore;
            };
            let delta_type = delta.get("type").and_then(|t| t.as_str());
            let text = delta.get("text").and_then(|t| t.as_str());
            match (delta_type, text) {
                (Some("text_delta"), Some(text)) => LineOutcome::Chunk(StreamChunk::Delta {
                    content: text.to_string(),
                    reasoning: None,
                }),
                (Some("thinking_delta"), Some(text)) => LineOutcome::Chunk(StreamChunk::Delta {
                    content: String::new(),
                    reasoning: Some(text.to_string()),
                }),
                _ => LineOutcome::Ignore,
            }
        }
        Some("result") => {
            let is_error = value
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_error {
                let message = value
                    .get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("claude reported an error")
                    .to_string();
                return LineOutcome::Error(StreamError::Model(message));
            }
            let usage = value.get("usage");
            let prompt_tokens = usage
                .and_then(|u| u.get("input_tokens"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let completion_tokens = usage
                .and_then(|u| u.get("output_tokens"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            LineOutcome::Chunk(StreamChunk::Done {
                finish_reason: Some("complete".to_string()),
                prompt_tokens,
                completion_tokens,
                ttft_ms,
                total_ms,
            })
        }
        _ => LineOutcome::Ignore,
    }
}

fn build_command(
    binary: &str,
    prompt: &str,
    system_prompt: Option<&str>,
    tmp: &TempDir,
) -> Command {
    let mut cmd = Command::new(binary);
    cmd.arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--include-partial-messages")
        .arg("--permission-mode")
        .arg("default")
        // Phase 3 (US1) safe default — see this module's doc comment.
        // Phase 5 (US3) replaces this with the live approval bridge.
        .arg("--permission-prompts")
        .arg("none");
    if let Some(system_prompt) = system_prompt {
        cmd.arg("--append-system-prompt").arg(system_prompt);
    }
    cmd.env("CLAUDE_CONFIG_DIR", tmp.path())
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

pub(super) async fn spawn_claude_invocation(
    binary: String,
    credentials: Vec<u8>,
    req: ChatRequest,
) -> Result<AdapterStream, AdapterError> {
    let token = std::str::from_utf8(&credentials)
        .map_err(|_| AdapterError::InvalidCredentials)?
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(AdapterError::InvalidCredentials);
    }

    // Disposable per-invocation CLAUDE_CONFIG_DIR + cwd (spec 007-cli-delegate
    // FR-002/FR-003) — removed when `tmp` drops at the end of the spawned
    // task below, success or failure alike.
    let tmp = TempDir::new().map_err(|e| AdapterError::Http {
        reason: format!("failed to create temp dir for claude invocation: {e}"),
    })?;

    let prompt = build_transcript_prompt(&req);
    let mut cmd = build_command(&binary, &prompt, req.system_prompt.as_deref(), &tmp);
    cmd.env("CLAUDE_CODE_OAUTH_TOKEN", &token);

    let mut child = cmd.spawn().map_err(|e| AdapterError::Http {
        reason: format!("failed to spawn \"{binary}\": {e}"),
    })?;

    let stdout = child.stdout.take().ok_or_else(|| AdapterError::Http {
        reason: "failed to capture claude stdout".into(),
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| AdapterError::Http {
        reason: "failed to capture claude stderr".into(),
    })?;

    let (tx, rx) = mpsc::unbounded_channel();
    let join = tokio::spawn(async move {
        // Keep `tmp` alive for the whole process lifetime; it is removed
        // when this task ends, on every path below.
        let _tmp = tmp;

        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let mut chunk = [0_u8; 4096];
            loop {
                match stderr.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if buf.len() < MAX_STDERR_BYTES {
                            buf.extend_from_slice(&chunk[..n]);
                        }
                    }
                }
            }
            buf
        });

        let start = Instant::now();
        let mut ttft_ms = None;
        let mut saw_result = false;
        let mut reader = BufReader::new(stdout).lines();

        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    match parse_line(&line, ttft_ms, start.elapsed().as_millis() as u64) {
                        LineOutcome::Ignore => {}
                        LineOutcome::Chunk(chunk) => {
                            if matches!(chunk, StreamChunk::Delta { .. }) && ttft_ms.is_none() {
                                ttft_ms = Some(start.elapsed().as_millis() as u64);
                            }
                            let is_done = matches!(chunk, StreamChunk::Done { .. });
                            if tx.send(Ok(chunk)).is_err() {
                                // Receiver dropped (AdapterStream discarded) —
                                // stop, the caller no longer wants output.
                                let _ = child.start_kill();
                                return;
                            }
                            if is_done {
                                saw_result = true;
                                break;
                            }
                        }
                        LineOutcome::Error(err) => {
                            saw_result = true;
                            let _ = tx.send(Err(err));
                            break;
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    let _ = tx.send(Err(StreamError::Internal(format!(
                        "failed to read claude stdout: {e}"
                    ))));
                    break;
                }
            }
        }

        let stderr_bytes = stderr_task.await.unwrap_or_default();
        let _ = child.wait().await;

        if !saw_result {
            let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();
            let reason = if stderr_text.is_empty() {
                "claude exited without producing a result".to_string()
            } else {
                format!("claude exited without producing a result: {stderr_text}")
            };
            let _ = tx.send(Err(StreamError::Internal(reason)));
        }
    });

    Ok(AdapterStream::new(rx, join.abort_handle()))
}
