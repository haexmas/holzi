//! `connect_cli_delegate`/`submit_cli_delegate_code` Tauri commands (spec
//! 007-cli-delegate US2) — acquires a `cli_delegate` credential without ever
//! touching host-level `claude`/`codex` login state (research.md §3/§5).
//!
//! The two vendors have different acquisition shapes (research.md §5):
//! `codex login --device-auth` is plain-text and completes end-to-end inside
//! one command; `claude setup-token` needs a real pty and pauses after
//! showing the OAuth URL, so a second command (`submit_cli_delegate_code`)
//! is needed to hand back the authorization code the user copies from the
//! browser.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::adapters::cli_delegate::connect_claude::{
    start_claude_connect, submit_claude_code, ClaudeConnectSession,
};
use crate::adapters::cli_delegate::connect_codex::run_device_auth;
use crate::adapters::cli_delegate::DelegateVendor;
use crate::error::{HolziError, Result};
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::providers::{self as storage, Provider, ProviderCapability, ProviderKind};

use super::{map_adapter_error, ProviderPayload};

pub const DELEGATE_CONNECT_PROGRESS_EVENT: &str = "delegate-connect-progress";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DelegateConnectProgress {
    vendor: &'static str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

fn emit_progress(app: &AppHandle, progress: DelegateConnectProgress) {
    if let Err(error) = app.emit(DELEGATE_CONNECT_PROGRESS_EVENT, progress) {
        log::warn!("emit {DELEGATE_CONNECT_PROGRESS_EVENT} failed: {error}");
    }
}

/// Holds a paused Claude connect flow between `connect_cli_delegate` and
/// `submit_cli_delegate_code` (contracts/tauri-commands.md). Only one flow
/// can be pending at a time; starting a new `connect_cli_delegate(claude)`
/// call implicitly abandons any previous unsubmitted flow — its PTY child is
/// killed by `ClaudeConnectSession`'s `Drop`.
#[derive(Default)]
pub struct DelegateConnectState {
    pending_claude: AsyncMutex<Option<ClaudeConnectSession>>,
}

impl DelegateConnectState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectCliDelegateArgs {
    pub vendor: String,
    pub name: String,
}

/// `rename_all = "snake_case"` (not `camelCase`) so the `status` tag value
/// matches `delegate-connect-progress`'s own snake_case status vocabulary
/// (`awaiting_code`, `awaiting_browser`, `success`) — `AwaitingCode` would
/// otherwise serialize its tag as `"awaitingCode"`, inconsistent with the
/// event and with contracts/tauri-commands.md.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ConnectCliDelegateResult {
    AwaitingCode { vendor: &'static str },
    Connected { provider: ProviderPayload },
}

/// Starts the connect flow (contracts/tauri-commands.md). For `codex` this
/// runs the whole flow and returns `Connected`; for `claude` it returns as
/// soon as the OAuth URL is known, and the flow is completed by a later
/// `submit_cli_delegate_code` call.
#[tauri::command]
pub async fn connect_cli_delegate(
    app: AppHandle,
    state: State<'_, AppState>,
    connect_state: State<'_, DelegateConnectState>,
    args: ConnectCliDelegateArgs,
) -> Result<ConnectCliDelegateResult> {
    let vendor = DelegateVendor::parse(&args.vendor).ok_or_else(|| HolziError::InvalidInput {
        reason: format!("unknown cli_delegate vendor \"{}\"", args.vendor),
    })?;

    match vendor {
        DelegateVendor::Claude => {
            let (session, url) = start_claude_connect("claude")
                .await
                .map_err(map_adapter_error)?;
            *connect_state.pending_claude.lock().await = Some(session);
            emit_progress(
                &app,
                DelegateConnectProgress {
                    vendor: "claude",
                    status: "awaiting_code",
                    url: Some(url),
                    code: None,
                    message: None,
                },
            );
            Ok(ConnectCliDelegateResult::AwaitingCode { vendor: "claude" })
        }
        DelegateVendor::Codex => {
            let app_for_prompt = app.clone();
            let credentials = run_device_auth("codex", move |prompt| {
                emit_progress(
                    &app_for_prompt,
                    DelegateConnectProgress {
                        vendor: "codex",
                        status: "awaiting_browser",
                        url: Some(prompt.url.clone()),
                        code: Some(prompt.code.clone()),
                        message: None,
                    },
                );
            })
            .await
            .map_err(map_adapter_error)?;

            let provider =
                upsert_delegate_provider(&state, "codex", args.name, credentials).await?;
            emit_progress(
                &app,
                DelegateConnectProgress {
                    vendor: "codex",
                    status: "success",
                    url: None,
                    code: None,
                    message: None,
                },
            );
            Ok(ConnectCliDelegateResult::Connected {
                provider: provider.into(),
            })
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitCliDelegateCodeArgs {
    pub code: String,
    pub name: String,
}

/// Completes a Claude connect flow started by `connect_cli_delegate`
/// (contracts/tauri-commands.md). A failure (e.g. a rejected code) leaves the
/// flow open for another attempt rather than tearing it down, since the
/// underlying `claude setup-token` process commonly re-prompts for the code
/// on a bad entry instead of exiting.
#[tauri::command]
pub async fn submit_cli_delegate_code(
    app: AppHandle,
    state: State<'_, AppState>,
    connect_state: State<'_, DelegateConnectState>,
    args: SubmitCliDelegateCodeArgs,
) -> Result<ProviderPayload> {
    let mut guard = connect_state.pending_claude.lock().await;
    let mut session = guard.take().ok_or_else(|| HolziError::InvalidInput {
        reason: "no claude connect flow is currently awaiting a code".into(),
    })?;

    let token = match submit_claude_code(&mut session, &args.code).await {
        Ok(token) => token,
        Err(error) => {
            *guard = Some(session);
            return Err(map_adapter_error(error));
        }
    };
    drop(guard);

    let provider = upsert_delegate_provider(&state, "claude", args.name, token).await?;
    emit_progress(
        &app,
        DelegateConnectProgress {
            vendor: "claude",
            status: "success",
            url: None,
            code: None,
            message: None,
        },
    );
    Ok(provider.into())
}

/// Inserts or, if a `cli_delegate` row already exists for `vendor`, updates
/// its credentials in place (upsert semantics, contracts/tauri-commands.md —
/// FR-014 reconnect is the same command pair, not a separate route). Refresh
/// failures are logged, not surfaced — matches `add_provider`'s existing
/// "insert always succeeds, refresh is best-effort" behavior.
async fn upsert_delegate_provider(
    state: &State<'_, AppState>,
    vendor: &str,
    name: String,
    credentials: Vec<u8>,
) -> Result<Provider> {
    let db = active_database(state)?;
    let vendor_owned = vendor.to_string();
    let db_write = db.clone();
    let provider = tauri::async_runtime::spawn_blocking(move || {
        db_write.with_connection(|conn| {
            let existing = storage::find_cli_delegate_provider(conn, &vendor_owned)
                .map_err(haex_crdt::Error::from)?;
            match existing {
                Some(id) => {
                    storage::update_credentials(conn, id, &credentials)
                        .map_err(haex_crdt::Error::from)?;
                    storage::get_provider(conn, id).map_err(haex_crdt::Error::from)
                }
                None => {
                    let provider = Provider {
                        id: Uuid::new_v4(),
                        kind: ProviderKind::CliDelegate,
                        adapter: Some(vendor_owned.clone()),
                        name,
                        base_url: Some(vendor_owned),
                        credentials: Some(credentials),
                        created_at: super::now_ms(),
                        capability: ProviderCapability::Chat,
                    };
                    storage::insert_provider(conn, &provider).map_err(haex_crdt::Error::from)?;
                    Ok(Some(provider))
                }
            }
        })
    })
    .await
    .map_err(|error| HolziError::CrdtInit {
        reason: format!("cli_delegate provider upsert join: {error}"),
    })?
    .map_err(HolziError::from)?
    .ok_or_else(|| HolziError::CrdtInit {
        reason: "cli_delegate provider vanished immediately after being written".into(),
    })?;

    if let Err(error) = super::do_refresh(&db, &provider).await {
        log::warn!(
            "cli_delegate model refresh after connect failed: {}",
            super::format_holzi_error(&error)
        );
    }

    Ok(provider)
}
