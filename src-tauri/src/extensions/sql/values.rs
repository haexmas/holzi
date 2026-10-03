//! Values between the bridge (JSON) and SQLite (research R7): parameters `null`, numbers, text,
//! `true`/`false` → 1/0, `{"$bytes": "<base64>"}` → BLOB, other arrays and objects → JSON text;
//! results INTEGER → number, REAL → number (NaN and ±∞ → `null`), TEXT → string, BLOB → base64.

use base64::Engine;
use haex_crdt::rusqlite::types::{Value as SqlValue, ValueRef};
use serde_json::{Number, Value};

use super::parse::violation;
use crate::extensions::error::{BridgeError, ExtensionErrorCode};

fn invalid(message: &str) -> BridgeError {
    BridgeError::new(ExtensionErrorCode::Validation, message)
}

/// One parameter.
pub fn to_sql(value: &Value) -> Result<SqlValue, BridgeError> {
    Ok(match value {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => SqlValue::Integer(i),
            (None, Some(f)) => SqlValue::Real(f),
            (None, None) => return Err(invalid("number out of range")),
        },
        Value::String(s) => SqlValue::Text(s.clone()),
        Value::Object(map) if map.len() == 1 && map.contains_key("$bytes") => {
            let encoded = map["$bytes"]
                .as_str()
                .ok_or_else(|| invalid("$bytes must be base64 text"))?;
            SqlValue::Blob(
                base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|_| invalid("$bytes must be base64 text"))?,
            )
        }
        other => SqlValue::Text(other.to_string()),
    })
}

/// The parameters of a call: an array, or nothing.
pub fn params(value: Option<&Value>) -> Result<Vec<SqlValue>, BridgeError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items.iter().map(to_sql).collect(),
        Some(_) => Err(invalid("params must be an array")),
    }
}

/// One result value and its size for the response limit.
pub fn to_json(value: ValueRef<'_>) -> (Value, usize) {
    match value {
        ValueRef::Null => (Value::Null, 4),
        ValueRef::Integer(i) => (Value::from(i), 20),
        ValueRef::Real(f) => (Number::from_f64(f).map_or(Value::Null, Value::Number), 24),
        ValueRef::Text(bytes) => (
            Value::String(String::from_utf8_lossy(bytes).into_owned()),
            bytes.len() + 2,
        ),
        ValueRef::Blob(bytes) => {
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            let size = encoded.len() + 2;
            (Value::String(encoded), size)
        }
    }
}

/// Refuses parameters in an unexpected shape for a statement entry `[sql, params]`.
pub fn statement_entry(entry: &Value) -> Result<(&str, Option<&Value>), BridgeError> {
    match entry {
        Value::Array(parts) if !parts.is_empty() && parts.len() <= 2 => {
            let sql = parts[0]
                .as_str()
                .ok_or_else(|| violation("a statement must be SQL text"))?;
            Ok((sql, parts.get(1)))
        }
        _ => Err(invalid("statements must be [sql, params] pairs")),
    }
}

#[cfg(test)]
#[path = "values_tests.rs"]
mod tests;
