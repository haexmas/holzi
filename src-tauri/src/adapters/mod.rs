//! Provider adapters — one implementation per provider kind.
//!
//! The `ProviderAdapter` trait is what the refresh and chat paths call
//! against. Both `list_models` (Etappe-2 refresh path) and `stream_chat`
//! (Etappe-3 chat dispatch) go through the trait, so `ChatState` sees a
//! single `Arc<dyn ProviderAdapter>` regardless of whether the model
//! runs in-process (`LocalAdapter`) or over HTTP (`AnthropicAdapter`).
//! Additional `api_key` vendors (OpenAI, Google, Groq) land as their own
//! follow-up PRs using the same trait surface. `cli_delegate` adapters
//! require a separate design pass and are not yet represented here.

pub mod anthropic;
#[cfg(test)]
mod anthropic_tests;
#[cfg(feature = "llm-cpu")]
pub mod local;
pub mod types;

pub use types::{AdapterStream, ChatMessage, ChatRequest, ChatRole, StreamChunk, StreamError};

use async_trait::async_trait;

/// One model as reported by a provider. Provider-agnostic subset of
/// what a listing endpoint returns; storage enrichment (composite id,
/// fetched_at) lives adjacent in `providers::mod` so this stays free of
/// persistence concerns.
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

/// Errors an adapter can return from the pre-stream request path. All
/// variants are terminal — the caller must not retry, especially on
/// 4xx. Plan §"Anbietermodelle" requires that "Ungültige Zugangsdaten
/// führen zu einer verständlichen Meldung": `InvalidCredentials` is
/// mapped from 401/403 explicitly so the frontend can render it as a
/// targeted message instead of a generic HTTP error. Cache updates
/// only run on `Ok(_)`; a failed refresh leaves the previous cache
/// intact.
///
/// Errors that occur mid-stream (after `stream_chat` returns Ok) are
/// delivered on the [`AdapterStream`] as [`StreamError`] instead.
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

/// Contract every provider adapter implements. Both the listing and the
/// streaming paths funnel through this trait so `ChatState` can hold
/// `Arc<dyn ProviderAdapter>` without knowing which vendor sits behind
/// it.
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// Fetches the provider's current model listing. Must paginate
    /// internally if the provider chunks the response so callers
    /// receive a complete list. On failure the caller's cache is
    /// preserved. `LocalAdapter` returns an empty `Vec` — local models
    /// enter the catalog via the download / import commands, not
    /// through refresh.
    async fn list_models(&self) -> Result<Vec<ProviderModel>, AdapterError>;

    /// Starts a streaming chat generation. Returns immediately with an
    /// [`AdapterStream`]; failures that surface before the first byte
    /// (transport, non-2xx, credential rejection) come back through
    /// [`AdapterError`], while failures mid-stream ride the stream's
    /// `Err` branch as [`StreamError`].
    async fn stream_chat(&self, req: ChatRequest) -> Result<AdapterStream, AdapterError>;
}
