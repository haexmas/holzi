//! `extension_frame_open`, `extension_frame_close` and `extension_bridge_call`
//! (contracts/tauri-commands.md §Rahmen).

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::extensions::bridge::dispatch::{answer, call, CallContext, Emit};
use crate::extensions::error::{BridgeError, ExtensionErrorCode};
use crate::extensions::protocol::{encode_path, prefix};
use crate::extensions::registry::start::start;
use crate::passwords::clock::unix_millis;
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::wm_session_commands::current_device_uuid;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct FrameOpened {
    /// Id of the frame session for bridge calls; never handed to the extension.
    pub frame: String,
    /// URL of the entry page with the start token.
    pub url: String,
}

/// Starts the extension on this device if needed and opens a frame session for one tab.
#[tauri::command]
pub async fn extension_frame_open(
    app: AppHandle,
    state: State<'_, AppState>,
    extension_id: String,
    tab_id: String,
) -> Result<FrameOpened> {
    let extension_id = Uuid::parse_str(&extension_id).map_err(|_| HolziError::ExtensionNotFound)?;
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    let started = tauri::async_runtime::spawn_blocking(move || {
        start(
            &db,
            extension_id,
            device,
            unix_millis(std::time::SystemTime::now()),
        )
    })
    .await
    .map_err(|e| HolziError::ExtensionNotReady {
        status: format!("start task: {e}"),
    })??;
    let host = state.extensions();
    let started = host.remember_started(started);
    let session = host.frames.open(extension_id, started.bundle_id, &tab_id);
    Ok(FrameOpened {
        frame: session.frame.clone(),
        url: format!(
            "{}{}?hf={}",
            prefix(extension_id),
            encode_path(&started.entry),
            session.token
        ),
    })
}

/// Ends a frame session.
#[tauri::command]
pub async fn extension_frame_close(
    app: AppHandle,
    state: State<'_, AppState>,
    frame: String,
) -> Result<()> {
    let host = state.extensions();
    host.frames.close(&frame);
    host.drop_dialogs_of(&frame);
    // Questions nobody waits for any more disappear from holzi's window.
    let emitter = WindowEmitter(app);
    for request_id in host.permissions.frame_closed(&frame) {
        emitter.emit(
            crate::extensions::bridge::permissions::PERMISSION_REQUEST_CANCELLED,
            serde_json::json!({ "requestId": request_id }),
        );
    }
    Ok(())
}

/// holzi's window answers a dialog an extension asked for (`extension_dialog_confirm`).
#[tauri::command]
pub async fn extension_dialog_resolve(
    state: State<'_, AppState>,
    request_id: String,
    confirmed: bool,
) -> Result<()> {
    state.extensions().resolve_dialog(&request_id, confirmed);
    Ok(())
}

/// Hands holzi's events to its own window.
pub(crate) struct WindowEmitter(pub(crate) AppHandle);

impl Emit for WindowEmitter {
    fn emit(&self, event: &str, payload: Value) {
        if let Err(error) = self.0.emit(event, payload) {
            log::warn!("{event} not delivered: {error}");
        }
    }
}

/// Relays one request of an extension frame (contracts/bridge.md §Weiterleitung). Always answers
/// in the SDK form `{id, result}` or `{id, error}`; an unknown frame is a security violation.
#[tauri::command]
pub async fn extension_bridge_call(
    app: AppHandle,
    state: State<'_, AppState>,
    frame: String,
    id: Value,
    method: String,
    params: Option<Value>,
) -> Result<Value> {
    let host = state.extensions();
    let Some(session) = host.frames.get(&frame) else {
        return Ok(answer(
            id,
            Err(BridgeError::new(
                ExtensionErrorCode::SecurityViolation,
                "unknown frame",
            )),
        ));
    };
    let db = active_database(&state)?;
    let device = current_device_uuid(&app, &db)?;
    let ctx = CallContext {
        db,
        host,
        session,
        device,
        emitter: Arc::new(WindowEmitter(app.clone())),
    };
    let params = params.unwrap_or(Value::Null);
    let outcome = tauri::async_runtime::spawn_blocking(move || call(&ctx, &method, &params))
        .await
        .unwrap_or_else(|e| {
            Err(BridgeError::new(
                ExtensionErrorCode::Database,
                format!("bridge task: {e}"),
            ))
        });
    Ok(answer(id, outcome))
}

/// holzi's window reports its color scheme and language (for `extension_context_get`). Only
/// holzi's own window can call this; an extension reaches nothing but `extension_bridge_call`.
#[tauri::command]
pub async fn extension_host_context_set(
    state: State<'_, AppState>,
    theme: String,
    locale: String,
) -> Result<()> {
    state.extensions().set_context(&theme, &locale);
    Ok(())
}
