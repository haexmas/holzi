//! Tool trait, registry, and the Manual/Auto/Plan approval gate.
//!
//! A [`Tool`] is anything the turn loop (`chat/commands.rs`) can hand to an
//! adapter as a [`crate::adapters::types::ToolSpec`] and later execute once
//! the model asks for it. The registry holds every tool from every source
//! (host-CLI, MCP) behind one `Arc<dyn Tool>` so the loop never branches on
//! where a tool came from.

pub mod cli;
#[cfg(test)]
mod cli_tests;
pub mod mcp;
#[cfg(test)]
mod mcp_tests;
pub mod permission;
#[cfg(test)]
mod permission_tests;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

/// Whether a tool call may run without confirmation under `auto` mode.
/// See `permission::decide` for the full Manual/Auto/Plan matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskClass {
    Safe,
    Risky,
}

/// Outcome of [`Tool::execute`]. Never a `Result` — a failed tool call is a
/// normal conversational outcome (spec.md: the model sees the error and can
/// react to it), not a turn-ending error.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// One callable tool, regardless of source (host-CLI, MCP). `execute` takes
/// the already-parsed JSON input the model produced and never panics —
/// execution failures (bad input, a crashed process, an unreachable MCP
/// server) come back as `ToolResult { is_error: true, .. }`.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique within the registry for a turn. For a disambiguated MCP tool
    /// this is the prefixed `registry_name` (data-model.md), not the raw
    /// name the server reported.
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// `mcp` or `cli` — persisted verbatim into `chat_messages.tool_source`
    /// (data-model.md).
    fn source(&self) -> &'static str;
    /// JSON-Schema-shaped object, reused as-is for both the Anthropic
    /// `input_schema` and mistralrs' `Function.parameters` (research.md
    /// §1/§2 — both are JSON-Schema-shaped).
    fn input_schema(&self) -> Value;
    fn risk_class(&self) -> RiskClass;
    async fn execute(&self, input: Value) -> ToolResult;
}

/// Every tool available this turn, from every source. Built once at
/// `ChatState` construction (host-CLI) and refreshed as MCP servers
/// (re)connect (T019). Tools are `Arc`-shared rather than owned outright so
/// the turn loop can clone one out and run its (possibly slow) `execute()`
/// without holding `ChatState.tool_registry`'s lock for the duration.
#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Registers a tool. If `name()` collides with an already-registered
    /// tool, the newly-registered one wins the slot under its own name.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        if let Some(existing) = self.tools.iter_mut().find(|existing| existing.name() == tool.name()) {
            *existing = tool;
        } else {
            self.tools.push(tool);
        }
    }

    /// Drops every currently-registered tool. Used before an MCP
    /// reconnect repopulates the registry (T019).
    pub fn clear(&mut self) {
        self.tools.clear();
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name).cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Tool>> {
        self.tools.iter()
    }
}

/// Resolution of one `tool-permission-request` (contracts/tauri-commands.md
/// §respond_tool_permission). Distinct from `permission::Decision`, which is
/// the gate's own Allow/Ask/Deny verdict before a human is ever asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny,
}
