use serde_json::json;

use super::prompt::{system_prompt, TOOL_INSTRUCTION};
use crate::adapters::types::ToolSpec;

fn tool(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_owned(),
        description: String::new(),
        input_schema: json!({ "type": "object" }),
    }
}

#[test]
fn a_turn_with_tools_gets_the_instruction() {
    assert_eq!(
        system_prompt(None, &[tool("find_actions")]).as_deref(),
        Some(TOOL_INSTRUCTION)
    );
    assert_eq!(
        system_prompt(Some("  "), &[tool("find_actions")]).as_deref(),
        Some(TOOL_INSTRUCTION)
    );
}

#[test]
fn a_given_prompt_comes_first() {
    let prompt = system_prompt(Some("Be brief."), &[tool("find_actions")]).unwrap();
    assert!(prompt.starts_with("Be brief.\n\n"));
    assert!(prompt.ends_with(TOOL_INSTRUCTION));
}

#[test]
fn a_turn_without_tools_is_left_as_it_is() {
    assert_eq!(system_prompt(None, &[]), None);
    assert_eq!(
        system_prompt(Some("Be brief."), &[]).as_deref(),
        Some("Be brief.")
    );
}

#[test]
fn a_turn_without_search_does_not_mention_an_unavailable_tool() {
    let prompt = system_prompt(None, &[tool("run_command")]).unwrap();
    assert!(!prompt.contains("find_actions"));
    assert!(prompt.contains("call one of the offered tools"));
}

#[test]
fn the_instruction_names_the_search_tool_it_relies_on() {
    assert!(TOOL_INSTRUCTION.contains(crate::chat::tools::offer::FIND_ACTIONS_TOOL_NAME));
}

#[test]
fn a_turn_offering_ask_user_is_told_to_ask_instead_of_refusing() {
    let with = system_prompt(None, &[tool("find_actions"), tool("ask_user")]).unwrap();
    assert!(with.contains("ask_user"));
    assert!(with.contains("instead of refusing"));
    let without = system_prompt(None, &[tool("find_actions")]).unwrap();
    assert!(!without.contains("ask_user"));
}
