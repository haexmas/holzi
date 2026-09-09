//! Built-in curated catalog of downloadable GGUF models.
//!
//! Per operator decision 2026-09-09: the catalog is a **suggestion**
//! list, not a gate. The user can also enter an arbitrary HuggingFace
//! `repo` + `filename` pair, or import a local GGUF. Model bytes are
//! NEVER shipped with the app; the catalog only carries metadata (repo
//! id, filename, approximate size, license) and the download itself
//! runs at pick time.
//!
//! The JSON blob is embedded at compile time via `include_str!` and
//! parsed once into a `OnceLock` so listing costs nothing after the
//! first call.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const CATALOG_JSON: &str = include_str!("model_catalog.json");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatalogEntry {
    /// Stable id used across `models.id` and
    /// `device_downloaded_models_no_sync.id`.
    pub id: String,
    pub name: String,
    pub family: String,
    pub parameters: String,
    pub quantization: String,
    pub hf_repo: String,
    pub hf_filename: String,
    pub tokenizer_repo: String,
    /// Approximate file size from bartowski's repo page. Rounded — the
    /// actual `Content-Length` from the HuggingFace CDN is authoritative
    /// and is captured on download into
    /// `device_downloaded_models_no_sync.size_bytes`.
    pub approx_size_bytes: u64,
    pub context_window: u64,
    pub license: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawCatalog {
    models: Vec<CatalogEntry>,
}

static CATALOG: OnceLock<Vec<CatalogEntry>> = OnceLock::new();

/// Returns the curated list, cheap-copied per call.
pub fn entries() -> &'static [CatalogEntry] {
    CATALOG
        .get_or_init(|| {
            let raw: RawCatalog = serde_json::from_str(CATALOG_JSON)
                .expect("built-in catalog JSON must be valid at compile time");
            raw.models
        })
        .as_slice()
}

/// Fetches one entry by id.
pub fn get(id: &str) -> Option<&'static CatalogEntry> {
    entries().iter().find(|e| e.id == id)
}

/// Builds the HuggingFace resolve URL for a repo + filename. Uses the
/// `main` revision on purpose — GGUFs in bartowski's repos are
/// append-only per quantization, and pinning a revision requires
/// enumerating the LFS pointer, which we do not do in the MVP.
pub fn hf_resolve_url(repo: &str, filename: &str) -> String {
    format!("https://huggingface.co/{repo}/resolve/main/{filename}")
}

/// Catalog entry enriched with the hardware fit classification. Shown
/// on the onboarding suggestion list so the operator sees at a glance
/// whether a model matches this device.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogEntryWithFit {
    #[serde(flatten)]
    pub entry: CatalogEntry,
    pub fit: crate::hardware::Fit,
}

/// Tauri command: list the built-in catalog, annotated with a fit
/// verdict against the current hardware. Blocks briefly (`nvidia-smi`
/// on CUDA hosts, capped by [`crate::hardware::CUDA_PROBE_TIMEOUT`]).
#[tauri::command]
pub async fn list_catalog() -> Vec<CatalogEntryWithFit> {
    let hw = crate::hardware::probe();
    entries()
        .iter()
        .cloned()
        .map(|entry| {
            let fit = crate::hardware::classify(
                &hw,
                crate::hardware::ModelFitInputs {
                    file_size_bytes: entry.approx_size_bytes,
                    context_window: Some(entry.context_window),
                },
            );
            CatalogEntryWithFit { entry, fit }
        })
        .collect()
}
