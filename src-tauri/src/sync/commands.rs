//! Tauri commands of the own-device sync (spec 024, contracts/
//! tauri-commands.md): linking a device. The main device's commands need the
//! running sync service of the open vault; the join commands run on the
//! start page, with no vault open.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::error::{HolziError, Result};
use crate::instances::passphrase::Passphrase;
use crate::instances::paths::get_app_local_data;
use crate::state::AppState;
use crate::sync::device_list;
use crate::sync::events::{self, LINK_JOIN_STATE_CHANGED};
use crate::sync::keys;
use crate::sync::link::host_task::LinkHost;
use crate::sync::link::join_task::{JoinArgs, JoinConfig, LinkJoin};
use crate::sync::link::status::{LinkCodeInfo, LinkJoinState, LinkingStatus};
use crate::sync::registry::{SyncRegistry, SyncRuntime};
use crate::sync::servers;

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

/// What this device is in the vault (spec 024, FR-034).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "snake_case")]
pub enum ThisDevice {
    Main,
    Linked,
    /// Not on the device list yet: a copy of the vault waits for a main
    /// device to admit it (FR-044).
    AwaitingAdmission,
    /// The list says it was removed and it no longer syncs (FR-027).
    Removed,
}

/// One open request to join the vault (FR-045).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct OpenAdmission {
    pub device_pubkey: String,
    pub name: String,
    #[ts(type = "number")]
    pub requested_at: u64,
}

/// What the device list view needs besides the devices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub this_device: ThisDevice,
    pub open_admissions: Vec<OpenAdmission>,
    pub linking: Option<LinkingStatus>,
}

/// The public key of the vault identity (FR-046).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct VaultPublicIdentity {
    /// Nostr `npub` form, the address others invite the vault at.
    pub npub: String,
    pub hex: String,
}

/// The servers devices find each other through (FR-008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SyncServers {
    pub nostr_relays: Vec<String>,
    pub iroh_relays: Vec<String>,
}

/// Reads this device's place in the vault from the lists and the vault
/// secret.
pub fn this_device(
    q: &mut impl crate::storage::query::Query,
    own: &[u8; 32],
) -> haex_crdt::Result<ThisDevice> {
    let vault = keys::vault_pubkey(q)?.unwrap_or([0; 32]);
    let valid = device_list::valid_lists(&device_list::load_all(q)?, &vault);
    let Some(effective) = device_list::effective(&valid) else {
        return Ok(ThisDevice::AwaitingAdmission);
    };
    if effective.list.removes(own) {
        return Ok(ThisDevice::Removed);
    }
    Ok(match effective.list.device(own) {
        Some(device) if device.role == device_list::Role::Main => ThisDevice::Main,
        Some(_) => ThisDevice::Linked,
        None => ThisDevice::AwaitingAdmission,
    })
}

/// Reads the vault's public identity as `npub` and hex.
pub fn public_identity(
    q: &mut impl crate::storage::query::Query,
) -> haex_crdt::Result<Option<VaultPublicIdentity>> {
    use nostr::nips::nip19::ToBech32;
    let Some(pubkey) = keys::vault_pubkey(q)? else {
        return Ok(None);
    };
    let key = nostr::key::PublicKey::from_slice(&pubkey)
        .map_err(|e| haex_crdt::Error::consumer(e.to_string()))?;
    let npub = key
        .to_bech32()
        .map_err(|e| haex_crdt::Error::consumer(e.to_string()))?;
    Ok(Some(VaultPublicIdentity {
        npub,
        hex: keys::hex(&pubkey),
    }))
}

/// Who is this device, what waits for an answer, and is a link going on.
#[tauri::command]
pub async fn sync_status(
    app: AppHandle,
    state: State<'_, AppState>,
    registry: State<'_, Arc<SyncRegistry>>,
) -> Result<SyncStatus> {
    let installation_id_file = crate::identity::installation_id_path(&get_app_local_data(&app)?);
    let installation = crate::identity::read_or_mint_installation_uuid(&installation_id_file)
        .map_err(HolziError::from)?;
    let db = crate::state_utils::active_database(&state)?;
    let this = db
        .read(move |r| {
            Ok(match keys::load_device_keys(r, installation)? {
                Some(own) => this_device(r, &own.device_pubkey)?,
                None => ThisDevice::AwaitingAdmission,
            })
        })
        .await?;
    Ok(SyncStatus {
        this_device: this,
        // Admission requests arrive with user story 7.
        open_admissions: Vec::new(),
        linking: registry.get().and_then(|runtime| runtime.link.status()),
    })
}

/// The public key of the vault identity; `NoActiveInstance` without an open
/// vault, an error when the vault has none yet.
#[tauri::command]
pub async fn vault_public_identity(state: State<'_, AppState>) -> Result<VaultPublicIdentity> {
    let db = crate::state_utils::active_database(&state)?;
    db.read(|r| public_identity(r))
        .await?
        .ok_or_else(|| HolziError::InvalidInput {
            reason: "the vault has no identity yet".into(),
        })
}

/// The servers as stored; empty lists mean the built-in defaults.
#[tauri::command]
pub async fn sync_servers_get(state: State<'_, AppState>) -> Result<SyncServers> {
    let db = crate::state_utils::active_database(&state)?;
    let config = db.read(|r| servers::read(r)).await?;
    Ok(SyncServers {
        nostr_relays: config.nostr_relays,
        iroh_relays: config.iroh_relays,
    })
}

/// Stores the servers. Empty lists go back to the defaults. The iroh relays
/// apply at once; the Nostr relays when the vault is opened next.
#[tauri::command]
pub async fn sync_servers_set(
    state: State<'_, AppState>,
    registry: State<'_, Arc<SyncRegistry>>,
    args: SyncServers,
) -> Result<()> {
    servers::validate(&args.nostr_relays, &args.iroh_relays)
        .map_err(|reason| HolziError::InvalidInput { reason })?;
    let config = servers::ServerConfig {
        nostr_relays: args.nostr_relays.clone(),
        iroh_relays: args.iroh_relays.clone(),
    };
    let db = crate::state_utils::active_database(&state)?;
    db.write(move |tx| servers::write(tx, &args.nostr_relays, &args.iroh_relays))
        .await?;
    if let Some(runtime) = registry.get() {
        runtime.node.apply_relays(&config.relay_mode()).await;
    }
    Ok(())
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
