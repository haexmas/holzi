//! Errors an extension receives over the bridge (spec 017, contracts/bridge.md §Fehlercodes).
//!
//! The codes are haex-vault's (`src-tauri/src/extension/error.rs` @ `8dce379`), so the vault-sdk
//! understands them unchanged — `1002` denied and `1004` prompt required drive its retry logic —
//! plus three of holzi's own. A message never names a table, file or entry outside the caller's
//! grants (FR-062): the caller learns that something is refused, not whether it exists.

use serde::Serialize;
use serde_json::Value;

/// The numeric error code of a bridge answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ExtensionErrorCode {
    /// A form that cannot be attributed, a blocked path, an unknown frame.
    SecurityViolation = 1000,
    NotFound = 1001,
    /// `PermissionErrorCode.DENIED` of the vault-sdk.
    PermissionDenied = 1002,
    /// `PermissionErrorCode.PROMPT_REQUIRED` of the vault-sdk: it waits for
    /// `extension:permission-resolved` and retries.
    PermissionPromptRequired = 1004,
    Database = 2000,
    Filesystem = 2001,
    Http = 2002,
    Shell = 2003,
    Web = 2005,
    Manifest = 3000,
    Validation = 3001,
    /// Rows, run time, size or concurrent requests over the limit (FR-031).
    LimitExceeded = 7000,
    /// The method is unknown or deliberately not offered (FR-060, FR-061).
    NotSupported = 8000,
    /// The function does not exist on this device or in this build (FR-053, FR-059, FR-066).
    NotAvailable = 8001,
    /// The extension is disabled (FR-007).
    Disabled = 8002,
}

impl ExtensionErrorCode {
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

impl Serialize for ExtensionErrorCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u16(self.as_u16())
    }
}

/// The `error` of a bridge answer: `{code, message, details?}` as the vault-sdk reads it.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[error("{message} ({code:?})")]
pub struct BridgeError {
    pub code: ExtensionErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl BridgeError {
    pub fn new(code: ExtensionErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// The answer for a method holzi does not offer.
    pub fn not_supported() -> Self {
        Self::new(ExtensionErrorCode::NotSupported, "not supported")
    }

    /// The answer for a function this device or build does not have.
    pub fn not_available() -> Self {
        Self::new(ExtensionErrorCode::NotAvailable, "not available")
    }

    /// The answer for every request of a disabled extension.
    pub fn disabled() -> Self {
        Self::new(ExtensionErrorCode::Disabled, "extension disabled")
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
