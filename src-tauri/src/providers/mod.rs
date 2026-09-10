//! Provider commands: add / list / get / delete + refresh model listing.
//!
//! CRUD is provider-kind-agnostic; the refresh path dispatches on
//! the persisted adapter discriminator to build the right adapter. Live invocation for
//! `cli_delegate` still awaits its own design pass — refresh returns
//! an error until then.

pub mod local;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

use crate::adapters::anthropic::AnthropicAdapter;
use crate::adapters::{AdapterError, ProviderAdapter, ProviderModel};
use crate::error::{HolziError, Result};
use crate::state::AppState;
use crate::state_utils::active_database;
use crate::storage::models::{self as models_store, ModelRow};
use crate::storage::providers::{self as storage, Provider, ProviderKind};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProviderArgs {
    pub kind: ProviderKind,
    pub name: String,
    /// Adapter/vendor for `api_key` providers. Only `anthropic` is currently
    /// supported; the field is required so refresh cannot guess a protocol.
    #[serde(default)]
    pub adapter: Option<String>,
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
    pub adapter: Option<String>,
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
            adapter: p.adapter,
            name: p.name,
            base_url: p.base_url,
            has_credentials: p.credentials.is_some(),
            created_at: p.created_at,
        }
    }
}

/// Return value of [`add_provider`]. Carries the created provider plus
/// an optional `refresh_error` set when the auto-refresh triggered
/// after insert failed. The add itself always succeeds when the return
/// is `Ok(_)` — a refresh failure never rolls back the row so the
/// operator can retry manually with a fixed key.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProviderResult {
    pub provider: ProviderPayload,
    pub model_count: Option<usize>,
    pub refresh_error: Option<String>,
}

/// Adds a provider. API-key providers require a supported adapter,
/// `base_url`, and a non-empty `api_key`; `local` needs neither. For
/// `api_key` providers the model catalog is refreshed inline; if that call
/// fails the provider is kept and `refresh_error` is set so the frontend can
/// toast it.
#[tauri::command]
pub async fn add_provider(
    state: State<'_, AppState>,
    args: AddProviderArgs,
) -> Result<AddProviderResult> {
    if args.name.trim().is_empty() {
        return Err(HolziError::InvalidInput {
            reason: "name is empty".into(),
        });
    }
    match args.kind {
        ProviderKind::ApiKey => {
            validate_adapter(args.adapter.as_deref())?;
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
            if args.adapter.is_some() {
                return Err(HolziError::InvalidInput {
                    reason: "adapter is only valid for api_key providers".into(),
                });
            }
            if args.base_url.as_deref().unwrap_or("").is_empty() {
                return Err(HolziError::InvalidInput {
                    reason: "cli_delegate provider requires base_url (cli command)".into(),
                });
            }
        }
        ProviderKind::Local => {
            // No requirements — one row per local runtime is enough.
            if args.adapter.is_some() {
                return Err(HolziError::InvalidInput {
                    reason: "adapter is only valid for api_key providers".into(),
                });
            }
        }
    }

    let db = active_database(&state)?;
    let now = now_ms();
    let provider = Provider {
        id: Uuid::new_v4(),
        kind: args.kind,
        adapter: args.adapter,
        name: args.name,
        base_url: args.base_url,
        credentials: args.api_key.map(|k| k.into_bytes()),
        created_at: now,
    };
    let inserted = provider.clone();
    let insert_db = db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        insert_db.with_connection(|conn| {
            storage::insert_provider(conn, &inserted).map_err(haex_crdt::Error::from)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("insert_provider join: {e}"),
    })?
    .map_err(HolziError::from)?;

    // Auto-refresh api_key providers so the model picker is populated
    // immediately after the credentials land. Refresh failures are
    // surfaced to the caller but do not undo the insert — the operator
    // can retry `refresh_provider_models` after fixing the key.
    let (model_count, refresh_error) = if matches!(provider.kind, ProviderKind::ApiKey) {
        match do_refresh(&db, &provider).await {
            Ok(count) => (Some(count), None),
            Err(e) => (None, Some(format_holzi_error(&e))),
        }
    } else {
        (None, None)
    };

    Ok(AddProviderResult {
        provider: provider.into(),
        model_count,
        refresh_error,
    })
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

/// Payload returned by [`refresh_provider_models`]. Reports what the
/// live fetch produced; the frontend can then re-query `models` via
/// the existing storage path for full display data.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshProviderModelsResult {
    pub provider_id: Uuid,
    pub model_count: usize,
    pub fetched_at: i64,
}

/// Fetches the given provider's model catalog live and replaces the
/// cached `models` rows for it. Plan §"Anbietermodelle": "Modelllisten
/// werden immer abgefragt, nie hartkodiert." A failed fetch leaves the
/// existing cache untouched — the caller sees an error and the UI
/// stays on the previous listing (or empty state).
///
/// Never routes for `ProviderKind::Local` — local models are managed
/// via the download/import commands. `ProviderKind::CliDelegate`
/// awaits a separate design pass and returns `InvalidInput`.
#[tauri::command]
pub async fn refresh_provider_models(
    state: State<'_, AppState>,
    provider_id: Uuid,
) -> Result<RefreshProviderModelsResult> {
    let db = active_database(&state)?;
    let db_read = db.clone();
    let provider = tauri::async_runtime::spawn_blocking(move || {
        db_read.with_connection(|conn| {
            storage::get_provider(conn, provider_id).map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("get_provider join: {e}"),
    })?
    .map_err(HolziError::from)?
    .ok_or_else(|| HolziError::InvalidInput {
        reason: format!("provider {provider_id} not found"),
    })?;

    let model_count = do_refresh(&db, &provider).await?;

    Ok(RefreshProviderModelsResult {
        provider_id,
        model_count,
        fetched_at: now_ms(),
    })
}

/// Frontend view of a provider-scoped model row. Local providers
/// intentionally return an empty list from this command — installed
/// GGUFs come through `list_installed_models` and carry file-system
/// info the api_key rows do not.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelPayload {
    pub id: String,
    pub name: String,
    pub provider_id: Uuid,
    pub context_window: Option<i64>,
}

/// Reads the cached models for one provider. Does NOT trigger a live
/// fetch — call `refresh_provider_models` for that.
#[tauri::command]
pub async fn list_provider_models(
    state: State<'_, AppState>,
    provider_id: Uuid,
) -> Result<Vec<ProviderModelPayload>> {
    let db = active_database(&state)?;
    let rows = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            models_store::list_models_by_provider(conn, provider_id).map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("list_provider_models join: {e}"),
    })?
    .map_err(HolziError::from)?;
    Ok(rows
        .into_iter()
        .map(|r| ProviderModelPayload {
            id: r.id,
            name: r.name,
            provider_id: r.provider_id,
            context_window: r.context_window,
        })
        .collect())
}

/// Shared refresh core used by both [`add_provider`] and
/// [`refresh_provider_models`]. Returns the number of models cached.
async fn do_refresh(db: &Arc<haex_crdt::Database>, provider: &Provider) -> Result<usize> {
    let provider = repair_legacy_adapter(db, provider).await?;
    let adapter = build_adapter(&provider)?;
    let fetched = adapter.list_models().await.map_err(map_adapter_error)?;

    let fetched_at = now_ms();
    let provider_id = provider.id;
    let rows: Vec<ModelRow> = fetched
        .into_iter()
        .map(|m| compose_model_row(provider_id, fetched_at, m))
        .collect();
    let model_count = rows.len();

    let db_write = db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        db_write.with_connection(|conn| {
            models_store::replace_provider_models(conn, provider_id, &rows)
                .map_err(haex_crdt::Error::from)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("replace_provider_models join: {e}"),
    })?
    .map_err(HolziError::from)?;

    Ok(model_count)
}

/// Repairs a legacy API-key row only when its stored base URL identifies the
/// Anthropic endpoint unambiguously. Other legacy rows stay unresolved and
/// are rejected by `build_adapter` instead of being guessed as Anthropic.
pub(crate) async fn repair_legacy_adapter(
    db: &Arc<haex_crdt::Database>,
    provider: &Provider,
) -> Result<Provider> {
    if provider.adapter.is_some() {
        return Ok(provider.clone());
    }
    let Some(adapter) = legacy_adapter_for_url(provider.base_url.as_deref()) else {
        return Ok(provider.clone());
    };

    let provider_id = provider.id;
    let db_write = db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        db_write.with_connection(|conn| {
            storage::set_adapter(conn, provider_id, adapter).map_err(haex_crdt::Error::from)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| HolziError::CrdtInit {
        reason: format!("repair provider adapter join: {e}"),
    })?
    .map_err(HolziError::from)?;

    let mut repaired = provider.clone();
    repaired.adapter = Some(adapter.to_string());
    Ok(repaired)
}

fn legacy_adapter_for_url(base_url: Option<&str>) -> Option<&'static str> {
    match base_url?.trim_end_matches('/') {
        "https://api.anthropic.com" | "http://api.anthropic.com" => Some("anthropic"),
        _ => None,
    }
}

/// Builds the right adapter for the provider's kind and credentials.
/// `Local` is rejected here — the chat path uses `LocalAdapter`
/// directly with an in-process `LocalModel`; refresh only makes sense
/// for remote listings.
pub(crate) fn build_adapter(provider: &Provider) -> Result<Box<dyn ProviderAdapter>> {
    match provider.kind {
        ProviderKind::ApiKey => {
            let base_url =
                provider
                    .base_url
                    .as_deref()
                    .ok_or_else(|| HolziError::InvalidInput {
                        reason: "api_key provider is missing base_url".into(),
                    })?;
            let credentials =
                provider
                    .credentials
                    .as_ref()
                    .ok_or_else(|| HolziError::InvalidInput {
                        reason: "api_key provider is missing credentials".into(),
                    })?;
            let api_key = std::str::from_utf8(credentials.as_slice())
                .map_err(|e| HolziError::InvalidInput {
                    reason: format!("api_key credentials are not UTF-8: {e}"),
                })?
                .to_string();
            let adapter = match provider.adapter.as_deref() {
                Some("anthropic") => AnthropicAdapter::new(base_url.to_string(), api_key)
                    .map_err(map_adapter_error)?,
                Some(other) => {
                    return Err(HolziError::InvalidInput {
                        reason: format!("unsupported provider adapter: {other}"),
                    });
                }
                None => {
                    return Err(HolziError::InvalidInput {
                        reason: "api_key provider is missing adapter".into(),
                    });
                }
            };
            Ok(Box::new(adapter))
        }
        ProviderKind::CliDelegate => Err(HolziError::InvalidInput {
            reason: "cli_delegate refresh is not yet implemented".into(),
        }),
        ProviderKind::Local => Err(HolziError::InvalidInput {
            reason: "refresh does not apply to local providers".into(),
        }),
    }
}

/// Validates the adapter discriminator before storing an API-key provider.
/// Dispatch must remain explicit as more vendor adapters are added.
fn validate_adapter(adapter: Option<&str>) -> Result<()> {
    match adapter {
        Some("anthropic") => Ok(()),
        Some(other) => Err(HolziError::InvalidInput {
            reason: format!("unsupported provider adapter: {other}"),
        }),
        None => Err(HolziError::InvalidInput {
            reason: "api_key provider requires adapter".into(),
        }),
    }
}

fn compose_model_row(provider_id: Uuid, fetched_at: i64, m: ProviderModel) -> ModelRow {
    // Composite id per plan §"Datenmodell". Keeps the same remote id
    // distinguishable when the operator configures two accounts with
    // the same provider.
    ModelRow {
        id: format!("{provider_id}:{}", m.remote_id),
        provider_id,
        name: m.display_name,
        context_window: m.context_window,
        fetched_at: Some(fetched_at),
        // API providers do not need a local tokenizer — the vendor's
        // server-side tokenizer handles that.
        tokenizer_repo: None,
    }
}

pub(crate) fn map_adapter_error(err: AdapterError) -> HolziError {
    // Surface `InvalidCredentials` as its own kind so the frontend can
    // render "Ungültige Zugangsdaten" per plan §"Anbietermodelle".
    // Other adapter errors ride the generic `InvalidInput` bucket for
    // now — a dedicated `HolziError::Provider*` variant lands with the
    // chat integration in slice (b), when the UI needs finer control.
    match err {
        AdapterError::InvalidCredentials => HolziError::InvalidInput {
            reason: "provider rejected credentials".into(),
        },
        AdapterError::Http { reason } => HolziError::InvalidInput {
            reason: format!("provider transport error: {reason}"),
        },
        AdapterError::Status { status, body } => HolziError::InvalidInput {
            reason: format!("provider returned {status}: {body}"),
        },
        AdapterError::Parse { reason } => HolziError::InvalidInput {
            reason: format!("provider response parse error: {reason}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{legacy_adapter_for_url, validate_adapter};
    use crate::error::HolziError;

    #[test]
    /// Only adapters with an implemented protocol may be persisted.
    fn validate_adapter_rejects_missing_and_unknown_values() {
        assert!(validate_adapter(Some("anthropic")).is_ok());
        assert!(matches!(
            validate_adapter(None),
            Err(HolziError::InvalidInput { reason }) if reason.contains("requires adapter")
        ));
        assert!(matches!(
            validate_adapter(Some("openai")),
            Err(HolziError::InvalidInput { reason }) if reason.contains("unsupported")
        ));
    }

    #[test]
    /// Legacy rows are repaired only when their endpoint identifies Anthropic.
    fn legacy_adapter_repair_does_not_guess_from_kind() {
        assert_eq!(
            legacy_adapter_for_url(Some("https://api.anthropic.com/")),
            Some("anthropic")
        );
        assert_eq!(legacy_adapter_for_url(Some("https://api.openai.com")), None);
        assert_eq!(legacy_adapter_for_url(None), None);
    }
}

fn format_holzi_error(err: &HolziError) -> String {
    match err {
        HolziError::InvalidInput { reason } => reason.clone(),
        other => other.to_string(),
    }
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
