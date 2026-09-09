//! Anthropic HTTP adapter.
//!
//! Reference: <https://docs.claude.com/en/api/models-list> and
//! <https://docs.claude.com/en/api/versioning>. Uses the stable
//! `2023-06-01` version header (see [`ANTHROPIC_VERSION`]). Additive
//! output fields under that version are safe — the deserializer
//! ignores unknown keys.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use super::{AdapterError, ProviderAdapter, ProviderModel};

/// The pinned Anthropic API version. Per docs, additive optional
/// inputs and outputs are allowed inside a version without breaking
/// callers.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic caps `limit` at 1000. The catalog is small (~a dozen
/// entries) so one page suffices in practice; the loop paginates on
/// `has_more` for correctness.
const PAGE_LIMIT: u16 = 1000;

/// Adapter for `api_key`-kind providers pointed at the Anthropic
/// Messages API. Constructed per refresh from the provider row plus
/// its stored credentials blob.
pub struct AnthropicAdapter {
    client: Client,
    base_url: String,
    api_key: String,
}

impl AnthropicAdapter {
    /// Builds an adapter. `base_url` should be
    /// `https://api.anthropic.com` for production; tests pass a
    /// wiremock URL. Trailing slashes on `base_url` are trimmed so
    /// callers do not need to normalise.
    pub fn new(base_url: String, api_key: String) -> Result<Self, AdapterError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .user_agent(concat!("holzi/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| AdapterError::Http {
                reason: format!("build reqwest client: {e}"),
            })?;
        Ok(Self {
            client,
            base_url,
            api_key,
        })
    }
}

#[derive(Deserialize)]
struct ListModelsPage {
    data: Vec<ModelInfo>,
    has_more: bool,
    last_id: Option<String>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
    display_name: String,
    /// Nullable integer in the response. A returned `0` is treated as
    /// "not reported" so the picker does not misrender a zero-width
    /// context.
    #[serde(default)]
    max_input_tokens: Option<i64>,
}

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    async fn list_models(&self) -> Result<Vec<ProviderModel>, AdapterError> {
        let endpoint = format!("{}/v1/models", self.base_url.trim_end_matches('/'));
        let limit_str = PAGE_LIMIT.to_string();
        let mut out: Vec<ProviderModel> = Vec::new();
        let mut after_id: Option<String> = None;

        loop {
            let mut query: Vec<(&str, &str)> = vec![("limit", limit_str.as_str())];
            if let Some(after) = after_id.as_deref() {
                query.push(("after_id", after));
            }
            let resp = self
                .client
                .get(&endpoint)
                .query(&query)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .send()
                .await
                .map_err(|e| AdapterError::Http {
                    reason: format!("GET {endpoint}: {e}"),
                })?;

            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(match status.as_u16() {
                    401 | 403 => AdapterError::InvalidCredentials,
                    other => AdapterError::Status {
                        status: other,
                        body,
                    },
                });
            }

            let page: ListModelsPage = resp.json().await.map_err(|e| AdapterError::Parse {
                reason: format!("decode /v1/models: {e}"),
            })?;
            for m in page.data {
                out.push(ProviderModel {
                    remote_id: m.id,
                    display_name: m.display_name,
                    context_window: m.max_input_tokens.filter(|n| *n > 0),
                });
            }
            if !page.has_more {
                break;
            }
            match page.last_id {
                Some(id) => after_id = Some(id),
                None => break,
            }
        }
        Ok(out)
    }
}
