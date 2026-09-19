//! Reasoning-effort levels (spec 011-composer-toolbar-parity).
//!
//! `EffortLevel` mirrors Anthropic's real `output_config.effort` request
//! field and Claude Code's real `--effort` CLI flag byte-for-byte
//! (research.md §1) — there is deliberately no holzi-internal relabeling,
//! since FR-005 requires the displayed names to be the provider's own.
//!
//! Ordered `Low < Medium < High < XHigh < Max` to match Claude Code's own
//! documented fallback rule ("falls back to highest supported level at or
//! below the requested one") — [`clamp`] implements the same rule for the
//! direct Anthropic API path, which (unlike Claude Code) is not documented
//! to self-clamp, so holzi must never send it a level a model doesn't
//! support.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffortLevel {
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl EffortLevel {
    /// The literal wire value both Anthropic's `output_config.effort` and
    /// Claude Code's `--effort` accept unchanged.
    pub fn as_str(self) -> &'static str {
        match self {
            EffortLevel::Low => "low",
            EffortLevel::Medium => "medium",
            EffortLevel::High => "high",
            EffortLevel::XHigh => "xhigh",
            EffortLevel::Max => "max",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "low" => Some(EffortLevel::Low),
            "medium" => Some(EffortLevel::Medium),
            "high" => Some(EffortLevel::High),
            "xhigh" => Some(EffortLevel::XHigh),
            "max" => Some(EffortLevel::Max),
            _ => None,
        }
    }
}

const LOW_MEDIUM_HIGH_MAX: &[EffortLevel] = &[
    EffortLevel::Low,
    EffortLevel::Medium,
    EffortLevel::High,
    EffortLevel::Max,
];

const LOW_MEDIUM_HIGH_XHIGH_MAX: &[EffortLevel] = &[
    EffortLevel::Low,
    EffortLevel::Medium,
    EffortLevel::High,
    EffortLevel::XHigh,
    EffortLevel::Max,
];

/// Curated per-model support table (research.md §1, fetched from
/// `platform.claude.com/docs/en/build-with-claude/effort` on 2026-09-19).
/// Not a version/substring formula like `request.rs::supports_adaptive_thinking`
/// — the `xhigh`/`max` split doesn't follow a clean numeric rule
/// (`claude-opus-4-6`/`claude-sonnet-4-6` support `max` but not `xhigh`) —
/// so this is an explicit, reviewable match table. Update it as Anthropic
/// documents support for additional models.
const MODEL_EFFORT_LEVELS: &[(&str, &[EffortLevel])] = &[
    ("claude-fable-5-1", LOW_MEDIUM_HIGH_XHIGH_MAX),
    ("claude-mythos-5-1", LOW_MEDIUM_HIGH_XHIGH_MAX),
    ("claude-fable-5", LOW_MEDIUM_HIGH_XHIGH_MAX),
    ("claude-mythos-5", LOW_MEDIUM_HIGH_XHIGH_MAX),
    ("claude-mythos-preview", LOW_MEDIUM_HIGH_MAX),
    ("claude-opus-5", LOW_MEDIUM_HIGH_XHIGH_MAX),
    ("claude-opus-4-8", LOW_MEDIUM_HIGH_XHIGH_MAX),
    ("claude-opus-4-7", LOW_MEDIUM_HIGH_XHIGH_MAX),
    ("claude-opus-4-6", LOW_MEDIUM_HIGH_MAX),
    ("claude-opus-4-5-20251101", LOW_MEDIUM_HIGH_MAX),
    ("claude-sonnet-5", LOW_MEDIUM_HIGH_XHIGH_MAX),
    ("claude-sonnet-4-6", LOW_MEDIUM_HIGH_MAX),
];

/// Returns the effort levels `model_id` actually supports on the direct
/// Anthropic API, or `&[]` when it supports none (FR-001/FR-003) — an
/// unrecognized model id is treated as unsupported rather than guessed.
pub fn anthropic_supported_levels(model_id: &str) -> &'static [EffortLevel] {
    MODEL_EFFORT_LEVELS
        .iter()
        .find(|(id, _)| *id == model_id)
        .map(|(_, levels)| *levels)
        .unwrap_or(&[])
}

/// The Claude Code delegate always offers the full set: unlike the direct
/// API, the CLI itself falls back to "the highest supported level at or
/// below the requested one" per model (research.md §1), so holzi does not
/// need — and must not maintain — a second copy of Claude Code's own
/// per-model table.
pub fn claude_delegate_levels() -> &'static [EffortLevel] {
    LOW_MEDIUM_HIGH_XHIGH_MAX
}

/// Picks `requested` if `supported` contains it, otherwise the next lower
/// level `supported` does contain, otherwise `None` (FR-002). Used by the
/// direct-API request builder, which — unlike the Claude Code delegate —
/// is not documented to clamp on its own.
pub fn clamp(requested: EffortLevel, supported: &[EffortLevel]) -> Option<EffortLevel> {
    supported
        .iter()
        .copied()
        .filter(|level| *level <= requested)
        .max()
}
