//! Maps the `capabilities` object of Anthropic's `GET /v1/models` response to
//! the provider-agnostic [`ModelCapabilities`] record (spec
//! 012-unified-model-capabilities, research.md R3).
//!
//! Kept beside `anthropic.rs` rather than inside it so the wire shape and its
//! mapping can be tested without an HTTP double. Every wire field is
//! optional: a response that omits a leaf yields "not determined" for what
//! depends on it, never an error and never "unsupported" (FR-002). The same
//! mapping serves the direct API-key path and the Claude Code delegate,
//! which lists models through the same endpoint.

use serde::Deserialize;

use crate::adapters::AttachmentKind;
use crate::model_capabilities::{
    ModelCapabilities, ReasoningControl, ReasoningOption, ThinkingStyle,
};

/// `{"supported": <bool>}` — the shape of every capability leaf.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Leaf {
    supported: Option<bool>,
}

impl Leaf {
    fn supported(leaf: &Option<Leaf>) -> Option<bool> {
        leaf.as_ref().and_then(|l| l.supported)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ThinkingTypes {
    enabled: Option<Leaf>,
    adaptive: Option<Leaf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Thinking {
    supported: Option<bool>,
    types: Option<ThinkingTypes>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Effort {
    supported: Option<bool>,
    low: Option<Leaf>,
    medium: Option<Leaf>,
    high: Option<Leaf>,
    xhigh: Option<Leaf>,
    max: Option<Leaf>,
}

/// The wire `capabilities` object. Unknown leaves are ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct WireCapabilities {
    image_input: Option<Leaf>,
    pdf_input: Option<Leaf>,
    thinking: Option<Thinking>,
    effort: Option<Effort>,
}

/// Effort levels in display order, paired with their wire names (which are
/// also the option ids that are validated, persisted and sent).
fn effort_levels(effort: &Effort) -> [(&'static str, &Option<Leaf>); 5] {
    [
        ("low", &effort.low),
        ("medium", &effort.medium),
        ("high", &effort.high),
        ("xhigh", &effort.xhigh),
        ("max", &effort.max),
    ]
}

fn reasoning(wire: &WireCapabilities) -> Option<ReasoningControl> {
    let effort_supported = wire.effort.as_ref().and_then(|e| e.supported);
    let thinking_supported = wire.thinking.as_ref().and_then(|t| t.supported);

    if effort_supported == Some(true) {
        let options: Vec<ReasoningOption> = wire
            .effort
            .as_ref()
            .map(effort_levels)
            .into_iter()
            .flatten()
            .filter(|(_, leaf)| Leaf::supported(leaf) == Some(true))
            .map(|(id, _)| ReasoningOption {
                id: id.to_string(),
                label: id.to_string(),
            })
            .collect();
        if !options.is_empty() {
            return Some(ReasoningControl::presets(options));
        }

        // `effort.supported: true` with missing level leaves is partial data,
        // not an authoritative statement that the model manages effort.
        let levels_complete = wire.effort.as_ref().is_some_and(|effort| {
            effort_levels(effort)
                .iter()
                .all(|(_, leaf)| Leaf::supported(leaf).is_some())
        });
        if !levels_complete {
            return None;
        }
    }
    // Both answers must be present before claiming the user has no say; a
    // missing subtree stays "not determined".
    match (effort_supported, thinking_supported) {
        (Some(_), Some(true)) => Some(ReasoningControl::ModelManaged),
        (Some(_), Some(false)) => Some(ReasoningControl::Unavailable),
        _ => None,
    }
}

fn thinking_style(wire: &WireCapabilities) -> Option<ThinkingStyle> {
    let types = wire.thinking.as_ref()?.types.as_ref()?;
    if Leaf::supported(&types.adaptive) == Some(true) {
        Some(ThinkingStyle::Adaptive)
    } else if Leaf::supported(&types.enabled) == Some(true) {
        Some(ThinkingStyle::Manual)
    } else {
        None
    }
}

/// ponytail: attachments are determined only when *both* `image_input` and
/// `pdf_input` are present, so one missing key undetermines every kind.
/// Ceiling: a provider that stops reporting `pdf_input` makes all its
/// attachments "not yet known". Upgrade path: per-kind granularity
/// (`Document` from `pdf_input` alone), see research.md R3.
fn attachment_kinds(wire: &WireCapabilities) -> Option<Vec<AttachmentKind>> {
    let image = Leaf::supported(&wire.image_input)?;
    let pdf = Leaf::supported(&wire.pdf_input)?;
    // Text files are inlined as a plain text block, which every
    // message-capable model accepts once support is determined at all.
    let mut kinds = vec![AttachmentKind::Text];
    if image {
        kinds.push(AttachmentKind::Image);
    }
    if pdf {
        kinds.push(AttachmentKind::Document);
    }
    Some(kinds)
}

/// Maps one model's wire capabilities to the persisted record.
pub(super) fn map_capabilities(wire: &WireCapabilities) -> ModelCapabilities {
    ModelCapabilities {
        reasoning: reasoning(wire),
        accepted_attachment_kinds: attachment_kinds(wire),
        thinking_style: thinking_style(wire),
    }
}

#[cfg(test)]
#[path = "anthropic_capabilities_tests.rs"]
mod anthropic_capabilities_tests;
