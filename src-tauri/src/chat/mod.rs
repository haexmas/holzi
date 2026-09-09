//! Chat runtime and Tauri commands.
//!
//! Feature-gated behind `llm-cpu` because the local-model session
//! carries an in-process `LocalModel`. Provider-backed chat (api_key,
//! cli_delegate) lands in a later slice — the schema for `providers`
//! is already in place.

#[cfg(feature = "llm-cpu")]
pub mod commands;
#[cfg(feature = "llm-cpu")]
pub mod session;
#[cfg(feature = "llm-cpu")]
pub mod thread_commands;
