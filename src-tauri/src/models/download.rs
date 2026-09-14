//! HuggingFace GGUF download with progress reporting.
//!
//! The download writes to `<final>.part` and renames on success so a
//! crashed or aborted download never leaves a partial file that looks
//! valid to the loader. A transfer that dies mid-body is Range-resumed
//! from the bytes already on disk, and the `.part` file is only promoted
//! to its final name once the transferred size matches the size the
//! server announced — a short body is retried, never finalized.
//!
//! The total size comes from `Content-Length` (or the `Content-Range`
//! total on a resumed response) and is reported through the progress
//! callback; if the server omits it (rare but allowed by HTTP), the
//! total is `None` and the UI shows an indeterminate bar.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures::StreamExt;
use reqwest::header::{
    HeaderValue, ACCEPT_ENCODING, CONTENT_RANGE, ETAG, IF_RANGE, LAST_MODIFIED, RANGE,
};
use reqwest::{Client, StatusCode};
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};

use crate::error::{HolziError, Result};

/// Minimum wall-clock gap between progress reports. Guards against
/// firing thousands of Tauri events per second on fast local networks.
const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(100);

/// A CDN connection can end cleanly at the TCP layer while the HTTP body is
/// still incomplete. Range-resuming makes those transient failures recoverable
/// without keeping a corrupted finalized model.
const MAX_TRANSFER_ATTEMPTS: u8 = 4;
const RETRY_BACKOFF: Duration = Duration::from_millis(250);

/// A stalled CDN connection that never closes would otherwise hang the
/// download forever: with nothing failing, the retry loop never engages.
/// Applies per read, not to the whole transfer, so large GGUFs are safe.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

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
        .read_timeout(READ_TIMEOUT)
        .user_agent(concat!("holzi/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| download_err(format!("http client: {e}")))?;

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

    let mut bytes_downloaded: u64 = 0;
    let mut bytes_total = None;
    let mut last_report = Instant::now() - PROGRESS_MIN_INTERVAL;
    // `ETag`/`Last-Modified` of the entity the first response served, replayed
    // as `If-Range` so a resume can never splice bytes from a different
    // revision or mirror onto what is already on disk.
    let mut entity_validator: Option<HeaderValue> = None;
    let mut completed = false;
    let mut last_failure: Option<String> = None;

    for attempt in 0..MAX_TRANSFER_ATTEMPTS {
        let is_final_attempt = attempt + 1 >= MAX_TRANSFER_ATTEMPTS;
        let requested_offset = bytes_downloaded;
        file.seek(SeekFrom::Start(requested_offset))
            .await
            .map_err(|e| download_err(format!("seek {}: {e}", part_path.display())))?;

        let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
        if requested_offset > 0 {
            request = request.header(RANGE, format!("bytes={requested_offset}-"));
            if let Some(validator) = entity_validator.clone() {
                request = request.header(IF_RANGE, validator);
            }
        }

        let resp = match request.send().await {
            Ok(resp) => resp,
            Err(e) => {
                last_failure = Some(format!("GET {url}: {e}"));
                if is_final_attempt {
                    break;
                }
                tokio::time::sleep(RETRY_BACKOFF * u32::from(attempt + 1)).await;
                continue;
            }
        };

        let status = resp.status();
        // The server cannot serve the offset we asked for — the entity shrank
        // or was replaced. Starting over is recoverable; failing is not.
        if status == StatusCode::RANGE_NOT_SATISFIABLE && requested_offset > 0 {
            restart_from_zero(&mut file, &part_path, &mut bytes_downloaded).await?;
            entity_validator = None;
            bytes_total = None;
            last_failure = Some(format!("GET {url} returned status {status}"));
            continue;
        }
        if !status.is_success() {
            last_failure = Some(format!("GET {url} returned status {status}"));
            if is_final_attempt || !is_transient_status(status) {
                break;
            }
            tokio::time::sleep(RETRY_BACKOFF * u32::from(attempt + 1)).await;
            continue;
        }

        let range_was_honored = requested_offset > 0 && status == StatusCode::PARTIAL_CONTENT;
        if requested_offset > 0 && !range_was_honored {
            // Some proxies/CDNs ignore Range, and an `If-Range` miss answers
            // 200 by design. Appending a full body would create a corrupt
            // model, so restart this attempt from an empty file.
            restart_from_zero(&mut file, &part_path, &mut bytes_downloaded).await?;
            // The body about to arrive is the authority on the size now, so
            // drop the old total instead of letting `.or` below preserve it.
            bytes_total = None;
            // Same reasoning for the validator: the stored one no longer
            // identifies what the server is serving, and replaying it would
            // make every later resume miss `If-Range` and restart again.
            entity_validator = None;
            on_progress(DownloadProgress {
                bytes_downloaded,
                bytes_total,
            });
            last_report = Instant::now();
        }

        if entity_validator.is_none() {
            entity_validator = resp
                .headers()
                .get(ETAG)
                .or_else(|| resp.headers().get(LAST_MODIFIED))
                .cloned();
        }

        let response_length = resp.content_length();
        let announced_total = if range_was_honored {
            resp.headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.rsplit_once('/'))
                .and_then(|(_, total)| total.parse::<u64>().ok())
                .or_else(|| response_length.map(|length| requested_offset + length))
        } else {
            response_length
        };
        // A retry whose response omits both headers must not erase a total the
        // UI is already rendering a determinate bar from.
        bytes_total = announced_total.or(bytes_total);

        let mut stream = resp.bytes_stream();
        let mut stream_error = None;
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(e) => {
                    stream_error = Some(e);
                    break;
                }
            };
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

        // A body can also end cleanly while short of the announced size — a
        // server may answer a Range request with fewer bytes than asked for.
        // Treat that exactly like a stream error: retry, never finalize.
        let short_body = bytes_total.is_some_and(|total| bytes_downloaded < total);
        if stream_error.is_none() && !short_body {
            completed = true;
            break;
        }

        file.flush()
            .await
            .map_err(|flush| download_err(format!("flush after interrupted transfer: {flush}")))?;
        last_failure = Some(match (stream_error, bytes_total) {
            (Some(e), _) => format!("stream: {e}"),
            (None, Some(total)) => {
                format!("body ended after {bytes_downloaded} of {total} bytes")
            }
            (None, None) => "body ended unexpectedly".to_string(),
        });
        if is_final_attempt {
            break;
        }
        tokio::time::sleep(RETRY_BACKOFF * u32::from(attempt + 1)).await;
    }

    if !completed {
        let reason = last_failure.unwrap_or_else(|| format!("GET {url} did not complete"));
        return Err(download_err(format!(
            "{reason} (gave up after {MAX_TRANSFER_ATTEMPTS} attempts)"
        )));
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

/// Empties the `.part` file and rewinds both the handle and the byte
/// counter so the next attempt writes a whole body from offset zero.
async fn restart_from_zero(
    file: &mut File,
    part_path: &std::path::Path,
    bytes_downloaded: &mut u64,
) -> Result<()> {
    file.set_len(0)
        .await
        .map_err(|e| download_err(format!("truncate {}: {e}", part_path.display())))?;
    file.seek(SeekFrom::Start(0))
        .await
        .map_err(|e| download_err(format!("seek {}: {e}", part_path.display())))?;
    *bytes_downloaded = 0;
    Ok(())
}

/// Statuses a CDN returns while it is briefly unable to serve, as opposed
/// to a 404 or a 403 that will answer the same way on every retry.
fn is_transient_status(status: StatusCode) -> bool {
    status.is_server_error()
        || status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
}

fn with_suffix(p: &std::path::Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn download_err(msg: impl Into<String>) -> HolziError {
    HolziError::ModelDownload { reason: msg.into() }
}

#[cfg(test)]
#[path = "download_tests.rs"]
mod tests;
