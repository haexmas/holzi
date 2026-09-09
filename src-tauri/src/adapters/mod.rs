//! Provider adapters — one implementation per non-local provider kind.
//!
//! The `ProviderAdapter` trait is what the refresh and chat paths call
//! against. Slice (a) ships the trait plus the Anthropic adapter for
//! `list_models`; slice (b) grows the trait with `stream_chat` and
//! wires `Box<dyn ProviderAdapter>` into `ChatState` so any adapter
//! can serve a conversation. Additional `api_key`-family adapters
//! (OpenAI, Google, Groq) land as their own follow-up PRs once the
//! trait surface is stable. `cli_delegate` adapters require a separate
//! design pass and are not yet represented here.

pub mod anthropic;
#[cfg(test)]
mod anthropic_tests;

use async_trait::async_trait;

/// One model as reported by a provider. Provider-agnostic subset of
/// what a listing endpoint returns; storage enrichment (composite id,
/// fetched_at) lives adjacent in `models::commands` so this stays
/// free of persistence concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModel {
    /// Remote id as returned by the provider (e.g. `"claude-opus-5"`).
    /// The persisted `models.id` composes this with the provider UUID
    /// per plan §"Datenmodell" (`<provider_id>:<remote_model_id>`).
    pub remote_id: String,
    /// Human-readable name (e.g. `"Claude Opus 5"`) that appears in
    /// the model picker.
    pub display_name: String,
    /// Maximum input context in tokens, if reported. `None` when the
    /// provider does not surface it. Persisted into
    /// `models.context_window`.
    pub context_window: Option<i64>,
}

/// Errors an adapter can return. All variants are terminal — the
/// caller must not retry, especially on 4xx. Plan §"Anbietermodelle"
/// requires that "Ungültige Zugangsdaten führen zu einer verständlichen
/// Meldung": `InvalidCredentials` is mapped from 401/403 explicitly so
/// the frontend can render it as a targeted message instead of a
/// generic HTTP error. Cache updates only run on `Ok(_)`; a failed
/// refresh leaves the previous cache intact.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("http transport failure: {reason}")]
    Http { reason: String },
    #[error("provider returned status {status}: {body}")]
    Status { status: u16, body: String },
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("failed to parse provider response: {reason}")]
    Parse { reason: String },
}

/// Contract every non-local provider adapter implements. Slice (a)
/// only requires `list_models`; slice (b) will add a `stream_chat`
/// method plus a `LocalAdapter` wrapper around `LocalModel` so the
/// chat dispatch does not special-case local vs. api_key.
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// Fetches the provider's current model listing. Must paginate
    /// internally if the provider chunks the response so callers
    /// receive a complete list. On failure the caller's cache is
    /// preserved.
    async fn list_models(&self) -> Result<Vec<ProviderModel>, AdapterError>;
}
