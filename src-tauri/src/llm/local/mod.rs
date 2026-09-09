//! In-process GGUF inference via mistralrs 0.8.x.
//!
//! Wraps [`mistralrs::Model`] for holzi's use case: single-model load,
//! token-by-token streaming into a Tauri event channel, cancellable by
//! dropping the [`GenerationHandle`]. Backend selection (CPU/CUDA/Metal)
//! happens at compile time via feature flags — the runtime cannot
//! upgrade CPU to CUDA without a rebuild (Etappe 0 finding #3).

mod loader;
#[cfg(test)]
mod loader_tests;
mod stream;

pub use loader::{LocalModel, LocalModelError};
pub use stream::{ChatMessage, ChatRequest, ChatRole, GenerationHandle, StreamChunk, StreamError};
