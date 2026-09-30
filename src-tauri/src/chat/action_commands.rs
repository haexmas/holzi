//! The two commands of the action bridge (spec 032, contracts/tauri-commands.md): the frontend
//! pushes the definitions of the actions the built-in agent may call, and answers the calls the
//! bridge sends out. Both need an open vault: they are not on the gate's app-scoped list.

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use super::session::ChatState;
use super::tools::action_bridge::ActionOutcomeWire;
use super::tools::action_tool::{
    is_valid_tool_name, schema_in_subset, ActionTool, AgentActionDef, ACTION_SOURCE,
};
use crate::error::{HolziError, Result};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAgentActionsArgs {
    pub actions: Vec<AgentActionDef>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAgentActionsResult {
    pub registered: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RespondActionCallArgs {
    pub request_id: Uuid,
    pub outcome: ActionOutcomeWire,
}

/// Replaces every tool of the source `action` by `actions`; the host-CLI and MCP tools stay.
/// Validates first, so a bad push leaves the registry as it was.
pub fn register_agent_actions(chat: &ChatState, actions: Vec<AgentActionDef>) -> Result<usize> {
    let mut seen = HashSet::new();
    for def in &actions {
        if !is_valid_tool_name(&def.tool_name) {
            return Err(HolziError::InvalidInput {
                reason: format!("invalid tool name: {}", def.tool_name),
            });
        }
        if !seen.insert(def.tool_name.as_str()) {
            return Err(HolziError::InvalidInput {
                reason: format!("duplicate tool name: {}", def.tool_name),
            });
        }
        if !schema_in_subset(&def.input_schema) {
            return Err(HolziError::InvalidInput {
                reason: format!(
                    "input schema of {} is outside the supported subset",
                    def.tool_name
                ),
            });
        }
    }
    let count = actions.len();
    let mut registry = chat
        .tool_registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry.remove_source(ACTION_SOURCE);
    for def in actions {
        registry.register(Arc::new(ActionTool::new(def, chat.action_bridge.clone())));
    }
    Ok(count)
}

/// Offers the actions the frontend lists to the built-in agent as tools.
#[tauri::command]
pub async fn set_agent_actions(
    chat: State<'_, ChatState>,
    args: SetAgentActionsArgs,
) -> Result<SetAgentActionsResult> {
    let registered = register_agent_actions(&chat, args.actions)?;
    Ok(SetAgentActionsResult { registered })
}

/// The frontend's answer to an `action-call-request`. An unknown or late request id is not an
/// error: the turn may have been cancelled, or the call timed out, in the meantime.
#[tauri::command]
pub async fn respond_action_call(
    chat: State<'_, ChatState>,
    args: RespondActionCallArgs,
) -> Result<()> {
    chat.action_bridge
        .resolve(args.request_id, args.outcome.into());
    Ok(())
}
