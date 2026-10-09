//! The safe meta-tool that searches the complete built-in action catalog.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::action_tool::{ActionEffect, AgentActionDef, ACTION_SOURCE};
use super::offer::{action_result, search_actions, MAX_SEARCH_RESULTS};
use super::{RiskClass, Tool, ToolResult};

pub const FIND_ACTIONS_TOOL_NAME: &str = super::offer::FIND_ACTIONS_TOOL_NAME;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct FindActionsInput {
    query: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

pub struct FindActionsTool {
    defs: Vec<AgentActionDef>,
}

impl FindActionsTool {
    pub fn new(defs: Vec<AgentActionDef>) -> Self {
        Self { defs }
    }
}

#[async_trait]
impl Tool for FindActionsTool {
    fn name(&self) -> &str {
        FIND_ACTIONS_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Search holzi actions by keywords, or enumerate the catalog when query is omitted. Matching actions become tools in the next step."
    }

    fn source(&self) -> &'static str {
        ACTION_SOURCE
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "cursor": { "type": "string" },
                "limit": { "type": "integer" },
            },
        })
    }

    fn risk_class(&self) -> RiskClass {
        ActionEffect::Read.risk_class()
    }

    async fn execute(&self, input: serde_json::Value, _cancel: CancellationToken) -> ToolResult {
        let input = match serde_json::from_value::<FindActionsInput>(input) {
            Ok(input) => input,
            Err(_) => return ToolResult::error("invalid_input"),
        };
        let (defs, next_cursor) = search_actions(
            &self.defs,
            input.query.as_deref(),
            input.cursor.as_deref(),
            input.limit.unwrap_or(MAX_SEARCH_RESULTS),
        );
        let mut result = json!({
            "actions": defs.iter().map(action_result).collect::<Vec<_>>(),
            "nextCursor": next_cursor,
        });
        if defs.is_empty() && input.query.is_some() {
            result["hint"] = json!(
                "no action matches these keywords; this is about actions, not about the user's data. \
                 Try other keywords, e.g. in English, or omit query to list every action."
            );
        }
        ToolResult::ok(result.to_string())
    }
}
