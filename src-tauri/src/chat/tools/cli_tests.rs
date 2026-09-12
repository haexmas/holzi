use tokio_util::sync::CancellationToken;

use super::cli::CliTool;
use super::Tool;

#[tokio::test]
async fn a_trivial_command_returns_its_stdout() {
    let tool = CliTool;
    let result = tool
        .execute(
            serde_json::json!({ "command": "echo hello" }),
            CancellationToken::new(),
        )
        .await;
    assert!(!result.is_error);
    assert_eq!(result.content.trim(), "hello");
}

#[tokio::test]
async fn a_failing_command_returns_is_error_without_panicking() {
    let tool = CliTool;
    let result = tool
        .execute(
            serde_json::json!({ "command": "exit 7" }),
            CancellationToken::new(),
        )
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn missing_command_input_is_a_tool_error_not_a_panic() {
    let tool = CliTool;
    let result = tool
        .execute(serde_json::json!({}), CancellationToken::new())
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn a_nonexistent_binary_is_a_tool_error_not_a_panic() {
    let tool = CliTool;
    let result = tool
        .execute(
            serde_json::json!({ "command": "definitely-not-a-real-binary-xyz" }),
            CancellationToken::new(),
        )
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn excessive_output_is_rejected_without_buffering_unbounded_data() {
    let tool = CliTool;
    let result = tool
        .execute(
            serde_json::json!({
                "command": "head -c 1048577 /dev/zero"
            }),
            CancellationToken::new(),
        )
        .await;
    assert!(result.is_error);
    assert!(result.content.contains("output exceeded"));
}

#[tokio::test]
async fn cancellation_kills_the_process_and_reaps_it_without_a_zombie() {
    let tool = CliTool;
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        cancel_for_task.cancel();
    });

    let started = std::time::Instant::now();
    let result = tool
        .execute(serde_json::json!({ "command": "sleep 5" }), cancel)
        .await;
    let elapsed = started.elapsed();

    assert!(result.is_error);
    assert_eq!(result.content, "tool_call_cancelled");
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "cancellation must kill the process instead of waiting out the full sleep: took {elapsed:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_kills_a_descendant_process_too() {
    let tool = CliTool;
    let cancel = CancellationToken::new();
    let cancel_for_execute = cancel.clone();
    let pid_file = tempfile::NamedTempFile::new().expect("pid file");
    let pid_path = pid_file.path().display().to_string();
    let command = format!("sleep 5 & echo $! > '{pid_path}'; wait");
    let execution = tokio::spawn(async move {
        tool.execute(
            serde_json::json!({ "command": command }),
            cancel_for_execute,
        )
        .await
    });

    let descendant_pid = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Ok(pid) = tokio::fs::read_to_string(&pid_path).await {
                if let Ok(pid) = pid.trim().parse::<libc::pid_t>() {
                    break pid;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("command must start its descendant");

    cancel.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), execution)
        .await
        .expect("cancellation must finish the command")
        .expect("command task must not panic");
    assert!(result.is_error);
    assert_eq!(result.content, "tool_call_cancelled");

    let descendant_gone = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            // `kill(pid, 0)` checks existence without sending a signal.
            if unsafe { libc::kill(descendant_pid, 0) } != 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        descendant_gone.is_ok(),
        "cancellation must terminate the shell's descendant process"
    );
}
