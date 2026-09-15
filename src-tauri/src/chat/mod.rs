//! Chat runtime and Tauri commands.
//!
//! Provider-backed chat is available in every build. Only the local
//! mistralrs loading path is feature-gated behind `llm-cpu`.

pub mod commands;
pub mod default_model;
pub mod events;
pub mod model_loading;
pub mod send_admission;
pub mod session;
pub mod thread_commands;
pub mod tools;
pub mod turn;

#[cfg(test)]
mod session_tests;
