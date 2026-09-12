//! MCP-client tool discovery and execution, via `rmcp` (research.md §3).
//!
//! Configuring which MCP servers to connect to is explicitly out of scope
//! for this feature (spec.md Assumptions: "configuring those servers
//! themselves is not part of this feature") — [`McpServerConfig`] is the
//! shape a future settings surface would produce. Until that surface
//! exists, [`discover_mcp_tools`] is simply called with an empty list
//! (`lib.rs`), which is a valid, spec-sanctioned state ("including zero
//! built-ins beyond the host-command tool").

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolRequestParams, ContentBlock, Tool as McpToolInfo};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::{RiskClass, Tool, ToolResult};

/// One MCP server to connect to over stdio: `command` is spawned with
/// `args`. Not persisted anywhere by this feature — see module docs.
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Used only to disambiguate a tool-name collision (data-model.md);
    /// not sent over the wire.
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
}

/// One tool discovered from one connected MCP server. `execute()` always
/// returns `Risky` (design doc §4) — MCP servers are arbitrary external
/// code the permission gate must not skip past by default.
struct McpTool {
    registry_name: String,
    mcp_tool_name: String,
    description: String,
    input_schema: Value,
    connection: Arc<RunningService<RoleClient, ()>>,
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.registry_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn source(&self) -> &'static str {
        "mcp"
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    fn risk_class(&self) -> RiskClass {
        RiskClass::Risky
    }

    async fn execute(&self, input: Value, cancel: CancellationToken) -> ToolResult {
        let arguments = input.as_object().cloned();
        let params = CallToolRequestParams::new(self.mcp_tool_name.clone());
        let params = match arguments {
            Some(args) => params.with_arguments(args),
            None => params,
        };
        tokio::select! {
            result = self.connection.call_tool(params) => {
                match result {
                    Ok(result) => {
                        let content = content_to_string(&result.content);
                        ToolResult {
                            content,
                            is_error: result.is_error.unwrap_or(false),
                        }
                    }
                    Err(e) => ToolResult::error(format!("mcp tools/call failed: {e}")),
                }
            }
            _ = cancel.cancelled() => {
                // Best-effort: `call_tool`'s multi-round convenience wrapper
                // does not expose the raw JSON-RPC request id a
                // spec-correct `notifications/cancelled` needs, and no
                // product surface can even reach this path yet (MCP
                // servers are not user-configurable in this feature,
                // spec.md Assumptions). The in-flight call is dropped and
                // any late response discarded, rather than reimplementing
                // `call_tool`'s retry loop over the raw cancellable-request
                // API just to attach an id.
                ToolResult::error("tool_call_cancelled")
            }
        }
    }
}

fn content_to_string(content: &[ContentBlock]) -> String {
    content
        .iter()
        .map(|block| {
            block
                .as_text()
                .map(|t| t.text.clone())
                .unwrap_or_else(|| "[unsupported content]".to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Connects to one server over stdio and completes the MCP handshake.
async fn connect(server: &McpServerConfig) -> Result<RunningService<RoleClient, ()>, String> {
    let args = server.args.clone();
    let command = tokio::process::Command::new(&server.command).configure(|cmd| {
        cmd.args(&args);
    });
    let transport = TokioChildProcess::new(command)
        .map_err(|e| format!("spawn {}: {e}", server.command))?;
    ().serve(transport)
        .await
        .map_err(|e| format!("mcp handshake with {}: {e}", server.id))
}

/// Lists `connection`'s tools and wraps each as a [`Tool`], disambiguating
/// any name collision with one already in `seen` via a `mcp:<server_id>:`
/// prefix (data-model.md Edge Case) rather than silently shadowing it.
/// Split out from [`discover_mcp_tools`] so tests can exercise the real
/// `tools/list` + wrapping logic against an in-process server (over a
/// `tokio::io::duplex` pair) without spawning a child process.
pub(crate) async fn tools_from_connection(
    server_id: &str,
    connection: Arc<RunningService<RoleClient, ()>>,
    seen: &mut std::collections::HashSet<String>,
) -> Result<Vec<Arc<dyn Tool>>, String> {
    let discovered: Vec<McpToolInfo> = connection
        .list_all_tools()
        .await
        .map_err(|e| format!("mcp server '{server_id}' tools/list failed: {e}"))?;

    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();
    for info in discovered {
        let raw_name = info.name.to_string();
        let mut registry_name = raw_name.clone();
        while seen.contains(&registry_name) {
            registry_name = format!("mcp:{server_id}:{registry_name}");
        }
        seen.insert(registry_name.clone());
        tools.push(Arc::new(McpTool {
            registry_name,
            mcp_tool_name: raw_name,
            description: info.description.clone().unwrap_or_default().into_owned(),
            input_schema: info.schema_as_json_value(),
            connection: connection.clone(),
        }));
    }
    Ok(tools)
}

/// Connects to every configured server, lists its tools, and wraps each as
/// a [`Tool`]. A server that fails to spawn or complete the handshake, or
/// whose `tools/list` fails, is skipped with a logged warning rather than
/// aborting discovery for the remaining servers (spec.md Edge Cases: a
/// disconnected/unavailable server must surface as a tool error, never a
/// panic — the same principle applies one level up, at discovery time).
pub async fn discover_mcp_tools(
    servers: &[McpServerConfig],
    existing_names: &std::collections::HashSet<String>,
) -> Vec<Arc<dyn Tool>> {
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();
    let mut seen: std::collections::HashSet<String> = existing_names.clone();

    for server in servers {
        let connection = match connect(server).await {
            Ok(c) => Arc::new(c),
            Err(reason) => {
                log::warn!("mcp server '{}' unavailable: {reason}", server.id);
                continue;
            }
        };
        match tools_from_connection(&server.id, connection, &mut seen).await {
            Ok(discovered) => tools.extend(discovered),
            Err(reason) => log::warn!("{reason}"),
        }
    }

    tools
}
