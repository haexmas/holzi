//! Host-CLI tool: runs an arbitrary shell command on this device.
//!
//! Always `Risky` (spec.md FR-015) — there is no working-directory or
//! command restriction in this feature's scope, so the permission gate is
//! the only thing standing between the model and an arbitrary local
//! command.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::process::ChildStderr;
use tokio::process::ChildStdout;

use super::{RiskClass, Tool, ToolResult};

const NAME: &str = "run_command";
const DESCRIPTION: &str = "Runs a shell command on the user's device and returns its output.";
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

async fn read_limited<R>(
    mut reader: R,
    total: Arc<AtomicUsize>,
) -> Result<Vec<u8>, &'static str>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|_| "failed to read command output")?;
        if read == 0 {
            return Ok(output);
        }
        let within_limit = total
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |current| {
                current
                    .checked_add(read)
                    .filter(|next| *next <= MAX_OUTPUT_BYTES)
            })
            .is_ok();
        if !within_limit {
            return Err("command output exceeded the 1 MiB limit");
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

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
        let mut child = match Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => return ToolResult::error(format!("failed to spawn command: {e}")),
        };

        let Some(stdout): Option<ChildStdout> = child.stdout.take() else {
            return ToolResult::error("failed to capture command stdout");
        };
        let Some(stderr): Option<ChildStderr> = child.stderr.take() else {
            return ToolResult::error("failed to capture command stderr");
        };
        let total = Arc::new(AtomicUsize::new(0));
        let mut stdout_task = Some(tokio::spawn(read_limited(stdout, total.clone())));
        let mut stderr_task = Some(tokio::spawn(read_limited(stderr, total)));
        let mut stdout_bytes = None;
        let mut stderr_bytes = None;

        let execution = tokio::time::timeout(EXECUTION_TIMEOUT, async {
            while stdout_bytes.is_none() || stderr_bytes.is_none() {
                match (stdout_bytes.is_none(), stderr_bytes.is_none()) {
                    (true, true) => tokio::select! {
                        result = stdout_task.as_mut().unwrap() => {
                            stdout_task = None;
                            stdout_bytes = Some(result.map_err(|_| "output reader task failed")??);
                        }
                        result = stderr_task.as_mut().unwrap() => {
                            stderr_task = None;
                            stderr_bytes = Some(result.map_err(|_| "output reader task failed")??);
                        }
                    },
                    (true, false) => {
                        let result = stdout_task.take().unwrap().await;
                        stdout_bytes = Some(result.map_err(|_| "output reader task failed")??);
                    }
                    (false, true) => {
                        let result = stderr_task.take().unwrap().await;
                        stderr_bytes = Some(result.map_err(|_| "output reader task failed")??);
                    }
                    (false, false) => unreachable!(),
                }
            }
            let status = child
                .wait()
                .await
                .map_err(|_| "failed to wait for command")?;
            Ok::<_, &'static str>((status, stdout_bytes.unwrap(), stderr_bytes.unwrap()))
        })
        .await;

        let failed = !matches!(&execution, Ok(Ok(_)));
        if failed {
            let _ = child.kill().await;
            let _ = child.wait().await;
            if let Some(task) = stdout_task {
                task.abort();
                let _ = task.await;
            }
            if let Some(task) = stderr_task {
                task.abort();
                let _ = task.await;
            }
        }

        match execution {
            Ok(Ok((status, stdout, stderr))) => {
                let mut content = String::from_utf8_lossy(&stdout).into_owned();
                if !stderr.is_empty() {
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(&String::from_utf8_lossy(&stderr));
                }
                if status.success() {
                    ToolResult::ok(content)
                } else {
                    let code = status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "signal".to_string());
                    if content.is_empty() {
                        content = format!("command exited with status {code}");
                    }
                    ToolResult::error(content)
                }
            }
            Ok(Err(reason)) => ToolResult::error(reason),
            Err(_) => ToolResult::error("command timed out after 30 seconds"),
        }
    }
}
