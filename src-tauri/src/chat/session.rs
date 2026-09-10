//! Active model session — the loaded provider adapter, its associated
//! catalog-side metadata, and the current in-flight generation (if any).
//!
//! At most one model is loaded at a time; loading a second model
//! unloads the first. This matches the plan's "one active generation
//! per device" constraint and the reality that a 7B GGUF plus a
//! second GGUF would fight over VRAM/RAM.

use std::sync::{Arc, Mutex};

use crate::adapters::ProviderAdapter;

/// Metadata about the currently-loaded model. Both the local and
/// api_key paths funnel through the same `Arc<dyn ProviderAdapter>`
/// so `send_message` never branches on model kind.
#[derive(Clone)]
pub struct ActiveSession {
    /// Composite id (`<provider_id>:<remote_id>`) for api_key models,
    /// or the catalog id for local ones. Persisted verbatim as the
    /// `chat_messages.model_id` foreign key.
    pub model_id: String,
    pub adapter: Arc<dyn ProviderAdapter>,
    /// Only meaningful for local models; api_key rows carry an empty
    /// string because the vendor's server-side tokenizer handles it.
    pub tokenizer_repo: String,
    pub context_window: Option<i64>,
}

/// Tauri-managed state for the chat runtime. `session` is the loaded
/// model; `current_generation` is an abort handle for the in-flight
/// streaming task, if any. Both are optional — an idle app has neither.
pub struct ChatState {
    pub session: Mutex<Option<ActiveSession>>,
    pub current_generation: Mutex<Option<tokio::task::AbortHandle>>,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
            current_generation: Mutex::new(None),
        }
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}
