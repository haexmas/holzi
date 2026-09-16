//! `claude -p` process driver (tasks.md T013, T015). Drives one
//! `stream_chat` call to completion as a subprocess and translates its
//! `--output-format stream-json` events into [`StreamChunk`]s.
//!
//! The command uses an isolated configuration directory and routes Claude's
//! live permission-prompt tool through the shared holzi approval bridge.

use std::path::Path;
use std::process::Stdio;
use std::time::Instant;

use serde_json::json;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::adapters::types::{StreamChunk, StreamError};
use crate::adapters::{AdapterError, AdapterStream, ChatRequest};

use super::approval_bridge;
use super::process::{configure_process_group, map_spawn_error, ChildLifecycle};
use super::{build_transcript_prompt, DelegateChatContext};

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
    mcp_config: &Path,
    system_prompt_path: Option<&Path>,
    tmp: &TempDir,
) -> Command {
    let mut cmd = Command::new(binary);
    cmd.arg("-p")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--include-partial-messages")
        .arg("--permission-mode")
        .arg("default")
        .arg("--mcp-config")
        .arg(mcp_config)
        .arg("--permission-prompt-tool")
        .arg("mcp__holzi-approve__approve");
    if let Some(system_prompt_path) = system_prompt_path {
        cmd.arg("--append-system-prompt-file")
            .arg(system_prompt_path);
    }
    cmd.env("CLAUDE_CONFIG_DIR", tmp.path())
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

/// Starts one isolated Claude Code invocation and exposes its output as an adapter stream.
pub(super) async fn spawn_claude_invocation(
    binary: String,
    credentials: Vec<u8>,
    req: ChatRequest,
    context: DelegateChatContext,
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
    let system_prompt_path = req.system_prompt.as_deref().map(|system_prompt| {
        let path = tmp.path().join("system-prompt.txt");
        (path, system_prompt)
    });
    if let Some((path, system_prompt)) = &system_prompt_path {
        std::fs::write(path, system_prompt).map_err(|error| AdapterError::Http {
            reason: format!("failed to write Claude system prompt: {error}"),
        })?;
    }
    let socket_path = tmp.path().join("approval.sock");
    let listener =
        approval_bridge::bind_socket(&socket_path).map_err(|error| AdapterError::Http {
            reason: format!("failed to create Claude approval socket: {error}"),
        })?;
    let listener_task = approval_bridge::start_listener(listener, context.clone(), req.thread_id);
    let mcp_config_path = tmp.path().join("mcp-config.json");
    let current_exe = std::env::current_exe().map_err(|error| AdapterError::Http {
        reason: format!("failed to locate holzi executable for approval bridge: {error}"),
    })?;
    let mcp_config = json!({
        "mcpServers": {
            "holzi-approve": {
                "command": current_exe,
                "args": ["--internal-cli-delegate-approval-bridge", "--socket", socket_path]
            }
        }
    });
    std::fs::write(
        &mcp_config_path,
        serde_json::to_vec(&mcp_config).map_err(|error| AdapterError::Http {
            reason: format!("failed to serialize Claude MCP config: {error}"),
        })?,
    )
    .map_err(|error| AdapterError::Http {
        reason: format!("failed to write Claude MCP config: {error}"),
    })?;
    let mut cmd = build_command(
        &binary,
        &mcp_config_path,
        system_prompt_path.as_ref().map(|(path, _)| path.as_path()),
        &tmp,
    );
    configure_process_group(&mut cmd);
    cmd.env("CLAUDE_CODE_OAUTH_TOKEN", &token);

    let mut child = ChildLifecycle::spawn(&mut cmd).map_err(|e| map_spawn_error(&binary, e))?;

    let mut stdin = child
        .child_mut()
        .stdin
        .take()
        .ok_or_else(|| AdapterError::Http {
            reason: "failed to capture claude stdin".into(),
        })?;
    stdin
        .write_all(prompt.as_bytes())
        .await
        .map_err(|error| AdapterError::Http {
            reason: format!("failed to write Claude prompt: {error}"),
        })?;
    stdin.shutdown().await.map_err(|error| AdapterError::Http {
        reason: format!("failed to close Claude prompt input: {error}"),
    })?;
    let stdout = child
        .child_mut()
        .stdout
        .take()
        .ok_or_else(|| AdapterError::Http {
            reason: "failed to capture claude stdout".into(),
        })?;
    let mut stderr = child
        .child_mut()
        .stderr
        .take()
        .ok_or_else(|| AdapterError::Http {
            reason: "failed to capture claude stderr".into(),
        })?;

    let (tx, rx) = mpsc::unbounded_channel();
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
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
                        let remaining = MAX_STDERR_BYTES.saturating_sub(buf.len());
                        if remaining > 0 {
                            buf.extend_from_slice(&chunk[..n.min(remaining)]);
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
            let line = tokio::select! {
                _ = task_cancellation.cancelled() => {
                    child.terminate_and_reap().await;
                    stderr_task.abort();
                    let _ = stderr_task.await;
                    listener_task.abort();
                    let _ = listener_task.await;
                    return;
                }
                line = reader.next_line() => line,
            };
            match line {
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
                                child.terminate_and_reap().await;
                                stderr_task.abort();
                                let _ = stderr_task.await;
                                listener_task.abort();
                                let _ = listener_task.await;
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

        // A successful result is the end of this one-shot invocation, even
        // if a CLI keeps its server loop alive after emitting it.
        child.terminate_and_reap().await;
        let stderr_bytes = stderr_task.await.unwrap_or_default();
        listener_task.abort();
        let _ = listener_task.await;

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

    Ok(AdapterStream::new_with_cancellation(
        rx,
        join.abort_handle(),
        cancellation,
    ))
}
