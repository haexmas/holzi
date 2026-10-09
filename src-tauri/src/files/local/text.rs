//! Text for the viewer (spec 044 FR-015): up to [`TEXT_LIMIT`] bytes, never the whole of a larger
//! file; a NUL byte near the start marks a binary file, which goes to the info view instead.

use std::io::Read;
use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use crate::files::local::ops::io_error;
use crate::files::{FilesError, FilesErrorCode};

/// The most the viewer shows of a text file (5 MB).
pub const TEXT_LIMIT: u64 = 5 * 1024 * 1024;

/// How far into a file a NUL byte marks it as binary.
const SNIFF: usize = 8 * 1024;

/// A text for the viewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct TextContent {
    pub text: String,
    /// The file is longer than [`TEXT_LIMIT`]; only its start is in `text`.
    pub truncated: bool,
}

/// The text of the file at `path`, at most `limit` bytes; invalid UTF-8 is replaced.
pub fn read_text(path: &Path, limit: u64) -> Result<TextContent, FilesError> {
    let file = std::fs::File::open(path).map_err(|error| io_error(error, path))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(error, path))?;
    text_from(bytes, limit)
}

/// The text of the first bytes of a file: `bytes` holds up to `limit + 1` of them, so one more
/// than `limit` means there is more (a storage reads them the same way, spec 044 US5).
pub fn text_from(mut bytes: Vec<u8>, limit: u64) -> Result<TextContent, FilesError> {
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    if bytes[..bytes.len().min(SNIFF)].contains(&0) {
        return Err(FilesError::new(FilesErrorCode::Binary, "not a text file"));
    }
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    Ok(TextContent {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        truncated,
    })
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
