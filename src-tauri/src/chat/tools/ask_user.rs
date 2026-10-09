//! The agent's own question to the user (spec 046, US3, FR-013–FR-016). When an instruction has
//! several sensible readings, the model calls `ask_user` instead of refusing; the tool round shows
//! the question in the chat (`chat/turn/choices.rs`) and the answer becomes this call's result.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::action_tool::ACTION_SOURCE;
use super::{ChoiceOption, ChoiceRequest, RiskClass, Tool, ToolResult};

pub const ASK_USER_TOOL_NAME: &str = "ask_user";
const MIN_OPTIONS: usize = 2;
const MAX_OPTIONS: usize = 5;

#[derive(Debug, Deserialize)]
struct AskUserInput {
    question: String,
    options: Vec<String>,
}

pub struct AskUserTool;

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        ASK_USER_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Ask the user a question and offer 2 to 5 possible answers. Use it when you cannot carry out an instruction unambiguously, instead of refusing. The user can also answer in their own words or decline."
    }

    /// Offered with the actions and recorded like them; it changes nothing, so it never needs an
    /// approval (`plan_calls` lets it through in every mode, FR-015).
    fn source(&self) -> &'static str {
        ACTION_SOURCE
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "question": { "type": "string" },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": MIN_OPTIONS,
                    "maxItems": MAX_OPTIONS,
                },
            },
            "required": ["question", "options"],
        })
    }

    fn risk_class(&self) -> RiskClass {
        RiskClass::Safe
    }

    async fn execute(&self, input: serde_json::Value, _cancel: CancellationToken) -> ToolResult {
        let Ok(input) = serde_json::from_value::<AskUserInput>(input) else {
            return ToolResult::error("invalid_input");
        };
        let question = input.question.trim();
        if question.is_empty() || !(MIN_OPTIONS..=MAX_OPTIONS).contains(&input.options.len()) {
            return ToolResult::error("invalid_input");
        }
        let options = input
            .options
            .into_iter()
            .map(|text| ChoiceOption {
                value: text.clone(),
                label: text,
                unavailable: None,
            })
            .collect::<Vec<_>>();
        // What a caller without a chat would see; the tool round replaces it by the answer.
        let content = json!({ "question": question, "options": options }).to_string();
        ToolResult::needs_choice(
            content,
            ChoiceRequest {
                question: Some(question.to_owned()),
                field: None,
                value: String::new(),
                options,
            },
        )
    }
}
