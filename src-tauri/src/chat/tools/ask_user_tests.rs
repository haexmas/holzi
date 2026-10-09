use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::ask_user::{AskUserTool, ASK_USER_TOOL_NAME};
use super::{ChoiceOption, ChoiceRequest, RiskClass, Tool};

fn option(text: &str) -> ChoiceOption {
    ChoiceOption {
        value: text.into(),
        label: text.into(),
        unavailable: None,
    }
}

#[test]
fn the_tool_asks_for_a_question_and_two_to_five_options() {
    let tool = AskUserTool;
    assert_eq!(tool.name(), ASK_USER_TOOL_NAME);
    assert_eq!(tool.risk_class(), RiskClass::Safe);
    let schema = tool.input_schema();
    assert_eq!(schema["required"], json!(["question", "options"]));
    assert_eq!(schema["properties"]["options"]["minItems"], 2);
    assert_eq!(schema["properties"]["options"]["maxItems"], 5);
}

#[tokio::test]
async fn a_question_becomes_a_choice_for_the_tool_round() {
    let result = AskUserTool
        .execute(
            json!({ "question": "Which colour scheme?", "options": ["Dark", "Light"] }),
            CancellationToken::new(),
        )
        .await;
    assert_eq!(
        result.choice,
        Some(ChoiceRequest {
            question: Some("Which colour scheme?".into()),
            field: None,
            value: String::new(),
            options: vec![option("Dark"), option("Light")],
        })
    );
}

#[tokio::test]
async fn too_few_or_too_many_options_or_no_question_are_invalid_input() {
    for input in [
        json!({ "question": "Which?", "options": ["Only"] }),
        json!({ "question": "Which?", "options": ["1", "2", "3", "4", "5", "6"] }),
        json!({ "question": "  ", "options": ["A", "B"] }),
        json!({ "options": ["A", "B"] }),
    ] {
        let result = AskUserTool.execute(input, CancellationToken::new()).await;
        assert!(result.is_error);
        assert_eq!(result.content, "invalid_input");
        assert!(result.choice.is_none());
    }
}
