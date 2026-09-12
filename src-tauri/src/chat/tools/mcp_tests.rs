//! Exercises MCP tool discovery and execution against a real, minimal
//! `ServerHandler` connected over an in-process `tokio::io::duplex` pair —
//! no child process needed, so these tests are hermetic and fast while
//! still driving the genuine `rmcp` client/server wire protocol (only the
//! transport is swapped for `TokioChildProcess`; `tools_from_connection`
//! and `Tool::execute` are exercised exactly as production uses them).

use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult,
    PaginatedRequestParams, ServerInfo, Tool as McpToolInfo,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer, RunningService};
use rmcp::{ErrorData as McpError, RoleClient, ServerHandler, ServiceExt};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::mcp::{tools_from_connection, McpServerConfig};

#[derive(Clone, Default)]
struct EchoServer {
    call_started: Option<Arc<Notify>>,
}

impl ServerHandler for EchoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::default()
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + MaybeSendFuture + '_ {
        let schema: serde_json::Map<String, serde_json::Value> = serde_json::json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"],
        })
        .as_object()
        .expect("object literal")
        .clone();
        let tools = vec![McpToolInfo::new(
            "echo",
            "Echoes the given text back",
            schema,
        )];
        std::future::ready(Ok(ListToolsResult {
            tools,
            ..Default::default()
        }))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, McpError>> + MaybeSendFuture + '_ {
        let result = if request.name == "echo" {
            let text = request
                .arguments
                .as_ref()
                .and_then(|args| args.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            CallToolResult::success(vec![ContentBlock::text(text)])
        } else {
            CallToolResult::error(vec![ContentBlock::text(format!(
                "unknown tool: {}",
                request.name
            ))])
        };
        let call_started = self.call_started.clone();
        async move {
            if let Some(call_started) = call_started {
                call_started.notify_one();
                std::future::pending::<Result<CallToolResponse, McpError>>().await
            } else {
                Ok(result.into())
            }
        }
    }
}

/// Spawns `EchoServer` on one end of an in-memory duplex pipe and connects
/// a bare client (`()`, matching production's `().serve(transport)`) to
/// the other end. Returns the server task's handle too, so a test can
/// `.abort()` it to simulate the server disconnecting mid-conversation.
async fn connect_in_memory(
    server: EchoServer,
) -> (RunningService<RoleClient, ()>, tokio::task::JoinHandle<()>) {
    let (server_io, client_io) = tokio::io::duplex(4096);
    let server_task = tokio::spawn(async move {
        let _ = server
            .serve(server_io)
            .await
            .expect("server should start")
            .waiting()
            .await;
    });
    let client = ().serve(client_io).await.expect("client should connect");
    (client, server_task)
}

#[tokio::test]
async fn tool_discovery_populates_the_registry() {
    let (connection, _server_task) = connect_in_memory(EchoServer::default()).await;
    let connection = Arc::new(connection);
    let mut seen = HashSet::new();
    let tools = tools_from_connection("test-server", connection, &mut seen)
        .await
        .expect("discovery succeeds");

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name(), "echo");
    assert_eq!(tools[0].source(), "mcp");

    let result = tools[0]
        .execute(
            serde_json::json!({ "text": "hello mcp" }),
            CancellationToken::new(),
        )
        .await;
    assert!(!result.is_error);
    assert_eq!(result.content, "hello mcp");
}

#[tokio::test]
async fn a_disconnected_server_surfaces_a_tool_error_not_a_panic() {
    let (connection, server_task) = connect_in_memory(EchoServer::default()).await;
    let connection = Arc::new(connection);
    let mut seen = HashSet::new();
    let tools = tools_from_connection("test-server", connection, &mut seen)
        .await
        .expect("discovery succeeds");
    assert_eq!(tools.len(), 1);

    // Kill the server side, simulating it disconnecting mid-conversation
    // (spec.md Edge Cases) — dropping its half of the duplex pipe closes
    // the transport out from under the still-live client connection.
    server_task.abort();

    // Bounded so a hang (rather than a clean error) fails the test
    // instead of blocking the suite indefinitely.
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tools[0].execute(
            serde_json::json!({ "text": "hello?" }),
            CancellationToken::new(),
        ),
    )
    .await;
    match outcome {
        Ok(result) => assert!(result.is_error, "a call on a dead connection must not panic"),
        Err(_) => panic!("execute() hung instead of erroring on a dead connection"),
    }
}

#[tokio::test]
async fn cancellation_ends_the_wait_without_erroring_on_the_transport() {
    let call_started = Arc::new(Notify::new());
    let (connection, server_task) = connect_in_memory(EchoServer {
        call_started: Some(call_started.clone()),
    })
    .await;
    let connection = Arc::new(connection);
    let mut seen = HashSet::new();
    let tools = tools_from_connection("test-server", connection, &mut seen)
        .await
        .expect("discovery succeeds");

    let cancel = CancellationToken::new();
    let cancel_for_execute = cancel.clone();
    let tool = tools[0].clone();
    let execution = tokio::spawn(async move {
        tool.execute(
            serde_json::json!({ "text": "hello mcp" }),
            cancel_for_execute,
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), call_started.notified())
        .await
        .expect("MCP handler must observe the in-flight call");
    cancel.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), execution)
        .await
        .expect("cancellation must finish the in-flight MCP call")
        .expect("MCP execution task must not panic");
    assert!(result.is_error);
    assert_eq!(result.content, "tool_call_cancelled");
    server_task.abort();
}

#[tokio::test]
async fn an_unspawnable_command_is_skipped_without_panicking() {
    let servers = vec![McpServerConfig {
        id: "bogus".to_string(),
        command: "definitely-not-a-real-binary-xyz".to_string(),
        args: vec![],
    }];
    let tools = super::mcp::discover_mcp_tools(&servers, &HashSet::new()).await;
    assert!(tools.is_empty());
}
