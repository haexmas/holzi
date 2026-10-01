//! The instruction that goes with the tools (spec 032, measured in research R17/R18). One text for
//! every model and every turn that offers tools; nothing in it depends on the model or on what
//! the person wrote. The first measurement without it showed a local model that never searched
//! for a tool outside the core offer and answered in text instead of calling a core tool.

use crate::adapters::types::ToolSpec;

/// What the model is told about the tools holzi offers. English works best for the models that
/// run locally; the model answers in the language of the person.
pub const TOOL_INSTRUCTION: &str = "You operate the app holzi for the user with the tools you are offered. \
When the user asks to look at or change something in holzi, call a tool instead of describing what you would do. \
If none of the offered tools fits, call find_actions with a few keywords: the tools it finds are available in your next step. \
Reply in the user's language. For questions that have nothing to do with holzi, just answer.";

/// The system prompt of a turn: what the caller passed, plus the tool instruction when the turn
/// offers tools. A turn without tools gets nothing added.
pub fn system_prompt(given: Option<&str>, tools: &[ToolSpec]) -> Option<String> {
    if tools.is_empty() {
        return given.map(str::to_owned);
    }
    Some(match given {
        Some(given) if !given.trim().is_empty() => format!("{given}\n\n{TOOL_INSTRUCTION}"),
        _ => TOOL_INSTRUCTION.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{system_prompt, TOOL_INSTRUCTION};
    use crate::adapters::types::ToolSpec;

    fn tool() -> ToolSpec {
        ToolSpec {
            name: "find_actions".to_owned(),
            description: String::new(),
            input_schema: json!({ "type": "object" }),
        }
    }

    #[test]
    fn a_turn_with_tools_gets_the_instruction() {
        assert_eq!(
            system_prompt(None, &[tool()]).as_deref(),
            Some(TOOL_INSTRUCTION)
        );
        assert_eq!(
            system_prompt(Some("  "), &[tool()]).as_deref(),
            Some(TOOL_INSTRUCTION)
        );
    }

    #[test]
    fn a_given_prompt_comes_first() {
        let prompt = system_prompt(Some("Be brief."), &[tool()]).unwrap();
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
    fn the_instruction_names_the_search_tool_it_relies_on() {
        assert!(TOOL_INSTRUCTION.contains(crate::chat::tools::offer::FIND_ACTIONS_TOOL_NAME));
    }
}
