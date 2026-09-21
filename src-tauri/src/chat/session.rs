//! Active model session — the loaded provider adapter, its associated
//! catalog-side metadata, and the current in-flight generation (if any).
//!
//! At most one model is loaded at a time; loading a second model
//! unloads the first. This matches the plan's "one active generation
//! per device" constraint and the reality that a 7B GGUF plus a
//! second GGUF would fight over VRAM/RAM.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::adapters::{AbortHandle, ProviderAdapter};
use crate::chat::tools::cli::CliTool;
use crate::chat::tools::mcp::{self, McpServerConfig};
use crate::chat::tools::{ApprovalDecision, Tool, ToolRegistry};
use crate::storage::providers::ProviderKind;
use crate::vault_gate::ChildRegistry;

/// Structured lifecycle state shared by the Workspace and Chat views.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status")]
pub enum ModelLoadStatus {
    #[serde(rename = "idle")]
    Idle {
        #[serde(rename = "vaultGeneration")]
        vault_generation: u64,
    },
    #[serde(rename = "loading")]
    Loading {
        #[serde(rename = "vaultGeneration")]
        vault_generation: u64,
        #[serde(rename = "loadId")]
        load_id: u64,
        #[serde(rename = "modelId")]
        model_id: String,
        #[serde(rename = "modelName")]
        model_name: String,
        phase: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "providerName")]
        provider_name: Option<String>,
    },
    #[serde(rename = "ready")]
    Ready {
        #[serde(rename = "vaultGeneration")]
        vault_generation: u64,
        #[serde(rename = "loadId")]
        load_id: u64,
        #[serde(rename = "modelId")]
        model_id: String,
        #[serde(rename = "modelName")]
        model_name: String,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(rename = "vaultGeneration")]
        vault_generation: u64,
        #[serde(rename = "loadId")]
        load_id: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "modelId")]
        model_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "modelName")]
        model_name: Option<String>,
        code: String,
    },
}

struct ModelLoadRuntime {
    vault_generation: u64,
    next_load_id: u64,
    status: ModelLoadStatus,
    preload: Option<PreloadHandle>,
}

struct PreloadHandle {
    cancel: CancellationToken,
    join: tokio::task::JoinHandle<()>,
}

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
    /// Resolved once at load time so callers don't need a second DB round
    /// trip just to tell a `cli_delegate` session apart from an `api_key`
    /// one (both carry `provider_id: Some(_)`) — needed to scope
    /// `ChatRequest.autonomy_mode` to `cli_delegate` sessions only (code
    /// review: a non-delegate session must never carry a non-`Standard`
    /// autonomy label).
    pub provider_kind: ProviderKind,
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
/// `tool_cancellation` is the current turn's cancellation signal (T032):
/// `abort_current_generation` fires it alongside aborting
/// `current_generation`, so an in-flight `Tool::execute` or a pending
/// approval wait ends immediately instead of only the LLM stream.
pub struct ChatState {
    /// Held across a model load, vault transition, or entire accepted turn.
    operation: Arc<tokio::sync::Mutex<()>>,
    pub session: Arc<Mutex<Option<ActiveSession>>>,
    pub current_generation: Arc<Mutex<Option<AbortHandle>>>,
    pub tool_registry: Arc<Mutex<ToolRegistry>>,
    pub pending_tool_approvals: Arc<Mutex<HashMap<Uuid, oneshot::Sender<ApprovalDecision>>>>,
    /// Session-lifetime tombstones make late replies to cancelled prompts harmless.
    pub cancelled_tool_approvals: Arc<Mutex<HashSet<Uuid>>>,
    pub tool_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    model_load: Arc<Mutex<ModelLoadRuntime>>,
    /// Where the tools register the child processes they start, so ending the vault ends them.
    children: ChildRegistry,
}

impl Clone for ChatState {
    fn clone(&self) -> Self {
        Self {
            operation: Arc::clone(&self.operation),
            session: Arc::clone(&self.session),
            current_generation: Arc::clone(&self.current_generation),
            tool_registry: Arc::clone(&self.tool_registry),
            pending_tool_approvals: Arc::clone(&self.pending_tool_approvals),
            cancelled_tool_approvals: Arc::clone(&self.cancelled_tool_approvals),
            tool_cancellation: Arc::clone(&self.tool_cancellation),
            model_load: Arc::clone(&self.model_load),
            children: self.children.clone(),
        }
    }
}

impl ChatState {
    /// The host-CLI tool is registered unconditionally and immediately —
    /// unlike MCP tools it is never connection-dependent (T019).
    pub fn new() -> Self {
        Self::with_children(ChildRegistry::default())
    }

    /// Like [`ChatState::new`], with the tools registering their child processes in `children`,
    /// which is the registry of the vault gate.
    pub fn with_children(children: ChildRegistry) -> Self {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(CliTool::new(children.clone())));
        Self {
            operation: Arc::new(tokio::sync::Mutex::new(())),
            session: Arc::new(Mutex::new(None)),
            current_generation: Arc::new(Mutex::new(None)),
            tool_registry: Arc::new(Mutex::new(registry)),
            pending_tool_approvals: Arc::new(Mutex::new(HashMap::new())),
            cancelled_tool_approvals: Arc::new(Mutex::new(HashSet::new())),
            tool_cancellation: Arc::new(Mutex::new(None)),
            model_load: Arc::new(Mutex::new(ModelLoadRuntime {
                vault_generation: 0,
                next_load_id: 0,
                status: ModelLoadStatus::Idle {
                    vault_generation: 0,
                },
                preload: None,
            })),
            children,
        }
    }

    /// The registry the tools of this state register their child processes in.
    pub fn children(&self) -> &ChildRegistry {
        &self.children
    }

    /// Returns the current active-Vault generation.
    pub fn vault_generation(&self) -> u64 {
        self.model_load
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .vault_generation
    }

    /// Invalidates loads belonging to the previous Vault and resets status.
    pub fn bump_vault_generation(&self) -> u64 {
        let mut runtime = self.model_load.lock().unwrap_or_else(|e| e.into_inner());
        runtime.vault_generation = runtime.vault_generation.saturating_add(1);
        runtime.next_load_id = runtime.next_load_id.saturating_add(1);
        runtime.status = ModelLoadStatus::Idle {
            vault_generation: runtime.vault_generation,
        };
        runtime.vault_generation
    }

    /// Allocates a load id within the current Vault generation.
    pub fn begin_model_load(&self) -> (u64, u64) {
        let mut runtime = self.model_load.lock().unwrap_or_else(|e| e.into_inner());
        runtime.next_load_id = runtime.next_load_id.saturating_add(1);
        (runtime.vault_generation, runtime.next_load_id)
    }

    pub fn set_model_loading(
        &self,
        vault_generation: u64,
        load_id: u64,
        model_id: String,
        model_name: String,
        phase: String,
        provider_name: Option<String>,
    ) -> bool {
        let mut runtime = self.model_load.lock().unwrap_or_else(|e| e.into_inner());
        if runtime.vault_generation != vault_generation || runtime.next_load_id != load_id {
            return false;
        }
        runtime.status = ModelLoadStatus::Loading {
            vault_generation,
            load_id,
            model_id,
            model_name,
            phase,
            provider_name,
        };
        true
    }

    pub fn set_model_ready(
        &self,
        vault_generation: u64,
        load_id: u64,
        model_id: String,
        model_name: String,
    ) -> bool {
        let mut runtime = self.model_load.lock().unwrap_or_else(|e| e.into_inner());
        if runtime.vault_generation != vault_generation || runtime.next_load_id != load_id {
            return false;
        }
        runtime.status = ModelLoadStatus::Ready {
            vault_generation,
            load_id,
            model_id,
            model_name,
        };
        true
    }

    pub fn set_model_error(
        &self,
        vault_generation: u64,
        load_id: u64,
        model_id: Option<String>,
        model_name: Option<String>,
        code: String,
    ) -> bool {
        let mut runtime = self.model_load.lock().unwrap_or_else(|e| e.into_inner());
        if runtime.vault_generation != vault_generation || runtime.next_load_id != load_id {
            return false;
        }
        runtime.status = ModelLoadStatus::Error {
            vault_generation,
            load_id,
            model_id,
            model_name,
            code,
        };
        true
    }

    pub fn model_load_status(&self) -> ModelLoadStatus {
        self.model_load
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .status
            .clone()
    }

    pub fn is_current_load(&self, vault_generation: u64, load_id: u64) -> bool {
        let runtime = self.model_load.lock().unwrap_or_else(|e| e.into_inner());
        runtime.vault_generation == vault_generation && runtime.next_load_id == load_id
    }

    pub fn install_preload_handle(
        &self,
        cancel: CancellationToken,
        join: tokio::task::JoinHandle<()>,
    ) {
        self.model_load
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .preload = Some(PreloadHandle { cancel, join });
    }

    /// Fires the cancellation of an active preload without waiting for it. The close uses this:
    /// the gate's drain is what waits, within its own limit, so a preload that ignores the signal
    /// can never hold the close up.
    pub fn cancel_preload(&self) {
        if let Some(preload) = self
            .model_load
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .preload
            .as_ref()
        {
            preload.cancel.cancel();
        }
    }

    /// Cancels an active preload and waits for all of its child work to terminate.
    pub async fn cancel_preload_and_wait(&self) {
        let handle = self
            .model_load
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .preload
            .take();
        if let Some(handle) = handle {
            handle.cancel.cancel();
            let _ = handle.join.await;
            let mut runtime = self.model_load.lock().unwrap_or_else(|e| e.into_inner());
            runtime.next_load_id = runtime.next_load_id.saturating_add(1);
            runtime.status = ModelLoadStatus::Idle {
                vault_generation: runtime.vault_generation,
            };
        }
    }

    /// Empties everything a vault session can leave in this state, for the close (spec 013). The
    /// caller has already cancelled the turn through `abort_turn`; this only forgets what is
    /// left: the loaded model, the turn's abort handle and cancellation slot, both approval maps,
    /// and every tool a server contributed. The always-present host-CLI tool stays, as after
    /// [`ChatState::new`]. Dropping the session is what releases the database handle a delegate
    /// adapter holds, so the close calls this before it waits for the drain. Idempotent.
    pub fn reset_for_close(&self) {
        // Take each value out under its lock and drop it after the guard is gone, so no drop of
        // an adapter or tool ever runs while a lock is held.
        let session = self
            .session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let generation = self
            .current_generation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let cancellation = self
            .tool_cancellation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let pending = std::mem::take(
            &mut *self
                .pending_tool_approvals
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
        self.cancelled_tool_approvals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let mut host_only = ToolRegistry::new();
        host_only.register(Arc::new(CliTool::new(self.children.clone())));
        let tools = std::mem::replace(
            &mut *self.tool_registry.lock().unwrap_or_else(|e| e.into_inner()),
            host_only,
        );
        drop((session, generation, cancellation, pending, tools));
    }

    /// Reject overlapping operations before they can replace another turn's
    /// cancellation handles or carry a loaded adapter into a different vault.
    pub fn acquire_operation(&self) -> crate::error::Result<tokio::sync::OwnedMutexGuard<()>> {
        self.operation.clone().try_lock_owned().map_err(|_| {
            crate::error::HolziError::InvalidInput {
                reason: "a chat or vault operation is still in progress".into(),
            }
        })
    }

    /// Rebuilds the registry's MCP-sourced tools from a fresh `tools/list`
    /// against every server in `servers`, keeping the always-present
    /// host-CLI tool. Nothing calls this yet: configuring which MCP
    /// servers to connect to is explicitly out of scope for this feature
    /// (spec.md Assumptions) — a future settings surface would call this
    /// whenever the user's configured server list changes.
    pub async fn refresh_mcp_tools(&self, servers: &[McpServerConfig]) {
        let cli_name = CliTool::default().name().to_string();
        let discovered = mcp::discover_mcp_tools(
            servers,
            &std::collections::HashSet::from([cli_name]),
            &self.children,
        )
        .await;

        let mut registry = self.tool_registry.lock().unwrap_or_else(|e| e.into_inner());
        registry.clear();
        registry.register(Arc::new(CliTool::new(self.children.clone())));
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
