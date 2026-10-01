use serde_json::json;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::action_bridge::ActionBridge;
use super::action_tool::ActionTool;
use super::action_tool::AgentActionDef;
use super::find_actions::FindActionsTool;
use super::offer::{core_offer, extend_offer, found_tools, search_actions};
use super::{Tool, ToolRegistry};
use crate::adapters::types::ToolSpec;

fn def(id: &str, description: &str, core: bool) -> AgentActionDef {
    serde_json::from_value(json!({
        "toolName": id.replace('.', "_"),
        "actionId": id,
        "description": description,
        "inputSchema": { "type": "object" },
        "effect": "read",
        "core": core,
        "titles": { "de": "", "en": "" },
    }))
    .expect("test action definition")
}

#[test]
fn search_ranks_word_hits_and_paginates_stably() {
    let defs = vec![
        def("settings.models.list", "List models", false),
        def("wm.apps.list", "List apps", true),
        def("settings.get", "Read all settings", true),
    ];

    let (page, next) = search_actions(&defs, Some("settings"), None, 2);
    assert_eq!(
        page.iter()
            .map(|action| action.action_id.as_str())
            .collect::<Vec<_>>(),
        ["settings.get", "settings.models.list"]
    );
    assert_eq!(next, None);

    let (page, next) = search_actions(&defs, None, None, 2);
    assert_eq!(page.len(), 2);
    assert_eq!(next.as_deref(), Some("2"));
    let (page, next) = search_actions(&defs, None, next.as_deref(), 2);
    assert_eq!(page.len(), 1);
    assert_eq!(next, None);
}

#[test]
fn extend_offer_replaces_non_core_actions_and_preserves_other_tools() {
    let core = def("wm.apps.list", "List apps", true);
    let first = def("settings.get", "Read settings", false);
    let second = def("settings.models.list", "List models", false);
    let defs = vec![core.clone(), first.clone(), second.clone()];
    let mut offer = vec![
        ToolSpec {
            name: "run_command".into(),
            description: "shell".into(),
            input_schema: json!({ "type": "object" }),
        },
        ToolSpec {
            name: core.tool_name.clone(),
            description: core.description.clone(),
            input_schema: core.input_schema.clone(),
        },
        ToolSpec {
            name: first.tool_name.clone(),
            description: first.description.clone(),
            input_schema: first.input_schema.clone(),
        },
    ];
    let found = vec![
        ToolSpec {
            name: second.tool_name.clone(),
            description: second.description.clone(),
            input_schema: second.input_schema.clone(),
        },
        ToolSpec {
            name: core.tool_name.clone(),
            description: "duplicate core".into(),
            input_schema: json!({}),
        },
    ];

    extend_offer(&mut offer, &found, &defs);
    assert_eq!(
        offer
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        ["run_command", "wm_apps_list", "settings_models_list",]
    );
}

#[test]
fn found_tools_only_accepts_the_search_result_shape() {
    let found = found_tools(
        &json!({
            "actions": [{
                "tool": "settings_get",
                "description": "Read settings",
                "inputSchema": { "type": "object" }
            }]
        })
        .to_string(),
    );
    assert_eq!(found[0].name, "settings_get");
    assert!(found_tools("not json").is_empty());
}

#[test]
fn core_offer_keeps_non_core_actions_hidden_but_offers_the_search_tool() {
    let core = def("wm.apps.list", "List apps", true);
    let hidden = def("settings.get", "Read settings", false);
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ActionTool::new(
        core.clone(),
        ActionBridge::default(),
    )));
    registry.register(Arc::new(ActionTool::new(hidden, ActionBridge::default())));
    registry.register(Arc::new(FindActionsTool::new(vec![core])));

    let names = core_offer(&registry)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    assert_eq!(names, ["wm_apps_list", "find_actions"]);
}

#[tokio::test]
async fn find_actions_returns_bounded_search_results() {
    let defs = (0..7)
        .map(|index| def(&format!("settings.item{index}"), "Settings item", false))
        .collect();
    let tool = FindActionsTool::new(defs);
    let result = tool
        .execute(
            json!({ "query": "settings", "limit": 99 }),
            CancellationToken::new(),
        )
        .await;
    assert!(!result.is_error);
    let value: serde_json::Value = serde_json::from_str(&result.content).expect("JSON result");
    assert_eq!(value["actions"].as_array().expect("actions").len(), 5);
    assert_eq!(value["nextCursor"], "5");
}
