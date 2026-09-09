//! HuggingFace GGUF download with progress reporting.
//!
//! The download writes to `<final>.part` and renames on success so a
//! crashed or aborted download never leaves a partial file that looks
//! valid to the loader. Content-Length is captured from the response
//! header and reported through the progress callback; if the server
//! omits it (rare but allowed by HTTP), the total is `None` and the
//! UI shows an indeterminate bar.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures::StreamExt;
use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::error::{HolziError, Result};

/// Minimum wall-clock gap between progress reports. Guards against
/// firing thousands of Tauri events per second on fast local networks.
const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(100);

/// Result of a successful download.
#[derive(Debug, Clone)]
pub struct DownloadedFile {
    pub absolute_path: PathBuf,
    pub bytes_written: u64,
}

/// Progress event payload passed to the caller. `bytes_total` is
/// `None` if the server omitted `Content-Length`.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
}

/// Downloads `url` to `destination`, invoking `on_progress` at most
/// once per [`PROGRESS_MIN_INTERVAL`] with the running byte count.
/// Writes to `<destination>.part` and renames on success.
pub async fn download_to_file<F>(
    url: &str,
    destination: PathBuf,
    mut on_progress: F,
) -> Result<DownloadedFile>
where
    F: FnMut(DownloadProgress),
{
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .user_agent(concat!("holzi/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| download_err(format!("http client: {e}")))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| download_err(format!("GET {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(download_err(format!(
            "GET {url} returned status {}",
            resp.status()
        )));
    }

    let bytes_total = resp.content_length();
    let part_path = with_suffix(&destination, ".part");

    // Ensure the parent exists — the caller may have used a fresh slug.
    if let Some(parent) = part_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| download_err(format!("mkdir {}: {e}", parent.display())))?;
    }

    let mut file = File::create(&part_path)
        .await
        .map_err(|e| download_err(format!("create {}: {e}", part_path.display())))?;

    let mut stream = resp.bytes_stream();
    let mut bytes_downloaded: u64 = 0;
    let mut last_report = Instant::now() - PROGRESS_MIN_INTERVAL;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| download_err(format!("stream: {e}")))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| download_err(format!("write: {e}")))?;
        bytes_downloaded = bytes_downloaded.saturating_add(chunk.len() as u64);

        if last_report.elapsed() >= PROGRESS_MIN_INTERVAL {
            on_progress(DownloadProgress {
                bytes_downloaded,
                bytes_total,
            });
            last_report = Instant::now();
        }
    }

    file.flush()
        .await
        .map_err(|e| download_err(format!("flush: {e}")))?;
    // Explicit drop so the rename below sees a closed handle on
    // platforms where the OS treats open handles as move-blockers.
    drop(file);

    tokio::fs::rename(&part_path, &destination)
        .await
        .map_err(|e| {
            download_err(format!(
                "rename {} -> {}: {e}",
                part_path.display(),
                destination.display()
            ))
        })?;

    // Final progress emit so a UI that only listens for progress sees
    // a 100 % frame before receiving the completion event.
    on_progress(DownloadProgress {
        bytes_downloaded,
        bytes_total,
    });

    Ok(DownloadedFile {
        absolute_path: destination,
        bytes_written: bytes_downloaded,
    })
}

fn with_suffix(p: &std::path::Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn download_err(msg: impl Into<String>) -> HolziError {
    HolziError::ModelDownload {
        reason: msg.into(),
    }
}
