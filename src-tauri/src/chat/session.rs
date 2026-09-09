//! Active model session — the loaded `LocalModel`, its associated
//! catalog-side metadata, and the current in-flight generation (if
//! any).
//!
//! At most one model is loaded at a time; loading a second model
//! unloads the first. This matches the plan's "one active generation
//! per device" constraint and the reality that a 7B GGUF plus a
//! second GGUF would fight over VRAM/RAM.

use std::sync::Mutex;

use crate::llm::local::LocalModel;

/// Metadata about the currently-loaded local model. `LocalModel`
/// (and its inner `mistralrs::Model`) does not implement `Debug`, so
/// neither does this struct.
#[derive(Clone)]
pub struct ActiveSession {
    pub model_id: String,
    pub model: LocalModel,
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
