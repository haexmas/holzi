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

// `claude`/`codex` submodules land in tasks.md T013/T014.
#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tauri::AppHandle;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::{AdapterError, AdapterStream, ChatRequest, ProviderAdapter, ProviderModel};
use crate::chat::tools::ApprovalDecision;

/// Shared shape of `ChatState.pending_tool_approvals` (`chat/session.rs`),
/// reused verbatim rather than re-declared so the type stays in lockstep
/// with the built-in tool loop's own map.
pub type PendingToolApprovals = Arc<Mutex<HashMap<Uuid, oneshot::Sender<ApprovalDecision>>>>;

/// What `build_adapter` needs, beyond the provider row itself, to
/// construct a `CliDelegateAdapter` capable of real `stream_chat` (not
/// just `list_models`) — see `CliDelegateAdapter`'s own doc comment.
pub type DelegateChatContext = (PendingToolApprovals, AppHandle);

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
/// `pending_tool_approvals`/`app_handle` are `None` when built from a
/// context that only ever calls `list_models` (the plain
/// refresh/`do_refresh` path, `providers/mod.rs`) rather than actually
/// running a chat turn — that path never needs the approval bridge.
/// `stream_chat` (real chat use) requires both `Some`; `build_adapter`'s
/// chat-loading call site (`chat/model_loading.rs::load_api_key_model`)
/// always provides them (tasks.md T005).
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
    app_handle: Option<AppHandle>,
}

impl CliDelegateAdapter {
    pub fn new(
        vendor: DelegateVendor,
        credentials: Vec<u8>,
        binary: String,
        pending_tool_approvals: Option<PendingToolApprovals>,
        app_handle: Option<AppHandle>,
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
            app_handle,
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

    /// Real dispatch to `claude.rs`/`codex.rs` lands in tasks.md T013-T016;
    /// until then this is a placeholder so `CliDelegateAdapter` can be
    /// constructed (T006) without every downstream task landing at once.
    async fn stream_chat(&self, _req: ChatRequest) -> Result<AdapterStream, AdapterError> {
        let (_pending, _app) = self
            .pending_tool_approvals
            .as_ref()
            .zip(self.app_handle.as_ref())
            .ok_or_else(|| AdapterError::Http {
                reason: "cli_delegate adapter built without chat context (list_models-only \
                         construction)"
                    .into(),
            })?;
        Err(AdapterError::Http {
            reason: format!(
                "cli_delegate ({}) stream_chat not yet implemented — tasks.md T013-T016",
                self.vendor.as_str()
            ),
        })
    }
}
