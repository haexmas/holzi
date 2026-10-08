//! Files in holzi. Spec 043: files the person hands to holzi, chosen in the system's dialog, read
//! and written the same way on every platform (`picked`, `commands`). Spec 044: the file browser's
//! core, files of the device and of storages for the window, the built-in agent and extensions
//! (`local`, [`FilesError`]).

pub mod commands;
pub mod local;
pub mod picked;

pub use picked::PickedFile;

#[cfg(test)]
pub mod test_support;

use serde::Serialize;
use ts_rs::TS;

/// Why a file operation was refused (contracts/tauri-commands.md). The message never holds
/// credentials or file contents (FR-033, FR-038).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
#[error("{message}")]
pub struct FilesError {
    pub code: FilesErrorCode,
    pub message: String,
}

/// The kinds of [`FilesError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub enum FilesErrorCode {
    /// The path is not absolute, holds `.`/`..` after its existing part, or a broken link.
    InvalidPath,
    Exists,
    IntoItself,
    NoSpace,
    HolziOwned,
    NoAccess,
    NotFound,
    Binary,
    Broken,
    TooLarge,
    Blocked,
    NotGranted,
    Unsupported,
}

impl FilesError {
    pub fn new(code: FilesErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_path(message: impl Into<String>) -> Self {
        Self::new(FilesErrorCode::InvalidPath, message)
    }
}
