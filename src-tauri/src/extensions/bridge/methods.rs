//! The bridge methods of L1 that are not about data: context, own info, tab attention.

use serde_json::{json, Value};

use super::dispatch::CallContext;
use crate::extensions::bundle::Manifest;
use crate::extensions::error::{BridgeError, ExtensionErrorCode};
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
