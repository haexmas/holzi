//! Tauri command surface for `preferences`. See spec 002
//! `contracts/tauri-commands.md` for the wire shapes.
//!
//! `PrefScopeWire` is a wire-side discriminated union that maps to the
//! Rust `PrefScope` enum. Frontend sends
//! `{ kind: 'vault' } | { kind: 'device', uuid }`; the Rust side folds
//! it back into the storage enum.

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::preferences::{self, PrefError, PrefScope};

/// Wire representation of `PrefScope`. Symmetric with the TypeScript
/// `PrefScope` type in `usePreferences.ts`.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PrefScopeWire {
    Vault,
    Device { uuid: Uuid },
}

impl TryFrom<PrefScopeWire> for PrefScope {
    type Error = HolziError;

    fn try_from(value: PrefScopeWire) -> std::result::Result<Self, Self::Error> {
        let scope = match value {
            PrefScopeWire::Vault => PrefScope::Vault,
            PrefScopeWire::Device { uuid } => PrefScope::Device(uuid),
        };
        preferences::validate_scope(scope).map_err(pref_error_to_holzi)?;
        Ok(scope)
    }
}

fn pref_error_to_holzi(err: PrefError) -> HolziError {
    HolziError::InvalidInput {
        reason: err.to_string(),
    }
}

fn validate_key(key: &str) -> Result<()> {
    preferences::validate_key(key).map_err(pref_error_to_holzi)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPrefArgs {
    pub scope: PrefScopeWire,
    pub key: String,
}

/// Reads a single preference. `None` covers both "row absent" and
/// "row present with SQL NULL value" (data-model.md).
#[tauri::command]
pub async fn get_pref(state: State<'_, AppState>, args: GetPrefArgs) -> Result<Option<String>> {
    validate_key(&args.key)?;
    let scope = PrefScope::try_from(args.scope)?;
    let key = args.key;
    let db = active_database(&state)?;
    let value = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            preferences::get(conn, scope, &key).map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("get_pref join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPrefArgs {
    pub scope: PrefScopeWire,
    pub key: String,
    pub value: String,
}

/// Inserts or updates a preference row.
#[tauri::command]
pub async fn set_pref(state: State<'_, AppState>, args: SetPrefArgs) -> Result<()> {
    validate_key(&args.key)?;
    let scope = PrefScope::try_from(args.scope)?;
    let SetPrefArgs { key, value, .. } = args;
    let db = active_database(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            preferences::insert_or_update(conn, scope, &key, &value)
                .map_err(haex_crdt::Error::from)
                .map(|_| ())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("set_pref join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearPrefArgs {
    pub scope: PrefScopeWire,
    pub key: String,
}

/// Deletes a preference row. Idempotent — deleting an absent row is
/// not an error.
#[tauri::command]
pub async fn clear_pref(state: State<'_, AppState>, args: ClearPrefArgs) -> Result<()> {
    validate_key(&args.key)?;
    let scope = PrefScope::try_from(args.scope)?;
    let key = args.key;
    let db = active_database(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            preferences::delete(conn, scope, &key)
                .map_err(haex_crdt::Error::from)
                .map(|_| ())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("clear_pref join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(())
}
