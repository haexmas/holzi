//! Chat runtime and Tauri commands.
//!
//! Provider-backed chat is available in every build. Only the local
//! mistralrs loading path is feature-gated behind `llm-cpu`.

pub mod commands;
#[cfg(test)]
mod commands_tests;
pub mod session;
pub mod thread_commands;
