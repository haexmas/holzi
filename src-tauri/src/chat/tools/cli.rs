//! Host-CLI tool: runs an arbitrary shell command on this device.
//!
//! Always `Risky` (spec.md FR-015) — there is no working-directory or
//! command restriction in this feature's scope, so the permission gate is
//! the only thing standing between the model and an arbitrary local
//! command.

use async_trait::async_trait;
use serde_json::Value;

use super::{RiskClass, Tool, ToolResult};

const NAME: &str = "run_command";
const DESCRIPTION: &str = "Runs a shell command on the user's device and returns its output.";

/// Registered unconditionally in every `ChatState` (T019) — unlike MCP
/// tools, this one is never connection-dependent.
pub struct CliTool;

fn input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "command": {
                "type": "string",
                "description": "The shell command to execute, e.g. \"ls -la\"."
            }
        },
        "required": ["command"],
    })
}

#[async_trait]
impl Tool for CliTool {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn source(&self) -> &'static str {
        "cli"
    }

    fn input_schema(&self) -> Value {
        input_schema()
    }

    fn risk_class(&self) -> RiskClass {
        RiskClass::Risky
    }

    async fn execute(&self, input: Value) -> ToolResult {
        let Some(command) = input.get("command").and_then(Value::as_str) else {
            return ToolResult::error("missing required \"command\" string input");
        };

        // A shell (`sh -c`) is used deliberately so the model can rely on
        // shell features (pipes, globs, redirection) — spec.md FR-015
        // already accepts no command/working-directory restriction, so
        // there is no additional risk introduced by going through a shell
        // versus exec'ing a single argv.
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await;

        match output {
            Ok(output) => {
                let mut content = String::from_utf8_lossy(&output.stdout).into_owned();
                if !output.stderr.is_empty() {
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(&String::from_utf8_lossy(&output.stderr));
                }
                if output.status.success() {
                    ToolResult::ok(content)
                } else {
                    let code = output
                        .status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "signal".to_string());
                    if content.is_empty() {
                        content = format!("command exited with status {code}");
                    }
                    ToolResult::error(content)
                }
            }
            Err(e) => ToolResult::error(format!("failed to spawn command: {e}")),
        }
    }
}
