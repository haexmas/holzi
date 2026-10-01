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

/// The instruction for a turn whose tools do not include the optional action search.
const TOOL_INSTRUCTION_WITHOUT_SEARCH: &str = "You operate the app holzi for the user with the tools you are offered. \
When the user asks to look at or change something in holzi, call one of the offered tools instead of describing what you would do. \
Reply in the user's language. For questions that have nothing to do with holzi, just answer.";

/// The system prompt of a turn: what the caller passed, plus the tool instruction when the turn
/// offers tools. A turn without tools gets nothing added.
pub fn system_prompt(given: Option<&str>, tools: &[ToolSpec]) -> Option<String> {
    if tools.is_empty() {
        return given.map(str::to_owned);
    }
    let instruction = if tools
        .iter()
        .any(|tool| tool.name == crate::chat::tools::offer::FIND_ACTIONS_TOOL_NAME)
    {
        TOOL_INSTRUCTION
    } else {
        TOOL_INSTRUCTION_WITHOUT_SEARCH
    };
    Some(match given {
        Some(given) if !given.trim().is_empty() => format!("{given}\n\n{instruction}"),
        _ => instruction.to_owned(),
    })
}
