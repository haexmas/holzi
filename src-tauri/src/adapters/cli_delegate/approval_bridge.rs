use std::io;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::chat::tools::permission::{self, PermissionMode};
use crate::chat::tools::ApprovalDecision;

use super::{DelegateChatContext, EventEmitter, PendingToolApprovals};

const PREF_PERMISSION_MODE: &str = "chat.permission_mode";

#[derive(Debug, Deserialize)]
struct ApprovalRequest {
    tool_name: String,
    input: Value,
}

/// Applies holzi's existing permission posture and, for `Ask`, waits on the
/// same oneshot map used by the built-in tool loop.
pub async fn request_approval(
    pending: &PendingToolApprovals,
    emit: &EventEmitter,
    database: Option<&Arc<haex_crdt::Database>>,
    thread_id: Option<Uuid>,
    tool_name: String,
    input: Value,
) -> ApprovalDecision {
    let mode = match database {
        Some(database) => read_permission_mode(database).await,
        None => PermissionMode::Manual,
    };
    let risk = if is_risky_tool(&tool_name) {
        crate::chat::tools::RiskClass::Risky
    } else {
        crate::chat::tools::RiskClass::Safe
    };
    match permission::decide(mode, risk) {
        permission::Decision::Allow => ApprovalDecision::Allow,
        permission::Decision::Deny => ApprovalDecision::Deny,
        permission::Decision::Ask => {
            let request_id = Uuid::new_v4();
            let (sender, receiver) = oneshot::channel();
            pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(request_id, sender);
            emit(
                "tool-permission-request",
                json!({
                    "requestId": request_id,
                    "threadId": thread_id.unwrap_or_else(Uuid::nil),
                    "toolName": tool_name,
                    "toolInput": input,
                    "riskClass": match risk {
                        crate::chat::tools::RiskClass::Safe => "safe",
                        crate::chat::tools::RiskClass::Risky => "risky",
                    },
                }),
            );
            receiver.await.unwrap_or(ApprovalDecision::Deny)
        }
    }
}

async fn read_permission_mode(database: &Arc<haex_crdt::Database>) -> PermissionMode {
    let database = Arc::clone(database);
    let device_id = database.device_id();
    let raw = tauri::async_runtime::spawn_blocking(move || {
        database.with_connection(|conn| {
            crate::storage::preferences::get(
                conn,
                crate::storage::preferences::PrefScope::Device(device_id),
                PREF_PERMISSION_MODE,
            )
            .map_err(haex_crdt::Error::from)
        })
    })
    .await
    .ok()
    .and_then(|result| result.ok())
    .flatten();
    raw.as_deref()
        .and_then(PermissionMode::parse)
        .unwrap_or_default()
}

fn is_risky_tool(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    !matches!(
        name.as_str(),
        "read" | "glob" | "grep" | "ls" | "list" | "search" | "cat"
    )
}

#[cfg(unix)]
pub fn bind_socket(path: &std::path::Path) -> io::Result<tokio::net::UnixListener> {
    std::fs::remove_file(path).ok();
    tokio::net::UnixListener::bind(path)
}

#[cfg(unix)]
pub fn start_listener(
    listener: tokio::net::UnixListener,
    context: DelegateChatContext,
    thread_id: Option<Uuid>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let context = context.clone();
            tokio::spawn(async move {
                let (read_half, mut write_half) = stream.into_split();
                let mut lines = BufReader::new(read_half).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let Ok(request) = serde_json::from_str::<ApprovalRequest>(&line) else {
                        break;
                    };
                    let decision = request_approval(
                        &context.pending_tool_approvals,
                        &context.emit,
                        context.database.as_ref(),
                        thread_id,
                        request.tool_name,
                        request.input,
                    )
                    .await;
                    let value = json!({
                        "decision": match decision {
                            ApprovalDecision::Allow => "allow",
                            ApprovalDecision::Deny => "deny",
                        }
                    });
                    let Ok(mut bytes) = serde_json::to_vec(&value) else {
                        break;
                    };
                    bytes.push(b'\n');
                    if write_half.write_all(&bytes).await.is_err() {
                        break;
                    }
                }
            });
        }
    })
}

#[cfg(not(unix))]
pub fn bind_socket(_path: &std::path::Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "CLI delegate approval sockets are not implemented on this target",
    ))
}

#[cfg(not(unix))]
pub fn start_listener(
    _listener: (),
    _context: DelegateChatContext,
    _thread_id: Option<Uuid>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {})
}
