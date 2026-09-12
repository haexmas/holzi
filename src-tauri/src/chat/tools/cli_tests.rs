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
