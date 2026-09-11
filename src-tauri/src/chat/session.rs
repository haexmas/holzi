//! Active model session — the loaded provider adapter, its associated
//! catalog-side metadata, and the current in-flight generation (if any).
//!
//! At most one model is loaded at a time; loading a second model
//! unloads the first. This matches the plan's "one active generation
//! per device" constraint and the reality that a 7B GGUF plus a
//! second GGUF would fight over VRAM/RAM.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use uuid::Uuid;

use crate::adapters::ProviderAdapter;
use crate::chat::tools::cli::CliTool;
use crate::chat::tools::mcp::{self, McpServerConfig};
use crate::chat::tools::{ApprovalDecision, Tool, ToolRegistry};

/// Metadata about the currently-loaded model. Both the local and
/// api_key paths funnel through the same `Arc<dyn ProviderAdapter>`
/// so `send_message` never branches on model kind.
#[derive(Clone)]
pub struct ActiveSession {
    /// Composite id (`<provider_id>:<remote_id>`) for api_key models,
    /// or the catalog id for local ones. Persisted verbatim as the
    /// `chat_messages.model_id` foreign key.
    pub model_id: String,
    /// Provider UUID for API-key sessions; `None` for local sessions.
    pub provider_id: Option<Uuid>,
    pub adapter: Arc<dyn ProviderAdapter>,
    /// Only meaningful for local models; api_key rows carry an empty
    /// string because the vendor's server-side tokenizer handles it.
    pub tokenizer_repo: String,
    pub context_window: Option<i64>,
}

/// Tauri-managed state for the chat runtime. `session` is the loaded
/// model; `current_generation` is an abort handle for the in-flight
/// streaming task, if any. Both are optional — an idle app has neither.
/// `tool_registry` holds every callable tool (host-CLI + MCP);
/// `pending_tool_approvals` holds one `oneshot::Sender` per open
/// `tool-permission-request`, resolved by `respond_tool_permission` or
/// dropped on cancellation (data-model.md `PendingToolApproval`).
pub struct ChatState {
    pub session: Mutex<Option<ActiveSession>>,
    pub current_generation: Mutex<Option<tokio::task::AbortHandle>>,
    pub tool_registry: Mutex<ToolRegistry>,
    pub pending_tool_approvals: Mutex<HashMap<Uuid, oneshot::Sender<ApprovalDecision>>>,
}

impl ChatState {
    /// The host-CLI tool is registered unconditionally and immediately —
    /// unlike MCP tools it is never connection-dependent (T019).
    pub fn new() -> Self {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(CliTool));
        Self {
            session: Mutex::new(None),
            current_generation: Mutex::new(None),
            tool_registry: Mutex::new(registry),
            pending_tool_approvals: Mutex::new(HashMap::new()),
        }
    }

    /// Rebuilds the registry's MCP-sourced tools from a fresh `tools/list`
    /// against every server in `servers`, keeping the always-present
    /// host-CLI tool. Nothing calls this yet: configuring which MCP
    /// servers to connect to is explicitly out of scope for this feature
    /// (spec.md Assumptions) — a future settings surface would call this
    /// whenever the user's configured server list changes.
    pub async fn refresh_mcp_tools(&self, servers: &[McpServerConfig]) {
        let cli_name = CliTool.name().to_string();
        let discovered = mcp::discover_mcp_tools(servers, &std::collections::HashSet::from([cli_name])).await;

        let mut registry = self
            .tool_registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        registry.clear();
        registry.register(Arc::new(CliTool));
        for tool in discovered {
            registry.register(tool);
        }
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}
