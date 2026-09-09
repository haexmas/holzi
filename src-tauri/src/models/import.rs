//! Local file import: copy an operator-picked GGUF into
//! `<AppLocalData>/models/<slug>/`.
//!
//! Plan §"Datenmodell": "Wird eine eigene GGUF importiert, wird sie
//! kopiert, nicht referenziert." Referencing the original path would
//! break when the user moves or deletes the source, and would tie the
//! model into a location outside the app's sandbox on mobile.

use std::path::{Path, PathBuf};

use crate::error::{HolziError, Result};

/// Copies `source` into `destination`, returning the actual byte count
/// copied. The destination directory is expected to exist (callers use
/// [`super::paths::slug_dir`] first).
pub async fn copy_into_managed(
    source: &Path,
    destination: PathBuf,
) -> Result<u64> {
    if !source.is_file() {
        return Err(HolziError::ModelImport {
            reason: format!("source is not a regular file: {}", source.display()),
        });
    }
    let bytes = tokio::fs::copy(source, &destination).await.map_err(|e| {
        HolziError::ModelImport {
            reason: format!(
                "copy {} -> {}: {e}",
                source.display(),
                destination.display()
            ),
        }
    })?;
    Ok(bytes)
}
