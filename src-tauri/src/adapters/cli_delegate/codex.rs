//! `codex app-server --stdio` process driver.

use std::io;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::adapters::types::{StreamChunk, StreamError};
use crate::adapters::{AdapterError, AdapterStream, ChatRequest};

use super::approval_bridge;
use super::process::{configure_process_group, ChildLifecycle};
use super::{build_transcript_prompt, DelegateChatContext};

const CLIENT_NAME: &str = "holzi";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_STDERR_BYTES: usize = 64 * 1024;

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

pub(super) fn build_request(id: i64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

async fn write_message(stdin: &mut ChildStdin, value: &Value) -> io::Result<()> {
    let mut line = serde_json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    line.push(b'\n');
    stdin.write_all(&line).await
}

#[derive(Debug, PartialEq)]
pub(super) enum Line {
    Response {
        id: i64,
        value: Value,
    },
    ServerRequest {
        id: Value,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    Unrecognized,
    Eof,
}

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
    Ok(categorize_line(lines.next_line().await?.as_deref()))
}

async fn respond_to_server_request(
    stdin: &mut ChildStdin,
    id: Value,
    method: &str,
    params: Value,
    context: &DelegateChatContext,
    thread_id: Option<uuid::Uuid>,
) -> io::Result<()> {
    let tool_name = match method {
        "item/commandExecution/requestApproval" => "Bash",
        "item/fileChange/requestApproval" => "Edit",
        "item/permissions/requestApproval" => "Permissions",
        _ => "Codex tool",
    };
    let decision = approval_bridge::request_approval(
        &context.pending_tool_approvals,
        &context.emit,
        context.database.as_ref(),
        thread_id,
        tool_name.to_string(),
        params,
    )
    .await;
    let decision = match decision {
        crate::chat::tools::ApprovalDecision::Allow => "accept",
        crate::chat::tools::ApprovalDecision::Deny => "decline",
    };
    write_message(
        stdin,
        &json!({"jsonrpc": "2.0", "id": id, "result": {"decision": decision}}),
    )
    .await
}

async fn call(
    stdin: &mut ChildStdin,
    lines: &mut Lines<BufReader<ChildStdout>>,
    next_id: &mut i64,
    method: &str,
    params: Value,
    context: &DelegateChatContext,
    thread_id: Option<uuid::Uuid>,
) -> Result<Value, AdapterError> {
    let id = *next_id;
    *next_id += 1;
    write_message(stdin, &build_request(id, method, params))
        .await
        .map_err(|error| AdapterError::Http {
            reason: format!("failed to write {method} request: {error}"),
        })?;
    loop {
        match read_line(lines).await.map_err(|error| AdapterError::Http {
            reason: format!("failed to read app-server response to {method}: {error}"),
        })? {
            Line::Response {
                id: response_id,
                value,
            } if response_id == id => {
                if let Some(error) = value.get("error") {
                    return Err(AdapterError::Http {
                        reason: format!("codex {method} failed: {error}"),
                    });
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            Line::ServerRequest { id, method, params } => {
                respond_to_server_request(stdin, id, &method, params, context, thread_id)
                    .await
                    .map_err(|error| AdapterError::Http {
                        reason: format!("failed to answer Codex approval request: {error}"),
                    })?;
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

async fn call_with_timeout(
    stdin: &mut ChildStdin,
    lines: &mut Lines<BufReader<ChildStdout>>,
    next_id: &mut i64,
    method: &str,
    params: Value,
    context: &DelegateChatContext,
    thread_id: Option<uuid::Uuid>,
) -> Result<Value, AdapterError> {
    tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        call(stdin, lines, next_id, method, params, context, thread_id),
    )
    .await
    .map_err(|_| AdapterError::Http {
        reason: format!("codex {method} handshake timed out after {HANDSHAKE_TIMEOUT:?}"),
    })?
}

async fn read_limited<R: AsyncRead + Unpin>(mut reader: R) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    while let Ok(read) = reader.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        let remaining = MAX_STDERR_BYTES.saturating_sub(output.len());
        if remaining > 0 {
            output.extend_from_slice(&buffer[..read.min(remaining)]);
        }
    }
    output
}

/// Starts one isolated Codex app-server session and exposes its output as an adapter stream.
pub(super) async fn spawn_codex_app_server(
    binary: String,
    credentials: Vec<u8>,
    req: ChatRequest,
    context: DelegateChatContext,
) -> Result<AdapterStream, AdapterError> {
    if credentials.is_empty() {
        return Err(AdapterError::InvalidCredentials);
    }
    let tmp = TempDir::new().map_err(|error| AdapterError::Http {
        reason: format!("failed to create temp dir for codex invocation: {error}"),
    })?;
    std::fs::write(tmp.path().join("auth.json"), &credentials).map_err(|error| {
        AdapterError::Http {
            reason: format!("failed to write codex auth.json: {error}"),
        }
    })?;

    let mut command = build_command(&binary, &tmp);
    configure_process_group(&mut command);
    let mut child = ChildLifecycle::spawn(&mut command).map_err(|error| AdapterError::Http {
        reason: format!("failed to spawn \"{binary}\": {error}"),
    })?;
    let mut stdin = child
        .child_mut()
        .stdin
        .take()
        .ok_or_else(|| AdapterError::Http {
            reason: "failed to capture codex stdin".into(),
        })?;
    let stdout = child
        .child_mut()
        .stdout
        .take()
        .ok_or_else(|| AdapterError::Http {
            reason: "failed to capture codex stdout".into(),
        })?;
    let stderr = child
        .child_mut()
        .stderr
        .take()
        .ok_or_else(|| AdapterError::Http {
            reason: "failed to capture codex stderr".into(),
        })?;
    let stderr_task = tokio::spawn(read_limited(stderr));
    let mut lines = BufReader::new(stdout).lines();
    let mut next_id = 1_i64;
    let original_thread_id = req.thread_id;

    let handshake = async {
        call_with_timeout(
            &mut stdin,
            &mut lines,
            &mut next_id,
            "initialize",
            json!({"clientInfo": {"name": CLIENT_NAME, "version": CLIENT_VERSION}}),
            &context,
            original_thread_id,
        )
        .await?;
        let thread_result = call_with_timeout(
            &mut stdin,
            &mut lines,
            &mut next_id,
            "thread/start",
            json!({
                "cwd": tmp.path().to_string_lossy(),
                "approvalPolicy": "on-request",
                "approvalsReviewer": "user",
            }),
            &context,
            original_thread_id,
        )
        .await?;
        let codex_thread_id = thread_result
            .get("thread")
            .and_then(|thread| thread.get("id"))
            .and_then(Value::as_str)
            .ok_or_else(|| AdapterError::Http {
                reason: "codex thread/start response missing thread.id".into(),
            })?
            .to_string();
        let prompt = build_transcript_prompt(&req);
        call_with_timeout(
            &mut stdin,
            &mut lines,
            &mut next_id,
            "turn/start",
            json!({
                "threadId": codex_thread_id,
                "input": [{"type": "text", "text": prompt}],
            }),
            &context,
            original_thread_id,
        )
        .await
    };
    if let Err(error) = handshake.await {
        child.terminate_and_reap().await;
        stderr_task.abort();
        let _ = stderr_task.await;
        return Err(error);
    }

    let (tx, rx) = mpsc::unbounded_channel();
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let join = tokio::spawn(async move {
        let _tmp = tmp;
        let start = Instant::now();
        let mut ttft_ms = None;
        let mut last_usage: Option<(Option<usize>, Option<usize>)> = None;
        loop {
            let line = tokio::select! {
                _ = task_cancellation.cancelled() => {
                    child.terminate_and_reap().await;
                    stderr_task.abort();
                    let _ = stderr_task.await;
                    return;
                }
                line = read_line(&mut lines) => line,
            };
            match line {
                Ok(Line::ServerRequest { id, method, params }) => {
                    let _ = respond_to_server_request(
                        &mut stdin,
                        id,
                        &method,
                        params,
                        &context,
                        original_thread_id,
                    )
                    .await;
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
                            child.terminate_and_reap().await;
                            stderr_task.abort();
                            let _ = stderr_task.await;
                            return;
                        }
                    }
                    "thread/tokenUsage/updated" => {
                        let usage = params.get("tokenUsage").and_then(|value| value.get("last"));
                        last_usage = Some((
                            usage
                                .and_then(|value| value.get("inputTokens"))
                                .and_then(Value::as_u64)
                                .map(|value| value as usize),
                            usage
                                .and_then(|value| value.get("outputTokens"))
                                .and_then(Value::as_u64)
                                .map(|value| value as usize),
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
                Err(error) => {
                    let _ = tx.send(Err(StreamError::Internal(format!(
                        "failed to read codex stdout: {error}"
                    ))));
                    break;
                }
            }
        }
        // `app-server` is persistent by design; a completed turn does not
        // imply that the process exits. End this one-shot adapter stream by
        // terminating and reaping the whole process group before draining
        // the bounded stderr task.
        child.terminate_and_reap().await;
        let _stderr_bytes = stderr_task.await.unwrap_or_default();
    });

    Ok(AdapterStream::new_with_cancellation(
        rx,
        join.abort_handle(),
        cancellation,
    ))
}
