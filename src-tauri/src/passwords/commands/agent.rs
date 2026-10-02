//! The entrance of the built-in agent (spec 034, FR-027, `contracts/access.md`): one command that
//! the action `passwords.items.search` calls. The caller is fixed in the body, never an argument,
//! and the answer holds title, tag names, folder name and whether a TOTP exists, nothing else.

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use super::service;
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::passwords::model::AgentHeader;
use crate::state::AppState;

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/", optional_fields)]
#[serde(rename_all = "camelCase")]
pub struct AgentSearchArgs {
    pub query: Option<String>,
    pub tag: Option<String>,
    /// At most 50, 20 when left out.
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct AgentSearchResult {
    pub items: Vec<AgentHeader>,
}

#[tauri::command]
pub async fn passwords_agent_search(
    state: State<'_, AppState>,
    args: AgentSearchArgs,
) -> Result<AgentSearchResult> {
    let items = service(&state)?
        .agent_search(&Caller::BuiltinAgent, args.query, args.tag, args.limit)
        .await?;
    Ok(AgentSearchResult { items })
}
