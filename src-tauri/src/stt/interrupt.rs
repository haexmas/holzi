//! Local, deterministic interrupt-word matching (spec 008 FR-007/FR-009).
//!
//! A pure function — no adapter, no provider involved — because
//! recognizing an interrupt must not depend on which STT backend
//! transcribed the utterance, or on the assistant's own state (FR-008).
//! The bounded local fast path (`stop_voice_recording`, T019) calls this on
//! the locally transcribed text before waiting on any external provider.

use serde::Serialize;

/// Wire representation matches exactly `"stop"` / `"halt"` / `"abbrechen"`
/// (data-model.md §`InterruptCommand`) — never a Rust variant name — since
/// `TranscriptionResult.interrupt` serializes this directly to the
/// frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InterruptCommand {
    Stop,
    Halt,
    Abbrechen,
}

/// Matches only when the *entire* (trimmed, lowercased) transcript equals
/// one of the three interrupt words. A longer sentence merely containing
/// one of them (e.g. "bitte nicht mehr stoppen mitten im Satz") must not
/// match — no substring match (FR-009, SC-006).
pub fn match_interrupt(transcript: &str) -> Option<InterruptCommand> {
    match transcript.trim().to_lowercase().as_str() {
        "stop" => Some(InterruptCommand::Stop),
        "halt" => Some(InterruptCommand::Halt),
        "abbrechen" => Some(InterruptCommand::Abbrechen),
        _ => None,
    }
}
