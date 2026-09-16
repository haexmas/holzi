//! `codex app-server --stdio` process driver (tasks.md T014, T036).
//!
//! Speaks Codex's own JSON-RPC protocol directly, verified live against
//! an installed CLI (research.md §2: `initialize` → `thread/start` →
//! `turn/start`, streaming `item/agentMessage/delta` notifications,
//! ending on `turn/completed`). Unlike the Claude Code path
//! (`claude.rs`), no separate approval-bridge child process or IPC hop
//! is needed — holzi owns this process's stdio pipes directly, so an
//! approval request (`item/commandExecution/requestApproval` and
//! friends) arrives as an ordinary server-to-client JSON-RPC request on
//! the same stream `initialize`'s response came in on.
//!
//! Phase 3 (US1) scope only: [`respond_to_server_request`] answers every
//! incoming approval request with a well-formed, safe fail-closed
//! `{"decision":"decline"}` — a stub, not real bridging. Phase 5 (US3,
//! tasks.md T036) replaces that one function's body with
//! `approval_bridge::request_approval`.

use std::io;
use std::process::Stdio;
use std::time::Instant;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;

use crate::adapters::types::{StreamChunk, StreamError};
use crate::adapters::{AdapterError, AdapterStream, ChatRequest};

use super::build_transcript_prompt;

const CLIENT_NAME: &str = "holzi";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn build_command(binary: &str, tmp: &TempDir) -> Command {
    let mut cmd = Command::new(binary);
    cmd.arg("app-server").arg("--stdio");
    cmd.env("CODEX_HOME", tmp.path())
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

/// Pure request-envelope builder — unit-tested directly (tasks.md T012)
/// against the `ClientRequest` shape confirmed via `codex app-server
/// generate-json-schema` (research.md §2): `{"jsonrpc":"2.0","id":...,
/// "method":...,"params":...}`.
pub(super) fn build_request(id: i64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

async fn write_message(stdin: &mut ChildStdin, value: &Value) -> io::Result<()> {
    let mut line = serde_json::to_vec(value).expect("Value always serializes");
    line.push(b'\n');
    stdin.write_all(&line).await
}

/// One line read from the app-server, categorized.
#[derive(Debug, PartialEq)]
pub(super) enum Line {
    /// A response to a request holzi sent (`id` + `result`/`error`).
    Response {
        id: i64,
        value: Value,
    },
    /// A server-initiated request needing a reply (`id` + `method`).
    /// `method`/`params` are unused until tasks.md T036 makes the
    /// response method-specific instead of always declining.
    #[allow(dead_code)]
    ServerRequest {
        id: Value,
        method: String,
        params: Value,
    },
    /// A notification (`method`, no `id`).
    Notification {
        method: String,
        params: Value,
    },
    /// A line that wasn't valid JSON, or had none of the shapes above.
    Unrecognized,
    Eof,
}

/// Pure JSON-RPC line classifier — no I/O, so it's directly
/// unit-testable (tasks.md T012) against the wire shapes verified live
/// in research.md §2, without spawning a real `codex` process.
pub(super) fn categorize_line(line: Option<&str>) -> Line {
    let Some(line) = line else {
        return Line::Eof;
    };
    if line.trim().is_empty() {
        return Line::Unrecognized;
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Line::Unrecognized;
    };
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .map(str::to_string);
    match (value.get("id"), method) {
        (Some(id), Some(method)) => Line::ServerRequest {
            id: id.clone(),
            method,
            params: value.get("params").cloned().unwrap_or(Value::Null),
        },
        (None, Some(method)) => Line::Notification {
            method,
            params: value.get("params").cloned().unwrap_or(Value::Null),
        },
        (Some(id), None) => match id.as_i64() {
            Some(id) => Line::Response { id, value },
            None => Line::Unrecognized,
        },
        (None, None) => Line::Unrecognized,
    }
}

async fn read_line(lines: &mut Lines<BufReader<ChildStdout>>) -> io::Result<Line> {
    let line = lines.next_line().await?;
    Ok(categorize_line(line.as_deref()))
}

/// Phase 3 (US1) fail-closed default for any approval-shaped server
/// request. `decline` is a real, documented `CommandExecutionApprovalDecision`
/// value ("user denied the command, the agent will continue the turn") —
/// a well-formed denial, not the malformed-response accident research.md
/// §2 found (which the server also treated as a rejection, but by
/// deserialization failure rather than deliberately).
async fn respond_to_server_request(stdin: &mut ChildStdin, id: Value) -> io::Result<()> {
    write_message(
        stdin,
        &json!({"jsonrpc": "2.0", "id": id, "result": {"decision": "decline"}}),
    )
    .await
}

/// Sends `method`/`params` as a request with a fresh id and waits for
/// its matching response, transparently answering any server requests
/// and ignoring notifications encountered while waiting — mirrors how
/// holzi's research.md §2 spike drove `initialize`/`thread/start`/
/// `turn/start` over the same interleaved stream.
async fn call(
    stdin: &mut ChildStdin,
    lines: &mut Lines<BufReader<ChildStdout>>,
    next_id: &mut i64,
    method: &str,
    params: Value,
) -> Result<Value, AdapterError> {
    let id = *next_id;
    *next_id += 1;
    write_message(stdin, &build_request(id, method, params))
        .await
        .map_err(|e| AdapterError::Http {
            reason: format!("failed to write {method} request: {e}"),
        })?;
    loop {
        match read_line(lines).await.map_err(|e| AdapterError::Http {
            reason: format!("failed to read app-server response to {method}: {e}"),
        })? {
            Line::Response { id: rid, value } if rid == id => {
                if let Some(error) = value.get("error") {
                    return Err(AdapterError::Http {
                        reason: format!("codex {method} failed: {error}"),
                    });
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            Line::ServerRequest { id: sid, .. } => {
                let _ = respond_to_server_request(stdin, sid).await;
            }
            Line::Eof => {
                return Err(AdapterError::Http {
                    reason: format!("codex app-server exited before responding to {method}"),
                });
            }
            _ => {}
        }
    }
}

pub(super) async fn spawn_codex_app_server(
    binary: String,
    credentials: Vec<u8>,
    req: ChatRequest,
) -> Result<AdapterStream, AdapterError> {
    if credentials.is_empty() {
        return Err(AdapterError::InvalidCredentials);
    }

    // Disposable per-invocation CODEX_HOME + cwd (spec 007-cli-delegate
    // FR-002/FR-003), pre-populated with the vault's stored `auth.json` —
    // removed when `tmp` drops at the end of the spawned task below.
    let tmp = TempDir::new().map_err(|e| AdapterError::Http {
        reason: format!("failed to create temp dir for codex invocation: {e}"),
    })?;
    std::fs::write(tmp.path().join("auth.json"), &credentials).map_err(|e| AdapterError::Http {
        reason: format!("failed to write codex auth.json: {e}"),
    })?;

    let mut child = build_command(&binary, &tmp)
        .spawn()
        .map_err(|e| AdapterError::Http {
            reason: format!("failed to spawn \"{binary}\": {e}"),
        })?;
    let mut stdin = child.stdin.take().ok_or_else(|| AdapterError::Http {
        reason: "failed to capture codex stdin".into(),
    })?;
    let stdout = child.stdout.take().ok_or_else(|| AdapterError::Http {
        reason: "failed to capture codex stdout".into(),
    })?;
    let stderr: ChildStderr = child.stderr.take().ok_or_else(|| AdapterError::Http {
        reason: "failed to capture codex stderr".into(),
    })?;
    let mut lines = BufReader::new(stdout).lines();

    let mut next_id: i64 = 1;
    call(
        &mut stdin,
        &mut lines,
        &mut next_id,
        "initialize",
        json!({"clientInfo": {"name": CLIENT_NAME, "version": CLIENT_VERSION}}),
    )
    .await?;

    let thread_result = call(
        &mut stdin,
        &mut lines,
        &mut next_id,
        "thread/start",
        json!({
            "cwd": tmp.path().to_string_lossy(),
            // Explicit even though "user" is already the documented
            // default (research.md §2) — removes any doubt from a
            // possibly customized config, the same host-leakage concern
            // already found on the Claude Code side (research.md §3).
            "approvalPolicy": "on-request",
            "approvalsReviewer": "user",
        }),
    )
    .await?;
    let thread_id = thread_result
        .get("thread")
        .and_then(|t| t.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| AdapterError::Http {
            reason: "codex thread/start response missing thread.id".into(),
        })?
        .to_string();

    let prompt = build_transcript_prompt(&req);
    call(
        &mut stdin,
        &mut lines,
        &mut next_id,
        "turn/start",
        json!({
            "threadId": thread_id,
            "input": [{"type": "text", "text": prompt}],
        }),
    )
    .await?;

    let (tx, rx) = mpsc::unbounded_channel();
    let join = tokio::spawn(async move {
        let _tmp = tmp;
        let _stderr = stderr; // drained implicitly by drop; diagnostics only matter on failure

        let start = Instant::now();
        let mut ttft_ms = None;
        let mut last_usage: Option<(Option<usize>, Option<usize>)> = None;

        loop {
            match read_line(&mut lines).await {
                Ok(Line::ServerRequest { id, .. }) => {
                    let _ = respond_to_server_request(&mut stdin, id).await;
                }
                Ok(Line::Notification { method, params }) => match method.as_str() {
                    "item/agentMessage/delta" => {
                        let Some(delta) = params.get("delta").and_then(Value::as_str) else {
                            continue;
                        };
                        if ttft_ms.is_none() {
                            ttft_ms = Some(start.elapsed().as_millis() as u64);
                        }
                        if tx
                            .send(Ok(StreamChunk::Delta {
                                content: delta.to_string(),
                                reasoning: None,
                            }))
                            .is_err()
                        {
                            let _ = child.start_kill();
                            return;
                        }
                    }
                    "thread/tokenUsage/updated" => {
                        let usage = params.get("tokenUsage").and_then(|u| u.get("last"));
                        last_usage = Some((
                            usage
                                .and_then(|u| u.get("inputTokens"))
                                .and_then(Value::as_u64)
                                .map(|v| v as usize),
                            usage
                                .and_then(|u| u.get("outputTokens"))
                                .and_then(Value::as_u64)
                                .map(|v| v as usize),
                        ));
                    }
                    "turn/completed" => {
                        let (prompt_tokens, completion_tokens) = last_usage.unwrap_or((None, None));
                        let _ = tx.send(Ok(StreamChunk::Done {
                            finish_reason: Some("complete".to_string()),
                            prompt_tokens,
                            completion_tokens,
                            ttft_ms,
                            total_ms: start.elapsed().as_millis() as u64,
                        }));
                        break;
                    }
                    "turn/failed" => {
                        let message = params
                            .get("error")
                            .and_then(Value::as_str)
                            .unwrap_or("codex turn failed")
                            .to_string();
                        let _ = tx.send(Err(StreamError::Model(message)));
                        break;
                    }
                    _ => {}
                },
                Ok(Line::Eof) => {
                    let _ = tx.send(Err(StreamError::UnexpectedEnd));
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    let _ = tx.send(Err(StreamError::Internal(format!(
                        "failed to read codex stdout: {e}"
                    ))));
                    break;
                }
            }
        }

        let _ = child.wait().await;
    });

    Ok(AdapterStream::new(rx, join.abort_handle()))
}
