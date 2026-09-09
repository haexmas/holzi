//! Local and provider-backed LLM inference.
//!
//! The entire module is gated behind the `llm-cpu` feature so a
//! `--no-default-features` build skips the mistralrs/candle dependency
//! tree while iterating on non-LLM code. See `Cargo.toml` for the
//! CUDA/Metal opt-ins that stack on top of `llm-cpu`.

#[cfg(feature = "llm-cpu")]
pub mod local;
