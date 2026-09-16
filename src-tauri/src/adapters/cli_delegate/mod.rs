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

// `claude`/`codex` submodules land in tasks.md T013/T014; `mod_tests`
// lands in T007.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::AppHandle;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::chat::tools::ApprovalDecision;

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

/// One approval request the delegate's own tool use raised, awaiting a
/// live decision through holzi's approval gate (data-model.md
/// "approval_bridge.rs"). Shared shape for both vendors — Claude Code's
/// MCP permission-prompt-tool bridge and Codex's `ServerRequest`
/// approval handling both resolve into this before crossing into
/// `pending_tool_approvals`.
pub struct PendingDelegateApproval {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
}

/// `ProviderAdapter` implementation for a `cli_delegate` provider row.
/// Constructed once per model load (`build_adapter`, `providers/mod.rs`)
/// and held as `Arc<dyn ProviderAdapter>` on the active session, exactly
/// like `AnthropicAdapter`/`LocalAdapter`.
///
/// Fields are populated by `build_adapter`'s `CliDelegate` branch — see
/// tasks.md T006 (construction) and T005 (threading `pending_tool_approvals`/
/// `app` through from `load_model_inner`).
pub struct CliDelegateAdapter {
    vendor: DelegateVendor,
    /// Decrypted `providers.credentials`: a Claude OAuth token (UTF-8
    /// bytes) or a Codex `auth.json` file's raw bytes, depending on
    /// `vendor`.
    credentials: Vec<u8>,
    /// From `providers.base_url`; the `claude`/`codex` binary name or
    /// path to spawn. Falls back to `vendor.as_str()` when empty.
    binary: String,
    /// Same `ChatState.pending_tool_approvals` map the built-in tool
    /// loop already uses (`chat/session.rs`) — threaded through so a
    /// delegate's own tool use can raise a live `tool-permission-request`
    /// through the identical gate, not a parallel mechanism.
    pending_tool_approvals: Arc<Mutex<HashMap<Uuid, oneshot::Sender<ApprovalDecision>>>>,
    /// Needed to emit `tool-permission-request` (`events.rs`) while
    /// `stream_chat` is still running, before it has returned anything
    /// to the turn loop.
    app_handle: AppHandle,
}

impl CliDelegateAdapter {
    pub fn new(
        vendor: DelegateVendor,
        credentials: Vec<u8>,
        binary: String,
        pending_tool_approvals: Arc<Mutex<HashMap<Uuid, oneshot::Sender<ApprovalDecision>>>>,
        app_handle: AppHandle,
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
