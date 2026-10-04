//! The key-value store of an extension (spec 017, US5, T085, contracts/bridge.md
//! `extension_web_storage_*`): the SDK's `client.storage`, like `localStorage`. Every row belongs to
//! this device and the calling frame's extension (`extension_kv`, ADR-0001): an extension never
//! sees another extension's values or those of another own device. The rows are vault data, so a
//! value survives a restart; removing the extension deletes them (research R11). A development
//! version keeps its values on this device only (US12).

use haex_crdt::rusqlite::params;
use serde_json::{json, Value};

use crate::extensions::bridge::dispatch::CallContext;
use crate::extensions::bridge::frames::FrameSource;
use crate::extensions::error::{BridgeError, ExtensionErrorCode};
use crate::storage::query::Query;

pub const MODULE: &str = module_path!();

/// Bytes of one key.
pub const MAX_KEY_BYTES: usize = 1024;
/// Bytes of one value.
pub const MAX_VALUE_BYTES: usize = 1024 * 1024;
/// Bytes of every key and value of one extension on one device together.
pub const MAX_TOTAL_BYTES: usize = 10 * 1024 * 1024;

fn unavailable() -> BridgeError {
    BridgeError::new(ExtensionErrorCode::Database, "database unavailable")
}

fn text<'a>(params: &'a Value, name: &str) -> Result<&'a str, BridgeError> {
    params.get(name).and_then(Value::as_str).ok_or_else(|| {
        BridgeError::new(
            ExtensionErrorCode::Validation,
            format!("{name} must be a string"),
        )
    })
}

fn key(params: &Value) -> Result<String, BridgeError> {
    let key = text(params, "key")?;
    if key.len() > MAX_KEY_BYTES {
        return Err(BridgeError::new(
            ExtensionErrorCode::LimitExceeded,
            "key too long",
        ));
    }
    Ok(key.to_owned())
}

/// The caller's extension and this device, as stored in `extension_kv`.
fn owner(ctx: &CallContext) -> (String, String) {
    (ctx.session.extension_id.to_string(), ctx.device.to_string())
}

/// Where the caller's values live. Every statement binds `?1` device, `?2` extension; a
/// development version (US12) keeps them in `dev_extension_kv_no_sync` on this device, which has
/// no device column.
struct Store {
    table: &'static str,
    owner: &'static str,
    insert: &'static str,
}

fn store(ctx: &CallContext) -> Store {
    match ctx.session.source {
        FrameSource::Bundle(_) => Store {
            table: "extension_kv",
            owner: "vault_device_uuid = ?1 AND extension_id = ?2",
            insert: "INSERT INTO extension_kv (vault_device_uuid, extension_id, key, value) \
                     VALUES (?1, ?2, ?3, ?4)",
        },
        FrameSource::DevServer => Store {
            table: "dev_extension_kv_no_sync",
            owner: "extension_id = ?2",
            insert: "INSERT INTO dev_extension_kv_no_sync (extension_id, key, value) \
                     VALUES (?2, ?3, ?4)",
        },
    }
}

/// `{key}` → the value, or `null`.
pub fn get_item(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let key = key(params)?;
    let (ext, device) = owner(ctx);
    let Store { table, owner, .. } = store(ctx);
    let value = ctx
        .db
        .read_blocking(move |q| {
            q.query_row(
                &format!("SELECT value FROM {table} WHERE {owner} AND key = ?3"),
                params![device, ext, key],
                |r| r.get::<_, String>(0),
            )
        })
        .map_err(|_| unavailable())?;
    Ok(value.map_or(Value::Null, Value::String))
}

/// `{key, value}`: stores the value; refused (7000) beyond the limits of one value or of the
/// whole store.
pub fn set_item(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let key = key(params)?;
    let value = text(params, "value")?.to_owned();
    if value.len() > MAX_VALUE_BYTES {
        return Err(BridgeError::new(
            ExtensionErrorCode::LimitExceeded,
            "value too large",
        ));
    }
    let (ext, device) = owner(ctx);
    let Store {
        table,
        owner,
        insert,
    } = store(ctx);
    let stored = ctx
        .db
        .write_blocking(move |tx| {
            // Every other key and value of this extension on this device, in bytes.
            let others = tx
                .query_row(
                    &format!(
                        "SELECT COALESCE(SUM(length(CAST(key AS BLOB)) + \
                         length(CAST(value AS BLOB))), 0) FROM {table} WHERE {owner} AND key <> ?3"
                    ),
                    params![device, ext, key],
                    |r| r.get::<_, i64>(0),
                )?
                .unwrap_or(0);
            let total = usize::try_from(others)
                .unwrap_or(usize::MAX)
                .saturating_add(key.len())
                .saturating_add(value.len());
            if total > MAX_TOTAL_BYTES {
                return Ok(false);
            }
            let exists = tx
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {owner} AND key = ?3"),
                    params![device, ext, key],
                    |r| r.get::<_, i64>(0),
                )?
                .unwrap_or(0)
                > 0;
            if exists {
                tx.execute(
                    &format!(
                        "UPDATE {table} SET value = ?4 WHERE {owner} AND key = ?3 AND value <> ?4"
                    ),
                    params![device, ext, key, value],
                )?;
            } else {
                tx.execute(insert, params![device, ext, key, value])?;
            }
            Ok(true)
        })
        .map_err(|_| unavailable())?;
    if !stored {
        return Err(BridgeError::new(
            ExtensionErrorCode::LimitExceeded,
            "storage full",
        ));
    }
    Ok(Value::Null)
}

/// `{key}`: removes the value, if any.
pub fn remove_item(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let key = key(params)?;
    let (ext, device) = owner(ctx);
    let Store { table, owner, .. } = store(ctx);
    ctx.db
        .write_blocking(move |tx| {
            tx.execute(
                &format!("DELETE FROM {table} WHERE {owner} AND key = ?3"),
                params![device, ext, key],
            )
            .map(drop)
        })
        .map_err(|_| unavailable())?;
    Ok(Value::Null)
}

/// Removes every value of the caller on this device.
pub fn clear(ctx: &CallContext, _params: &Value) -> Result<Value, BridgeError> {
    let (ext, device) = owner(ctx);
    let Store { table, owner, .. } = store(ctx);
    ctx.db
        .write_blocking(move |tx| {
            tx.execute(
                &format!("DELETE FROM {table} WHERE {owner}"),
                params![device, ext],
            )
            .map(drop)
        })
        .map_err(|_| unavailable())?;
    Ok(Value::Null)
}

/// Every key of the caller on this device, sorted.
pub fn keys(ctx: &CallContext, _params: &Value) -> Result<Value, BridgeError> {
    let (ext, device) = owner(ctx);
    let Store { table, owner, .. } = store(ctx);
    let keys: Vec<String> = ctx
        .db
        .read_blocking(move |q| {
            q.query_map(
                &format!("SELECT key FROM {table} WHERE {owner} ORDER BY key"),
                params![device, ext],
                |r| r.get(0),
            )
        })
        .map_err(|_| unavailable())?;
    Ok(json!(keys))
}

#[cfg(test)]
#[path = "kv_tests.rs"]
mod tests;
