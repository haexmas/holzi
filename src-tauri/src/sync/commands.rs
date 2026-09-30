//! Tauri commands of the own-device sync (spec 024, contracts/
//! tauri-commands.md): linking a device. The main device's commands need the
//! running sync service of the open vault; the join commands run on the
//! start page, with no vault open.

use std::sync::Arc;

use serde::Deserialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::error::{HolziError, Result};
use crate::instances::passphrase::Passphrase;
use crate::state::AppState;
use crate::sync::events::{self, LINK_JOIN_STATE_CHANGED};
use crate::sync::link::host_task::LinkHost;
use crate::sync::link::join_task::{JoinArgs, JoinConfig, LinkJoin};
use crate::sync::link::status::{LinkCodeInfo, LinkJoinState};
use crate::sync::registry::{SyncRegistry, SyncRuntime};

/// The running service of the open vault, or `NoActiveInstance`.
fn runtime(registry: &SyncRegistry) -> Result<Arc<SyncRuntime>> {
    registry.get().ok_or(HolziError::NoActiveInstance)
}

fn host(registry: &SyncRegistry) -> Result<LinkHost> {
    Ok(runtime(registry)?.link.clone())
}

/// Shows a new link code (main device only, `NotMainDevice` otherwise).
#[tauri::command]
pub async fn link_code_create(registry: State<'_, Arc<SyncRegistry>>) -> Result<LinkCodeInfo> {
    let runtime = runtime(&registry)?;
    runtime.link.create_code(&runtime).await
}

/// Withdraws the live code.
#[tauri::command]
pub async fn link_code_cancel(registry: State<'_, Arc<SyncRegistry>>) -> Result<()> {
    host(&registry)?.cancel();
    Ok(())
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct LinkConfirmArgs {
    /// Whether the new device also becomes a main device.
    pub as_main_device: bool,
}

/// The user's yes for the new device that proved the code.
#[tauri::command]
pub async fn link_confirm(
    registry: State<'_, Arc<SyncRegistry>>,
    args: LinkConfirmArgs,
) -> Result<()> {
    host(&registry)?.confirm(args.as_main_device)
}

/// The user's no.
#[tauri::command]
pub async fn link_reject(registry: State<'_, Arc<SyncRegistry>>) -> Result<()> {
    host(&registry)?.reject()
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct LinkJoinStartArgs {
    pub code: String,
    pub vault_name: String,
    pub device_name: String,
    #[ts(type = "string")]
    pub passphrase: Passphrase,
}

/// Starts joining a vault with a code; the vault is created here, with its
/// own passphrase, and only stays when the link finishes.
#[tauri::command]
pub async fn link_join_start(
    app: AppHandle,
    state: State<'_, AppState>,
    join: State<'_, LinkJoin>,
    args: LinkJoinStartArgs,
) -> Result<LinkJoinState> {
    state.gate().ensure_can_open()?;
    let emit_app = app.clone();
    let emit = Arc::new(move |state: &LinkJoinState| {
        events::emit(&emit_app, LINK_JOIN_STATE_CHANGED, state.clone());
    });
    join.start(
        &app,
        emit,
        JoinArgs {
            code: args.code,
            vault_name: args.vault_name,
            device_name: args.device_name,
            passphrase: args.passphrase,
        },
        JoinConfig::production(),
    )
    .await
}

/// Where the join stands.
#[tauri::command]
pub async fn link_join_status(join: State<'_, LinkJoin>) -> Result<LinkJoinState> {
    join.status().ok_or_else(|| HolziError::InvalidInput {
        reason: "no link was started".into(),
    })
}

/// Cancels the join; its vault is removed.
#[tauri::command]
pub async fn link_join_cancel(join: State<'_, LinkJoin>) -> Result<()> {
    join.cancel().await;
    Ok(())
}
