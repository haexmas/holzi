//! Provider commands: add / list / get / delete.
//!
//! This slice ships the schema and CRUD. Live provider invocation
//! (Anthropic/OpenAI HTTP, `cli_delegate` subprocess spawning) is a
//! later slice — a `local` provider row is enough to drive the chat
//! loop through the mistralrs wrapper from slice a.

pub mod local;

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::error::{HolziError, Result};
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::providers::{self as storage, Provider, ProviderKind};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProviderArgs {
    pub kind: ProviderKind,
    pub name: String,
    /// Base URL for `api_key` providers (e.g. `https://api.anthropic.com`).
    /// `None` for `local` and `cli_delegate`.
    pub base_url: Option<String>,
    /// API key for `api_key` providers. Sent as UTF-8; storage is opaque
    /// bytes. `None` for `local` and `cli_delegate`.
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPayload {
    pub id: Uuid,
    pub kind: ProviderKind,
    pub name: String,
    pub base_url: Option<String>,
    /// True when a credentials blob is present; the blob itself is
    /// never returned to the frontend.
    pub has_credentials: bool,
    pub created_at: i64,
}

impl From<Provider> for ProviderPayload {
    fn from(p: Provider) -> Self {
        ProviderPayload {
            id: p.id,
            kind: p.kind,
            name: p.name,
            base_url: p.base_url,
            has_credentials: p.credentials.is_some(),
            created_at: p.created_at,
        }
    }
}

/// Adds a provider. Validates minimally: `api_key` needs `base_url` and
/// a non-empty `api_key`; `local` needs neither. The frontend sees a
/// `Provider` without the credentials blob.
#[tauri::command]
pub async fn add_provider(
    state: State<'_, AppState>,
    args: AddProviderArgs,
) -> Result<ProviderPayload> {
    if args.name.trim().is_empty() {
        return Err(HolziError::InvalidInput {
            reason: "name is empty".into(),
        });
    }
    match args.kind {
        ProviderKind::ApiKey => {
            if args.base_url.as_deref().unwrap_or("").is_empty() {
                return Err(HolziError::InvalidInput {
                    reason: "api_key provider requires base_url".into(),
                });
            }
            if args.api_key.as_deref().unwrap_or("").is_empty() {
                return Err(HolziError::InvalidInput {
                    reason: "api_key provider requires api_key".into(),
                });
            }
        }
        ProviderKind::CliDelegate => {
            if args.base_url.as_deref().unwrap_or("").is_empty() {
                return Err(HolziError::InvalidInput {
                    reason: "cli_delegate provider requires base_url (cli command)".into(),
                });
            }
        }
        ProviderKind::Local => {
            // No requirements — one row per local runtime is enough.
        }
    }

    let db = active_database(&state)?;
    let now = now_ms();
    let provider = Provider {
        id: Uuid::new_v4(),
        kind: args.kind,
        name: args.name,
        base_url: args.base_url,
        credentials: args.api_key.map(|k| k.into_bytes()),
        created_at: now,
    };
    let inserted = provider.clone();
    let inserted_payload = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            storage::insert_provider(conn, &inserted).map_err(haex_crdt::Error::from)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("insert_provider join: {e}"),
    })?;
    inserted_payload.map_err(HolziError::from)?;
    Ok(provider.into())
}

/// Lists all providers configured on this instance.
#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderPayload>> {
    let db = active_database(&state)?;
    let rows = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| storage::list_providers(conn).map_err(haex_crdt::Error::from))
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("list_providers join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Deletes a provider by id. Callers (frontend) are responsible for
/// warning the operator when a provider still has referencing threads
/// or models — the schema does not enforce foreign keys.
#[tauri::command]
pub async fn delete_provider(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    let db = active_database(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            storage::delete_provider(conn, id).map_err(haex_crdt::Error::from)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("delete_provider join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
