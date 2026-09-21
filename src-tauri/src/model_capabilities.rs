//! What one provider/model pair supports, as one persisted record.
//!
//! Replaces the independent, hand-maintained rules that used to decide
//! reasoning-effort levels, thinking style and attachment usability from
//! model-id tables and provider kinds (spec 012-unified-model-capabilities).
//! Every field is an `Option` on purpose: `None` means "not determined yet"
//! (a provider that has not been asked, or answered incompletely) and is
//! never the same as an authoritative negative answer such as
//! [`ReasoningControl::Unavailable`] or `Some(vec![])` attachments.
//!
//! This is a top-level leaf module — `storage` (the cached `models` row) and
//! `adapters` (the live provider answer) both need these types, and neither
//! should have to depend on the other for them. It is unrelated to
//! `providers.capability` (`chat` vs `transcription`), which classifies a
//! provider, not a model.

use serde::{Deserialize, Serialize};

use crate::adapters::AttachmentKind;

/// The persisted capability record. Stored as JSON in
/// `models.capabilities_json`; a missing field reads as `None` and unknown
/// JSON fields are ignored, so a newer build's extra fields never break an
/// older reader.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelCapabilities {
    /// Whether and how the user can influence reasoning. `None` = not
    /// determined.
    pub reasoning: Option<ReasoningControl>,
    /// Attachment kinds the model accepts. `None` = not determined;
    /// `Some(vec![])` = authoritatively none.
    pub accepted_attachment_kinds: Option<Vec<AttachmentKind>>,
    /// How a thinking-capable model expects thinking to be requested — an
    /// adapter-private hint read only by the adapter that produced it.
    /// `None` for local, Codex and undetermined models.
    pub thinking_style: Option<ThinkingStyle>,
}

/// A model's answer to "can the user influence reasoning?".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReasoningControl {
    /// Determined: there is no reasoning control.
    Unavailable,
    /// Determined: the model reasons on its own and the user cannot choose.
    ModelManaged,
    /// Determined: the user may pick one of these provider-native options.
    /// Non-empty by construction ([`ReasoningControl::presets`]) and by
    /// [`ModelCapabilities::normalized`] for values read from storage.
    Presets { options: Vec<ReasoningOption> },
}

/// One selectable reasoning option. `id` is the provider-native wire value
/// that is validated, persisted and sent; `label` is a display fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningOption {
    pub id: String,
    pub label: String,
}

/// How a thinking-capable model wants thinking requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingStyle {
    Adaptive,
    Manual,
}

impl ReasoningControl {
    /// Builds a `Presets` control, or `Unavailable` when there is nothing
    /// to choose — an empty option list must never reach the composer as a
    /// "selectable" control.
    pub fn presets(options: Vec<ReasoningOption>) -> Self {
        if options.is_empty() {
            ReasoningControl::Unavailable
        } else {
            ReasoningControl::Presets { options }
        }
    }

    /// True when the model reasons at all — the user can choose how
    /// (`Presets`) or the model decides on its own (`ModelManaged`). This is
    /// what decides whether reasoning is requested and shown while it answers
    /// (spec 012 FR-023).
    pub fn reasons(&self) -> bool {
        matches!(
            self,
            ReasoningControl::Presets { .. } | ReasoningControl::ModelManaged
        )
    }

    /// True when `id` is one of the options this control currently offers.
    pub fn offers(&self, id: &str) -> bool {
        match self {
            ReasoningControl::Presets { options } => options.iter().any(|o| o.id == id),
            ReasoningControl::Unavailable | ReasoningControl::ModelManaged => false,
        }
    }
}

impl ModelCapabilities {
    /// True when nothing has been determined at all. Such a record is stored
    /// as SQL `NULL` rather than as a JSON object of nulls.
    pub fn is_undetermined(&self) -> bool {
        *self == ModelCapabilities::default()
    }

    /// Restores the "presets are never empty" invariant for a record that
    /// came through serde (storage or the wire), which bypasses
    /// [`ReasoningControl::presets`].
    pub fn normalized(mut self) -> Self {
        if let Some(ReasoningControl::Presets { options }) = &self.reasoning {
            if options.is_empty() {
                self.reasoning = Some(ReasoningControl::Unavailable);
            }
        }
        self
    }

    /// The record for a built-in (local GGUF) model. There is no provider to
    /// ask, so reasoning comes from [`local_model_reasoning`] and attachments
    /// are authoritatively none until local multimodal support exists.
    pub fn local(model_id: &str) -> Self {
        ModelCapabilities {
            reasoning: Some(local_model_reasoning(model_id)),
            accepted_attachment_kinds: Some(Vec::new()),
            thinking_style: None,
        }
    }
}

/// Reasoning control for a built-in local model, derived from its id.
///
/// ponytail: a naive id heuristic — unknown families default to no
/// reasoning. Ceiling: a new reasoning family is missed until it is added
/// here. Upgrade path: read the chat template from the GGUF metadata.
///
/// Local models are the one place this remains: they have no provider to
/// ask. Provider models get their record from the provider's own answer, so
/// no Anthropic ids are matched here.
pub fn local_model_reasoning(model_id: &str) -> ReasoningControl {
    let id = model_id.to_ascii_lowercase();
    // Some published checkpoints use a family name that is otherwise
    // reasoning-capable but explicitly disable thinking. Keep these exact
    // model exceptions ahead of the family fallback below.
    const EXACT_CAPABILITIES: &[(&str, bool)] = &[
        ("qwen3-4b-instruct-2507", false),
        ("qwen3-30b-a3b-instruct-2507", false),
    ];
    let reasons = if let Some((_, supported)) = EXACT_CAPABILITIES
        .iter()
        .find(|(known_id, _)| id == *known_id)
    {
        *supported
    } else if id.contains("qwen3") && id.contains("instruct-2507") {
        false
    } else {
        id.contains("qwen3") || id.contains("deepseek-r1") || id.contains("gpt-oss")
    };
    if reasons {
        ReasoningControl::ModelManaged
    } else {
        ReasoningControl::Unavailable
    }
}

#[cfg(test)]
#[path = "model_capabilities_tests.rs"]
mod model_capabilities_tests;
