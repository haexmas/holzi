//! Tauri commands of the file actions of agents (contracts/tauri-commands.md): the answer to the
//! question for a storage, and the check behind `files.show`, which opens a window and so runs in
//! the frontend (ADR 0011).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::env::TauriEnv;
use super::exec::FilesAgent;
use super::prompt::{FilesAgentChoice, PermissionPrompt};
use crate::files::{FilesError, SourceRef};

/// The executor of the built-in agent, managed as a state.
pub type BuiltinFilesAgent = Arc<FilesAgent<TauriEnv<tauri::Wry>>>;

/// The answer to `files-agent-permission-request`. A late or unknown id is not an error.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesAgentAnswerArgs {
    pub request_id: String,
    pub choice: FilesAgentChoice,
}

#[tauri::command]
pub fn files_agent_permission_answer(
    prompt: State<'_, PermissionPrompt>,
    args: FilesAgentAnswerArgs,
) {
    prompt.answer(&args.request_id, args.choice);
}

/// Whether the agent may show the entry, and where the file browser opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct FilesAgentShow {
    pub allowed: bool,
    /// Why not: the code and message the agent gets.
    pub reason: Option<String>,
    pub source: Option<SourceRef>,
    /// The folder the browser opens.
    pub folder: Option<String>,
    /// The file opened in the viewer, by name in `folder`.
    pub open: Option<String>,
}

/// The checks of `files.stat` for the built-in agent, without content (FR-032a). A refusal is an
/// answer, not an error: the handler passes the reason on to the agent.
#[tauri::command]
pub async fn files_agent_check(
    agent: State<'_, BuiltinFilesAgent>,
    source: String,
    path: String,
) -> Result<FilesAgentShow, FilesError> {
    Ok(
        match agent
            .show_target(&source, &path, &CancellationToken::new())
            .await
        {
            Ok((source, folder, open)) => FilesAgentShow {
                allowed: true,
                reason: None,
                source: Some(source),
                folder: Some(folder),
                open,
            },
            Err(error) => FilesAgentShow {
                allowed: false,
                reason: Some(format!("{}: {}", error.code, error.message)),
                source: None,
                folder: None,
                open: None,
            },
        },
    )
}
