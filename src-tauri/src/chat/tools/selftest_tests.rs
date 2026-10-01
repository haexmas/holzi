use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::{find_tool_use, verdict, SELF_TEST_PASS};
use crate::adapters::types::{StreamChunk, ToolTemplateProbe};
use crate::chat::eval::runner::EvalError;
use crate::chat::eval::testing::{calls, oracle, Scripted};
use crate::model_capabilities::{ToolSupport, ToolUse, ToolUseBasis};

#[test]
fn the_verdict_follows_the_threshold() {
    assert_eq!(verdict(SELF_TEST_PASS), ToolSupport::Supported);
    assert_eq!(verdict(1.0), ToolSupport::Supported);
    assert_eq!(verdict(SELF_TEST_PASS - 0.01), ToolSupport::Unsupported);
    assert_eq!(verdict(0.0), ToolSupport::Unsupported);
}

#[tokio::test]
async fn a_model_that_calls_what_is_expected_is_supported_on_the_self_test() {
    let found = find_tool_use(&oracle(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        found,
        ToolUse::new(ToolSupport::Supported, ToolUseBasis::SelfTest)
    );
}

#[tokio::test]
async fn a_model_that_only_talks_is_unsupported_on_the_self_test() {
    let talker = Scripted::new(|_| {
        vec![StreamChunk::Delta {
            content: "Sorry, I cannot.".to_owned(),
            reasoning: None,
        }]
    });
    let found = find_tool_use(&talker, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        found,
        ToolUse::new(ToolSupport::Unsupported, ToolUseBasis::SelfTest)
    );
}

#[tokio::test]
async fn a_model_that_always_calls_a_tool_is_unsupported_too() {
    let eager = Scripted::new(|_| vec![calls(vec![("wm_state_get", json!({}))])]);
    let found = find_tool_use(&eager, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(found.support, ToolSupport::Unsupported);
}

#[tokio::test]
async fn a_template_that_ignores_tools_ends_it_without_asking_the_model() {
    let mut blind = oracle();
    blind.probe = ToolTemplateProbe::IgnoresTools;
    let found = find_tool_use(&blind, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        found,
        ToolUse::new(ToolSupport::Unsupported, ToolUseBasis::Template)
    );
    assert!(
        blind.seen.lock().unwrap().is_empty(),
        "no sentence was asked"
    );
}

#[tokio::test]
async fn only_the_marked_sentences_are_asked() {
    let model = oracle();
    find_tool_use(&model, &CancellationToken::new())
        .await
        .unwrap();
    let asked = model.seen.lock().unwrap().len();
    assert!(
        (5..=6).contains(&asked),
        "{asked} requests for five sentences"
    );
}

#[tokio::test]
async fn a_cancelled_check_gives_no_result() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = find_tool_use(&oracle(), &cancel).await;
    assert!(matches!(result, Err(EvalError::Cancelled)));
}
