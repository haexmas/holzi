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
/// copied. Publishes only a completed copy, preserving an existing model
/// on failure. The destination directory is expected to exist (callers use
/// [`super::paths::slug_dir`] first).
pub async fn copy_into_managed(source: &Path, destination: PathBuf) -> Result<u64> {
    if !tokio::fs::metadata(source)
        .await
        .map_err(|e| HolziError::ModelImport {
            reason: format!("source metadata {}: {e}", source.display()),
        })?
        .is_file()
    {
        return Err(HolziError::ModelImport {
            reason: format!("source is not a regular file: {}", source.display()),
        });
    }
    // A unique sidecar also keeps a cancelled copy's blocking worker from
    // racing the next import. Discovery ignores its non-GGUF extension.
    let staging = destination.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = async {
        let bytes = tokio::fs::copy(source, &staging).await?;
        tokio::fs::rename(&staging, &destination).await?;
        Ok::<u64, std::io::Error>(bytes)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&staging).await;
    }
    result.map_err(|e| HolziError::ModelImport {
        reason: format!(
            "import {} -> {}: {e}",
            source.display(),
            destination.display()
        ),
    })
}
