use super::cli::CliTool;
use super::Tool;

#[tokio::test]
async fn a_trivial_command_returns_its_stdout() {
    let tool = CliTool;
    let result = tool
        .execute(serde_json::json!({ "command": "echo hello" }))
        .await;
    assert!(!result.is_error);
    assert_eq!(result.content.trim(), "hello");
}

#[tokio::test]
async fn a_failing_command_returns_is_error_without_panicking() {
    let tool = CliTool;
    let result = tool
        .execute(serde_json::json!({ "command": "exit 7" }))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn missing_command_input_is_a_tool_error_not_a_panic() {
    let tool = CliTool;
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn a_nonexistent_binary_is_a_tool_error_not_a_panic() {
    let tool = CliTool;
    let result = tool
        .execute(serde_json::json!({ "command": "definitely-not-a-real-binary-xyz" }))
        .await;
    assert!(result.is_error);
}
