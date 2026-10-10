use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::action_tool::AgentActionDef;
use super::native_action::{NativeActionTool, NativeError, NativeExecutor, NativeExecutors};
use super::{RiskClass, Tool};

/// Answers `files.list` with its input, fails `files.stat` with a field and never ends `files.read`.
struct Echo;

#[async_trait]
impl NativeExecutor for Echo {
    fn action_ids(&self) -> &'static [&'static str] {
        &["files.list", "files.stat", "files.read"]
    }

    async fn run(
        &self,
        action_id: &str,
        input: Value,
        _cancel: &CancellationToken,
    ) -> Result<Value, NativeError> {
        match action_id {
            "files.list" => Ok(json!({ "got": input })),
            "files.stat" => Err(NativeError::invalid_input(Some("path"), "no such path")),
            _ => std::future::pending().await,
        }
    }
}

fn def(action_id: &str, effect: &str) -> AgentActionDef {
    serde_json::from_value(json!({
        "toolName": action_id.replace('.', "_"),
        "actionId": action_id,
        "description": "d",
        "inputSchema": { "type": "object" },
        "effect": effect,
        "runner": "native",
    }))
    .expect("a valid definition")
}

#[test]
fn source_risk_and_definition_are_those_of_an_action() {
    for (effect, risk) in [
        ("read", RiskClass::Safe),
        ("write", RiskClass::Change),
        ("destructive", RiskClass::Risky),
    ] {
        let tool = NativeActionTool::new(def("files.list", effect), Arc::new(Echo));
        assert_eq!(tool.risk_class(), risk, "{effect}");
        assert_eq!(tool.source(), "action");
        assert_eq!(tool.name(), "files_list");
        assert_eq!(
            tool.action_definition().map(|d| d.action_id.as_str()),
            Some("files.list")
        );
    }
}

#[tokio::test]
async fn a_result_goes_to_the_model_as_json() {
    let tool = NativeActionTool::new(def("files.list", "read"), Arc::new(Echo));
    let result = tool
        .execute(json!({ "path": "/a" }), CancellationToken::new())
        .await;
    assert!(!result.is_error);
    assert_eq!(
        serde_json::from_str::<Value>(&result.content).expect("json"),
        json!({ "got": { "path": "/a" } })
    );
}

#[tokio::test]
async fn an_error_carries_code_field_and_message() {
    let tool = NativeActionTool::new(def("files.stat", "read"), Arc::new(Echo));
    let result = tool.execute(json!({}), CancellationToken::new()).await;
    assert!(result.is_error);
    assert_eq!(
        serde_json::from_str::<Value>(&result.content).expect("json"),
        json!({ "error": { "code": "invalid_input", "field": "path", "message": "no such path" } })
    );
}

#[tokio::test]
async fn a_cancelled_turn_ends_the_call() {
    let tool = NativeActionTool::new(def("files.read", "read"), Arc::new(Echo));
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = tool.execute(json!({}), cancel).await;
    assert!(result.is_error);
    assert_eq!(result.content, "tool_call_cancelled");
}

#[test]
fn executors_are_found_by_each_of_their_ids() {
    let executors = NativeExecutors::default();
    assert!(executors.get("files.list").is_none());
    executors.add(Arc::new(Echo));
    assert!(executors.get("files.stat").is_some());
    let mut ids = executors.ids();
    ids.sort_unstable();
    assert_eq!(ids, ["files.list", "files.read", "files.stat"]);
}
