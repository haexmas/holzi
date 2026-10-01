//! Measuring how well a model calls holzi's tools (spec 032 US5, research R9). The set of example
//! sentences, the pure scoring and the runner that asks a model; the in-app self-test and the
//! full measurement (`tests/model_tool_eval.rs`) both go through here. Needs no vault and never
//! runs an action (FR-021, FR-022).

pub mod runner;
pub mod scoring;

#[cfg(test)]
#[path = "runner_tests.rs"]
mod runner_tests;
#[cfg(test)]
#[path = "scoring_tests.rs"]
mod scoring_tests;
#[cfg(test)]
pub(crate) mod testing;

use crate::chat::tools::action_tool::AgentActionDef;

use scoring::EvalSet;

/// The versioned set of example sentences (contracts/eval-format.md).
const EVAL_SET_JSON: &str = include_str!("eval_set.json");

/// Snapshot of the tools offered to the built-in agent, written by `scripts/export-eval-tools.ts`.
/// `pnpm check:agent-actions` fails when it differs from the catalog.
const TOOLS_JSON: &str = include_str!("tools.json");

/// The embedded set. Panics on a malformed file: it is built in, and a test parses it.
pub fn embedded_set() -> EvalSet {
    serde_json::from_str(EVAL_SET_JSON).expect("the embedded eval_set.json is valid")
}

/// The embedded tool definitions the sentences are measured against.
pub fn embedded_tools() -> Vec<AgentActionDef> {
    serde_json::from_str(TOOLS_JSON).expect("the embedded tools.json is valid")
}
