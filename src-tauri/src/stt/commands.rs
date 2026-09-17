//! STT-catalog Tauri commands (spec 010): list the built-in catalog, get
//! tier recommendations, list installed tiers, and download one. Setting
//! the active tier is a plain preference write (`voice.stt_model_id`) via
//! the existing generic `set_pref` command — no dedicated command for that.

#[cfg(feature = "llm-cpu")]
use std::path::PathBuf;

#[cfg(feature = "llm-cpu")]
use tauri::AppHandle;

use crate::error::{HolziError, Result};
use crate::hardware;

use super::catalog::{self, SttCatalogEntry, SttCatalogEntryWithFit, SttTierRecommendation};

/// One installed (fully downloaded) STT catalog entry.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSttModel {
    pub id: String,
    pub name: String,
}

/// Tauri command: list the built-in STT catalog, annotated with a fit
/// verdict against the current hardware. Mirrors `list_catalog`
/// (`catalog/commands.rs`) for the LLM catalog.
#[tauri::command]
pub async fn list_stt_catalog() -> Vec<SttCatalogEntryWithFit> {
    let hw = hardware::probe_async().await;
    catalog::entries_with_fit(&hw)
}

/// Tauri command: exactly three tier-labelled STT recommendations for the
/// onboarding wizard's STT step. Mirrors `catalog_recommend_tiers`.
#[tauri::command]
pub async fn stt_recommend_tiers() -> Result<[SttTierRecommendation; 3]> {
    let hw = hardware::probe_async().await;
    catalog::recommend_tiers(&hw).ok_or_else(|| HolziError::CatalogEntryNotFound {
        id: "<empty stt catalog>".to_string(),
    })
}

/// Pure scanning logic behind `list_installed_stt_models`, parameterized
/// over how to resolve each entry's directory so it's unit-testable
/// without a Tauri `AppHandle` — this codebase has no mocking convention
/// for that type (every other `AppHandle`-taking function is likewise
/// tested only through its `&Path`-based helpers). `resolve_dir` returning
/// `None` (a resolution failure, e.g. `AppLocalData` itself unresolvable)
/// is treated the same as "not installed" for that one entry rather than
/// failing the whole listing.
/// `pub` (rather than private) so `commands_tests.rs` can exercise it
/// directly with fake directories, without a Tauri `AppHandle`.
#[cfg(feature = "llm-cpu")]
pub fn scan_installed(
    entries: &[SttCatalogEntry],
    resolve_dir: impl Fn(&SttCatalogEntry) -> Option<PathBuf>,
) -> Vec<InstalledSttModel> {
    entries
        .iter()
        .filter_map(|entry| {
            let dir = resolve_dir(entry)?;
            super::local::is_complete_model(&dir).then(|| InstalledSttModel {
                id: entry.id.clone(),
                name: entry.name.clone(),
            })
        })
        .collect()
}

/// Tauri command: catalog entries whose files are already fully installed
/// on this device. No DB table behind this — a directory-existence scan
/// (data-model.md), migrating a complete legacy `whisper-tiny` install
/// into the canonical location first if one is found (only relevant for
/// that one entry).
#[cfg(feature = "llm-cpu")]
#[tauri::command]
pub async fn list_installed_stt_models(app: AppHandle) -> Result<Vec<InstalledSttModel>> {
    Ok(scan_installed(catalog::entries(), |entry| {
        super::local::resolve_or_migrate_model_dir(&app, entry).ok()
    }))
}

/// `llm-cpu`-off builds have no local Whisper adapter at all, so nothing
/// can ever be installed — an empty list is the correct answer, not an
/// error (mirrors how `voice::invalidate_stt_model_cache` is a successful
/// no-op in the same build).
#[cfg(not(feature = "llm-cpu"))]
#[tauri::command]
pub async fn list_installed_stt_models() -> Result<Vec<InstalledSttModel>> {
    Ok(Vec::new())
}

/// Resolves `catalog_id` to its catalog entry, or `CatalogEntryNotFound`.
/// Pure/`AppHandle`-free on purpose so the error-mapping is directly
/// unit-testable. `pub` for the same reason as [`scan_installed`].
pub fn resolve_catalog_entry(catalog_id: &str) -> Result<&'static SttCatalogEntry> {
    catalog::get(catalog_id).ok_or_else(|| HolziError::CatalogEntryNotFound {
        id: catalog_id.to_string(),
    })
}

/// Tauri command: downloads (or repairs an incomplete download of)
/// `catalog_id`'s files. Idempotent. Does **not** change the active
/// `voice.stt_model_id` preference — the frontend does that separately
/// after a successful download, mirroring `download_model_from_catalog`'s
/// contract for chat models (`ModelChoiceStep.vue` calls
/// `downloadFromCatalogAsync` then `setPrefAsync` as two separate steps).
#[cfg(feature = "llm-cpu")]
#[tauri::command]
pub async fn download_stt_model(app: AppHandle, catalog_id: String) -> Result<InstalledSttModel> {
    let entry = resolve_catalog_entry(&catalog_id)?;
    let dir = super::local::resolve_or_migrate_model_dir(&app, entry)?;
    super::local::ensure_model_files(&dir, entry).await?;
    Ok(InstalledSttModel {
        id: entry.id.clone(),
        name: entry.name.clone(),
    })
}

/// `llm-cpu`-off builds can't download or load a local model at all —
/// surfaced as the same `LocalUnavailable`-backed error the transcription
/// path itself would report if it tried.
#[cfg(not(feature = "llm-cpu"))]
#[tauri::command]
pub async fn download_stt_model(_catalog_id: String) -> Result<InstalledSttModel> {
    Err(crate::stt::SttError::LocalUnavailable {
        reason: "local transcription is not enabled in this build".into(),
    }
    .into())
}
