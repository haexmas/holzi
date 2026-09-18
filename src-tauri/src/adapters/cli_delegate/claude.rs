//! `claude -p` process driver (tasks.md T013, T015). Drives one
//! `stream_chat` call to completion as a subprocess and translates its
//! `--output-format stream-json` events into [`StreamChunk`]s.
//!
//! The command uses an isolated configuration directory and routes Claude's
//! live permission-prompt tool through the shared holzi approval bridge.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::json;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::adapters::types::{StreamChunk, StreamError};
use crate::adapters::{AdapterError, AdapterStream, ChatRequest};

use super::approval_bridge;
use super::autonomy::{self, AutonomyMode};
use super::process::{configure_process_group, map_spawn_error, ChildLifecycle};
use super::{build_transcript_prompt, DelegateChatContext, DelegateVendor};

/// Caps how much of a `claude` stderr we keep around for an error
/// message — mirrors `chat/tools/cli.rs`'s own output cap, just applied
/// to diagnostics rather than command output.
const MAX_STDERR_BYTES: usize = 64 * 1024;

/// Bounds the `Ungated` "first line vs. early exit" startup probe (see its
/// call site below) — mirrors `codex.rs`'s `HANDSHAKE_TIMEOUT` for the
/// analogous wait on Codex's own startup handshake.
const STARTUP_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

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
    model: &str,
    mcp_config: Option<&Path>,
    system_prompt_path: Option<&Path>,
    tmp: &TempDir,
    autonomy_mode: AutonomyMode,
) -> Command {
    let mut cmd = Command::new(binary);
    cmd.arg("-p")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--include-partial-messages");
    // `req.model_id` is one of `CLAUDE_MODEL_ALIASES` (mod.rs) — the model
    // picker only ever offers those, but an empty id (e.g. a session
    // loaded before this feature existed) falls back to the CLI's own
    // default rather than passing `--model ""`.
    if !model.is_empty() {
        cmd.arg("--model").arg(model);
    }
    if autonomy_mode == AutonomyMode::Ungated {
        // Native full-autonomy mechanism (research.md §2): no bridge is
        // spawned for this mode at all, so there is no approval tool to
        // wire up either. `Standard`/`GatedPermissive` keep today's exact
        // command — only the approval *decision function* changes for
        // `GatedPermissive` (approval_bridge.rs), not the invocation.
        cmd.arg("--permission-mode").arg("bypassPermissions");
    } else {
        let mcp_config = mcp_config.expect("mcp_config is required outside Ungated mode");
        cmd.arg("--permission-mode")
            .arg("default")
            .arg("--mcp-config")
            .arg(mcp_config)
            .arg("--permission-prompt-tool")
            .arg("mcp__holzi-approve__approve");
    }
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

/// Terminates `child`, drains `stderr_task`, aborts `listener_task` (if
/// any), and classifies the collected stderr against `autonomy_mode`
/// (tasks.md T009/T022): a recognized unsupported-mode rejection becomes
/// `AdapterError::Unavailable`; otherwise `fallback` (with the stderr text
/// appended, if any) becomes the reason of a generic `AdapterError::Http`.
/// Called on every startup failure path below regardless of mode — a
/// spawned child and its background tasks must not be left running just
/// because the failure happened before the `Ungated`-only peek-ahead
/// (code review). The unsupported-mode classification itself still only
/// ever matches in practice for `Ungated`: `Standard`/`GatedPermissive`
/// send `--permission-mode default`, a value every `claude` accepts, so
/// their failures always fall through to the generic `Http` case.
async fn classify_exit_error(
    child: &mut ChildLifecycle,
    stderr_task: tokio::task::JoinHandle<Vec<u8>>,
    listener_task: Option<tokio::task::JoinHandle<()>>,
    autonomy_mode: AutonomyMode,
    fallback: &str,
) -> AdapterError {
    child.terminate_and_reap().await;
    let stderr_bytes = stderr_task.await.unwrap_or_default();
    if let Some(task) = listener_task {
        task.abort();
        let _ = task.await;
    }
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();
    if let Some(unavailable) =
        autonomy::classify_autonomy_spawn_error(DelegateVendor::Claude, autonomy_mode, &stderr_text)
    {
        return AdapterError::Unavailable {
            reason: unavailable.to_string(),
        };
    }
    let reason = if stderr_text.is_empty() {
        fallback.to_string()
    } else {
        format!("{fallback}: {stderr_text}")
    };
    AdapterError::Http { reason }
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
    let autonomy_mode = req.autonomy_mode;
    // `Ungated` never needs a bridge process at all — Claude is told to
    // bypass permissions entirely, so there is no approval tool call to
    // relay (tasks.md T021).
    let (mcp_config_path, listener_task) = if autonomy_mode == AutonomyMode::Ungated {
        (None, None)
    } else {
        let socket_path = tmp.path().join("approval.sock");
        let listener =
            approval_bridge::bind_socket(&socket_path).map_err(|error| AdapterError::Http {
                reason: format!("failed to create Claude approval socket: {error}"),
            })?;
        let listener_task = approval_bridge::start_listener(
            listener,
            context.clone(),
            req.thread_id,
            autonomy_mode,
            tmp.path().to_path_buf(),
        );
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
        (Some(mcp_config_path), Some(listener_task))
    };
    let mut cmd = build_command(
        &binary,
        &req.model_id,
        mcp_config_path.as_deref(),
        system_prompt_path.as_ref().map(|(path, _)| path.as_path()),
        &tmp,
        autonomy_mode,
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

    // Spawned before writing the prompt: an installed `claude` too old to
    // accept `bypassPermissions` can reject it and exit immediately, on its
    // own argument parser, before ever reading stdin — which surfaces as a
    // broken-pipe *write* failure below, not a read/EOF one. Both early-exit
    // shapes route through `classify_exit_error` so either is classified
    // the same way (tasks.md T009/T022, spec FR-013/SC-006).
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

    if let Err(error) = stdin.write_all(prompt.as_bytes()).await {
        return Err(classify_exit_error(
            &mut child,
            stderr_task,
            listener_task,
            autonomy_mode,
            &format!("failed to write Claude prompt: {error}"),
        )
        .await);
    }
    if let Err(error) = stdin.shutdown().await {
        return Err(classify_exit_error(
            &mut child,
            stderr_task,
            listener_task,
            autonomy_mode,
            &format!("failed to close Claude prompt input: {error}"),
        )
        .await);
    }
    // `shutdown()` alone does not release the underlying pipe fd — only
    // `Drop` does. The success path used to return (and thus drop `stdin`)
    // almost immediately after this point, so the child always saw EOF
    // quickly; the `Ungated` peek-ahead below now keeps this function
    // running longer, so `stdin` must be dropped explicitly or a child that
    // actually reads its prompt from stdin (any real `claude`, and any
    // stub written to behave like one) deadlocks waiting for EOF.
    drop(stdin);

    let mut reader = BufReader::new(stdout).lines();

    // Reactive classification (spec FR-013/SC-006, tasks.md T009/T022): a
    // slower-to-reject `claude` may accept the prompt write but still exit
    // before emitting any `stream-json` line. Unlike Codex's synchronous
    // `thread/start`, Claude has no separate handshake call to fail — the
    // only way to detect this before committing to the stream is to race
    // "first line" against "process already exited" once, here, before
    // returning. `GatedPermissive` sends the exact same flags as `Standard`
    // (only the approval bridge's decision function differs), so it can
    // never hit this path and is not worth the same peek — scoped to
    // `Ungated` only.
    let mut peeked_line: Option<String> = None;
    if autonomy_mode == AutonomyMode::Ungated {
        match tokio::time::timeout(STARTUP_PROBE_TIMEOUT, reader.next_line()).await {
            Ok(Ok(Some(line))) => peeked_line = Some(line),
            Ok(Ok(None)) => {
                return Err(classify_exit_error(
                    &mut child,
                    stderr_task,
                    listener_task,
                    autonomy_mode,
                    "claude exited without producing a result",
                )
                .await);
            }
            Ok(Err(e)) => {
                return Err(AdapterError::Http {
                    reason: format!("failed to read claude stdout: {e}"),
                })
            }
            // A silent `claude` that neither emits output nor exits would
            // otherwise hang this probe indefinitely — cancellation aside
            // (the caller can still drop this future), nothing else here
            // bounds the wait (code review).
            Err(_) => {
                return Err(classify_exit_error(
                    &mut child,
                    stderr_task,
                    listener_task,
                    autonomy_mode,
                    &format!(
                        "claude produced no output within {STARTUP_PROBE_TIMEOUT:?} of starting"
                    ),
                )
                .await);
            }
        }
    }

    let (tx, rx) = mpsc::unbounded_channel();
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let join = tokio::spawn(async move {
        // Keep `tmp` alive for the whole process lifetime; it is removed
        // when this task ends, on every path below.
        let _tmp = tmp;

        let start = Instant::now();
        let mut ttft_ms = None;
        let mut saw_result = false;
        let mut peeked_line = peeked_line;

        loop {
            // The line already consumed by the peek-ahead above (if any)
            // must be processed first, before reading any further.
            let line = if let Some(line) = peeked_line.take() {
                Ok(Some(line))
            } else {
                tokio::select! {
                    _ = task_cancellation.cancelled() => {
                        child.terminate_and_reap().await;
                        stderr_task.abort();
                        let _ = stderr_task.await;
                        if let Some(task) = listener_task {
                            task.abort();
                            let _ = task.await;
                        }
                        return;
                    }
                    line = reader.next_line() => line,
                }
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
                                if let Some(task) = listener_task {
                                    task.abort();
                                    let _ = task.await;
                                }
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
        if let Some(task) = listener_task {
            task.abort();
            let _ = task.await;
        }

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
