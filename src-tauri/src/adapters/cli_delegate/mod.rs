//! `cli_delegate` provider adapters (spec 007-cli-delegate) — Claude Code
//! (`claude -p`) and Codex (`codex app-server --stdio`) as chat backends
//! that reuse an existing subscription instead of a metered API key.
//!
//! Unlike [`super::anthropic::AnthropicAdapter`]/[`super::local`], a
//! [`CliDelegateAdapter`] drives an external CLI subprocess to
//! completion per [`super::ProviderAdapter::stream_chat`] call and
//! streams its answer back as plain [`super::StreamChunk::Delta`]/
//! [`super::StreamChunk::Done`] — it never emits `ToolCalls`, since the
//! delegate performs its own tool use internally (research.md §4). That
//! internal tool use still needs to cross holzi's *existing* approval
//! gate (`chat/tools/permission.rs`, `ChatState.pending_tool_approvals`)
//! so Manual/Auto/Plan apply identically to a delegate as to any other
//! backend — hence this module's otherwise-unusual dependency on
//! `crate::chat::tools::ApprovalDecision` (data-model.md
//! "approval_bridge.rs").

mod claude;
#[cfg(test)]
mod claude_tests;
mod codex;
#[cfg(test)]
mod codex_tests;
#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::{AdapterError, AdapterStream, ChatRequest, ChatRole, ProviderAdapter, ProviderModel};
use crate::chat::tools::ApprovalDecision;

/// Builds the plain-text transcript both `claude -p` and `codex`'s
/// `turn/start` receive as their conversation input. Holzi remains the
/// sole source of conversation history for either backend — neither
/// vendor's own session persistence has anything to resume under a
/// fresh, isolated `CLAUDE_CONFIG_DIR`/`CODEX_HOME` per invocation
/// (research.md §3), so relying on it would find nothing even if used.
pub(super) fn build_transcript_prompt(req: &ChatRequest) -> String {
    let mut prompt = String::new();
    for message in &req.messages {
        match &message.role {
            ChatRole::User => {
                prompt.push_str("Human: ");
                prompt.push_str(&message.content);
                prompt.push_str("\n\n");
            }
            ChatRole::Assistant => {
                prompt.push_str("Assistant: ");
                prompt.push_str(&message.content);
                prompt.push_str("\n\n");
            }
            // A prior turn in this thread may have been answered by a
            // different backend (spec.md Edge Cases: switching backends
            // mid-conversation) — summarize rather than silently drop.
            ChatRole::ToolCall { name, .. } => {
                prompt.push_str(&format!("[Assistant used a tool: {name}]\n\n"));
            }
            ChatRole::ToolResult { is_error, .. } => {
                let outcome = if *is_error { "failed" } else { "succeeded" };
                prompt.push_str(&format!("[Tool use {outcome}]\n\n"));
            }
        }
    }
    prompt
}

/// Shared shape of `ChatState.pending_tool_approvals` (`chat/session.rs`),
/// reused verbatim rather than re-declared so the type stays in lockstep
/// with the built-in tool loop's own map.
pub type PendingToolApprovals = Arc<Mutex<HashMap<Uuid, oneshot::Sender<ApprovalDecision>>>>;

/// Emits a named event with a JSON-serializable payload. In production
/// this is a thin wrapper around `AppHandle::emit` (`chat/model_loading.rs`
/// builds the real one); tests substitute a plain closure instead of
/// needing a live Tauri app — the same shape
/// `tests/common/tool_loop_fixture.rs` already uses for the built-in
/// tool loop's own event emission, kept consistent here rather than
/// introducing a second, `AppHandle`-only pattern this codebase's tests
/// have no way to drive.
pub type EventEmitter = Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>;

/// What `build_adapter` needs, beyond the provider row itself, to
/// construct a `CliDelegateAdapter` capable of real `stream_chat` (not
/// just `list_models`) — see `CliDelegateAdapter`'s own doc comment.
pub type DelegateChatContext = (PendingToolApprovals, EventEmitter);

/// Which subscription vendor a `cli_delegate` provider row talks to.
/// Parsed from `providers.adapter` (`storage/providers.rs`), mirroring
/// [`crate::storage::providers::ProviderKind`]'s own `parse`/`as_str`
/// pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegateVendor {
    Claude,
    Codex,
}

impl DelegateVendor {
    pub fn as_str(self) -> &'static str {
        match self {
            DelegateVendor::Claude => "claude",
            DelegateVendor::Codex => "codex",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(DelegateVendor::Claude),
            "codex" => Some(DelegateVendor::Codex),
            _ => None,
        }
    }
}

/// `ProviderAdapter` implementation for a `cli_delegate` provider row.
/// Constructed once per model load (`build_adapter`, `providers/mod.rs`)
/// and held as `Arc<dyn ProviderAdapter>` on the active session, exactly
/// like `AnthropicAdapter`/`LocalAdapter`.
///
/// `pending_tool_approvals`/`emit` are `None` when built from a context
/// that only ever calls `list_models` (the plain refresh/`do_refresh`
/// path, `providers/mod.rs`) rather than actually running a chat turn —
/// that path never needs the approval bridge. `stream_chat` (real chat
/// use) requires both `Some`; `build_adapter`'s chat-loading call site
/// (`chat/model_loading.rs::load_api_key_model`) always provides them
/// (tasks.md T005).
pub struct CliDelegateAdapter {
    vendor: DelegateVendor,
    /// Decrypted `providers.credentials`: a Claude OAuth token (UTF-8
    /// bytes) or a Codex `auth.json` file's raw bytes, depending on
    /// `vendor`.
    credentials: Vec<u8>,
    /// From `providers.base_url`; the `claude`/`codex` binary name or
    /// path to spawn. Falls back to `vendor.as_str()` when empty.
    binary: String,
    pending_tool_approvals: Option<PendingToolApprovals>,
    emit: Option<EventEmitter>,
}

impl CliDelegateAdapter {
    pub fn new(
        vendor: DelegateVendor,
        credentials: Vec<u8>,
        binary: String,
        pending_tool_approvals: Option<PendingToolApprovals>,
        emit: Option<EventEmitter>,
    ) -> Self {
        let binary = if binary.is_empty() {
            vendor.as_str().to_string()
        } else {
            binary
        };
        Self {
            vendor,
            credentials,
            binary,
            pending_tool_approvals,
            emit,
        }
    }
}

#[async_trait]
impl ProviderAdapter for CliDelegateAdapter {
    /// Delegates have no per-refresh model catalog — the one connected
    /// backend per vendor *is* the model (data-model.md). Mirrors
    /// `LocalAdapter::list_models`'s existing empty-`Vec` convention.
    async fn list_models(&self) -> Result<Vec<ProviderModel>, AdapterError> {
        Ok(vec![])
    }

    async fn stream_chat(&self, req: ChatRequest) -> Result<AdapterStream, AdapterError> {
        let (_pending, _emit) = self
            .pending_tool_approvals
            .as_ref()
            .zip(self.emit.as_ref())
            .ok_or_else(|| AdapterError::Http {
                reason: "cli_delegate adapter built without chat context (list_models-only \
                         construction)"
                    .into(),
            })?;
        match self.vendor {
            DelegateVendor::Claude => {
                claude::spawn_claude_invocation(self.binary.clone(), self.credentials.clone(), req)
                    .await
            }
            DelegateVendor::Codex => {
                codex::spawn_codex_app_server(self.binary.clone(), self.credentials.clone(), req)
                    .await
            }
        }
    }
}
