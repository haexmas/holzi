//! Catalog Tauri commands beyond the plain `list_catalog` listing.
//!
//! `catalog_recommend_tiers` powers the onboarding wizard's three
//! model-suggestion chips (Easy / Sweet / Max). See spec 002
//! `contracts/tauri-commands.md`.

use crate::error::{HolziError, Result};
use crate::hardware;

use super::{recommend_tiers, TierRecommendation};

/// Returns exactly three tier-labelled recommendations against the
/// current hardware. Errors as `CatalogEntryNotFound` when the built-in
/// catalog is empty; in production this only happens if the compiled
/// JSON blob is malformed.
#[tauri::command]
pub async fn catalog_recommend_tiers() -> Result<[TierRecommendation; 3]> {
    let hw = hardware::probe();
    recommend_tiers(&hw).ok_or_else(|| HolziError::CatalogEntryNotFound {
        id: "<empty catalog>".to_string(),
    })
}
