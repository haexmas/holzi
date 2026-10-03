//! The bridge methods of L1 that are not about data: context, own info, tab attention.

use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

use super::dispatch::CallContext;
use crate::extensions::bundle::Manifest;
use crate::extensions::error::{BridgeError, ExtensionErrorCode};
use crate::extensions::protocol::token;
use crate::storage::query::Query;

pub const MODULE: &str = module_path!();

/// The platform names of the SDK's `ApplicationContext`.
fn platform() -> Option<&'static str> {
    match std::env::consts::OS {
        os @ ("linux" | "macos" | "ios" | "freebsd" | "dragonfly" | "netbsd" | "openbsd"
        | "solaris" | "android" | "windows") => Some(os),
        _ => None,
    }
}

/// `{theme, locale, platform, deviceId}`.
pub fn context_get(ctx: &CallContext, _params: &Value) -> Result<Value, BridgeError> {
    let context = ctx.host.context();
    Ok(json!({
        "theme": context.theme,
        "locale": context.locale,
        "platform": platform(),
        "deviceId": ctx.device.to_string(),
    }))
}

/// `{publicKey, name, version, displayName}` of the calling extension only.
pub fn get_info(ctx: &CallContext, _params: &Value) -> Result<Value, BridgeError> {
    let bundle_id = ctx.session.bundle_id.to_string();
    let manifest_json = ctx
        .db
        .read_blocking(move |q| {
            q.query_row(
                "SELECT manifest_json FROM extension_bundles WHERE id = ?1",
                &[&bundle_id],
                |r| r.get::<_, Vec<u8>>(0),
            )
        })
        .map_err(|_| BridgeError::new(ExtensionErrorCode::Database, "database unavailable"))?
        .ok_or_else(|| BridgeError::new(ExtensionErrorCode::NotFound, "not found"))?;
    let manifest = Manifest::from_stored(&manifest_json)
        .map_err(|_| BridgeError::new(ExtensionErrorCode::Manifest, "manifest unreadable"))?;
    Ok(json!({
        "publicKey": manifest.public_key.as_str(),
        "name": manifest.name.as_str(),
        "version": manifest.version.to_string(),
        "displayName": manifest.title(),
    }))
}

/// `{active}`: marks or unmarks the calling frame's own tab (event `extension-tab-attention`).
pub fn tab_attention(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let active = params
        .get("active")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            BridgeError::new(ExtensionErrorCode::Validation, "active must be a boolean")
        })?;
    ctx.emitter.emit(
        "extension-tab-attention",
        json!({ "frame": ctx.session.frame, "active": active }),
    );
    Ok(Value::Null)
}

/// Longest dialog text and longest title or button label (characters).
const MAX_DIALOG_MESSAGE: usize = 2000;
const MAX_DIALOG_LABEL: usize = 200;
/// A dialog nobody answers ends as "cancel", so the call never waits forever.
const DIALOG_WAIT: Duration = Duration::from_secs(300);

fn optional_text(params: &Value, key: &str) -> Result<Option<String>, BridgeError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.chars().count() <= MAX_DIALOG_LABEL => {
            Ok(Some(text.clone()))
        }
        Some(_) => Err(BridgeError::new(
            ExtensionErrorCode::Validation,
            format!("{key} must be a string of at most {MAX_DIALOG_LABEL} characters"),
        )),
    }
}

/// `{message, title?, confirmLabel?, cancelLabel?, destructive?}` → `true` or `false`: holzi asks
/// the user over the calling frame's own tab (replaces `confirm()`, which the sandbox blocks;
/// research R12, T045). Blocks this call until the answer, the frame's end or [`DIALOG_WAIT`].
pub fn dialog_confirm(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let message = params
        .get("message")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty() && m.chars().count() <= MAX_DIALOG_MESSAGE)
        .ok_or_else(|| {
            BridgeError::new(
                ExtensionErrorCode::Validation,
                format!("message must be a string of 1 to {MAX_DIALOG_MESSAGE} characters"),
            )
        })?;
    let title = optional_text(params, "title")?;
    let confirm_label = optional_text(params, "confirmLabel")?;
    let cancel_label = optional_text(params, "cancelLabel")?;
    let destructive = params.get("destructive").and_then(Value::as_bool) == Some(true);

    let request_id = token::mint();
    let frame = &ctx.session.frame;
    let (answer, waiting) = mpsc::channel();
    if !ctx.host.open_dialog(&request_id, frame, answer) {
        return Err(BridgeError::new(
            ExtensionErrorCode::LimitExceeded,
            "a dialog of this frame is already open",
        ));
    }
    ctx.emitter.emit(
        "extension-dialog-request",
        json!({
            "requestId": request_id,
            "frame": frame,
            "message": message,
            "title": title,
            "confirmLabel": confirm_label,
            "cancelLabel": cancel_label,
            "destructive": destructive,
        }),
    );
    let confirmed = waiting.recv_timeout(DIALOG_WAIT).unwrap_or(false);
    // After a timeout the dialog is still registered; answering it later must not reach anyone.
    ctx.host.resolve_dialog(&request_id, false);
    Ok(Value::Bool(confirmed))
}
