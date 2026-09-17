//! Built-in curated catalog of local speech-to-text model tiers (spec 010).
//!
//! Mirrors `catalog/mod.rs` (the LLM catalog) structurally, but for Whisper
//! size tiers instead of GGUF chat models — no `hf_filename`/`tokenizer_repo`/
//! `context_window` fields, since those don't apply to the multi-file
//! `candle-transformers` Whisper load path (see data-model.md). The three
//! required filenames (`config.json`/`tokenizer.json`/`model.safetensors`)
//! are `local.rs`'s concern, not a catalog attribute — a later backend swap
//! must not need to touch this catalog (FR-008).
//!
//! Every `hf_revision` here is a pinned commit SHA, never `"main"` (see
//! research.md §6) — config, tokenizer, and weights must never drift apart
//! across an upstream repo's mutable default branch.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::hardware::tiers::{pick_three, Tier};
use crate::hardware::{classify, Fit, HardwareInfo, ModelFitInputs};

const STT_CATALOG_JSON: &str = include_str!("stt_catalog.json");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SttCatalogEntry {
    /// Stable id, also the `models::paths` slug this entry's files live
    /// under (e.g. `"whisper-tiny"`).
    pub id: String,
    pub name: String,
    pub hf_repo: String,
    /// Pinned commit SHA — never `"main"`.
    pub hf_revision: String,
    /// Sum of all files this entry's local backend needs (currently
    /// `config.json` + `tokenizer.json` + `model.safetensors`), not just the
    /// weights — the real on-disk/download footprint for the hardware-fit
    /// check.
    pub approx_size_bytes: u64,
    pub license: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSttCatalog {
    models: Vec<SttCatalogEntry>,
}

static STT_CATALOG: OnceLock<Vec<SttCatalogEntry>> = OnceLock::new();

/// Returns the curated list, cheap-copied per call.
pub fn entries() -> &'static [SttCatalogEntry] {
    STT_CATALOG
        .get_or_init(|| {
            let raw: RawSttCatalog = serde_json::from_str(STT_CATALOG_JSON)
                .expect("built-in STT catalog JSON must be valid at compile time");
            raw.models
        })
        .as_slice()
}

/// Fetches one entry by id.
pub fn get(id: &str) -> Option<&'static SttCatalogEntry> {
    entries().iter().find(|e| e.id == id)
}

/// Catalog entry enriched with the hardware fit classification. Shown on
/// the onboarding suggestion list and in Settings.
#[derive(Debug, Clone, Serialize)]
pub struct SttCatalogEntryWithFit {
    #[serde(flatten)]
    pub entry: SttCatalogEntry,
    pub fit: Fit,
}

/// One tier recommendation shown as a chip in the onboarding wizard's STT
/// step.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SttTierRecommendation {
    pub tier: Tier,
    pub entry: SttCatalogEntry,
    pub fit: Fit,
}

fn fit_for(hw: &HardwareInfo, entry: &SttCatalogEntry) -> Fit {
    classify(
        hw,
        ModelFitInputs {
            file_size_bytes: entry.approx_size_bytes,
            // No context-window concept for transcription; `fit.rs`
            // defaults this to a conservative 4096 tokens' worth of KV
            // cache, which is harmless here since Whisper tiers are tiny
            // relative to any realistic RAM/VRAM budget.
            context_window: None,
        },
    )
}

/// Lists every catalog entry annotated with a fit verdict against the given
/// hardware snapshot.
pub fn entries_with_fit(hw: &HardwareInfo) -> Vec<SttCatalogEntryWithFit> {
    entries()
        .iter()
        .cloned()
        .map(|entry| {
            let fit = fit_for(hw, &entry);
            SttCatalogEntryWithFit { entry, fit }
        })
        .collect()
}

/// Picks three tier-labelled model suggestions for the onboarding wizard's
/// STT step, using the same generic algorithm the LLM catalog uses (see
/// `hardware::tiers::pick_three`). Returns `None` only when the catalog is
/// empty (never happens in production — the built-in JSON always has three
/// entries).
pub fn recommend_tiers(hw: &HardwareInfo) -> Option<[SttTierRecommendation; 3]> {
    let candidates: Vec<(SttCatalogEntry, Fit)> = entries()
        .iter()
        .cloned()
        .map(|entry| {
            let fit = fit_for(hw, &entry);
            (entry, fit)
        })
        .collect();
    let picked = pick_three(candidates, |e| e.approx_size_bytes, |e| e.id.as_str())?;
    Some(picked.map(|(tier, entry, fit)| SttTierRecommendation { tier, entry, fit }))
}
