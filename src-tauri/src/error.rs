//! Top-level error type surfaced across the Tauri boundary. Contract:
//! [`specs/001-frontend-onboarding/contracts/tauri-commands.md`](../../../specs/001-frontend-onboarding/contracts/tauri-commands.md).

use serde::Serialize;
use ts_rs::TS;

use haex_crdt::db::error::DatabaseError;
use haex_crdt::{Error as CrdtError, MigrationJournal};

#[derive(Debug, thiserror::Error, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(tag = "kind")]
pub enum HolziError {
    #[error("Instance '{name}' already exists")]
    NameConflict { name: String },

    #[error("Invalid instance name: {reason}")]
    InvalidName { reason: String },

    #[error("Passphrase does not meet policy: {reason}")]
    WeakPassphrase { reason: String },

    #[error("Wrong passphrase")]
    WrongPassphrase,

    #[error("No instance named '{name}'")]
    NotFound { name: String },

    #[error("An instance is already active in this process")]
    InstanceAlreadyActive,

    #[error("The vault is closed")]
    VaultClosed,

    #[error("A vault is already open in this app process")]
    VaultAlreadyActive,

    #[error("Vault file is locked by another process on this host")]
    VaultAlreadyOpenElsewhere,

    #[error("Instance '{name}' is active; close it before this operation")]
    InstanceActive { name: String },

    #[error("File is not a valid holzi instance: {reason}")]
    NotAValidInstance { reason: String },

    #[error("SQLCipher error from haex-crdt: {reason}")]
    CrdtSqlite { reason: String },

    #[error("I/O error from haex-crdt: {reason}")]
    CrdtIo { reason: String },

    #[error("HLC error from haex-crdt: {reason}")]
    CrdtHlc { reason: String },

    #[error("Device UUID mismatch: expected {expected}, supplied {supplied}")]
    DeviceIdMismatch { expected: String, supplied: String },

    #[error("Migration {name} from {journal} is missing from MigrationSource")]
    MigrationMissingFromSource { journal: String, name: String },

    #[error("Migration {name} content drift between database and MigrationSource")]
    MigrationContentDrift {
        name: String,
        expected: String,
        found: String,
    },

    #[error("Legacy schema is incompatible: {reason}")]
    MigrationCompatibility { reason: String },

    #[error("Remote CRDT signature verification failed at change #{first_failed_change}")]
    CrdtSignatureVerificationFailed { first_failed_change: usize },

    #[error("Unexpected non-empty signature under NoopSignatureProvider")]
    CrdtUnexpectedSignatureUnderNoop,

    #[error("CRDT already installed for table {table}")]
    CrdtAlreadyInstalled { table: String },

    #[error("haex-crdt init failed: {reason}")]
    CrdtInit { reason: String },

    #[error("Path resolution failed: {reason}")]
    PathResolution { reason: String },

    #[error("I/O error: {reason}")]
    Io { reason: String },

    #[error("No active instance for this operation")]
    NoActiveInstance,

    /// An action only a main device may take (spec 024, FR-024); the backend checks, not
    /// only the interface.
    #[error("This device is not a main device")]
    NotMainDevice,

    #[error("Catalog entry not found: {id}")]
    CatalogEntryNotFound { id: String },

    #[error("Model download failed: {reason}")]
    ModelDownload { reason: String },

    #[error("Model import failed: {reason}")]
    ModelImport { reason: String },

    #[error("Model not found: {id}")]
    ModelNotFound { id: String },

    #[error("Idempotency key must not be empty")]
    InvalidIdempotencyKey,

    #[error("Idempotency key conflicts with an existing request")]
    IdempotencyKeyConflict,

    #[error("Invalid input: {reason}")]
    InvalidInput { reason: String },

    /// A window manager session above `wm_session::MAX_SESSION_BYTES` (spec
    /// 022-session-restore); the frontend retries once without tab histories.
    #[error("The saved session is too large ({bytes} bytes)")]
    SessionTooLarge {
        #[ts(type = "number")]
        bytes: usize,
    },

    /// The parameters of one vault write transaction exceed
    /// `DatabaseConfig::max_transaction_bytes` (spec 024, research R20); nothing was written.
    #[error("The change is too large to save ({bytes} bytes, limit {limit})")]
    TransactionTooLarge {
        #[ts(type = "number")]
        bytes: usize,
        #[ts(type = "number")]
        limit: usize,
    },

    // --- HuggingFace discovery / install (spec 005) ---------------------
    #[error("Network error contacting Hugging Face: {reason}")]
    Network { reason: String },

    #[error("Hugging Face request timed out: {reason}")]
    Timeout { reason: String },

    #[error("Hugging Face returned HTTP {status}: {reason}")]
    HttpStatus { status: u16, reason: String },

    #[error("Hugging Face rate-limited this request")]
    RateLimited {
        #[ts(type = "number | null")]
        retry_after_seconds: Option<u64>,
    },

    #[error("File is not an installable GGUF: {filename}")]
    UnsupportedFormat { filename: String },

    #[error("A tokenizer repository is required for {repo_id}")]
    TokenizerRequired { repo_id: String },

    #[error("Model size requires explicit hardware confirmation: {fit}")]
    HardwareConfirmationRequired { fit: String },

    #[error("Failed to register downloaded model: {reason}")]
    ModelRegistrationFailed { reason: String },

    // --- Local file integrity (spec 005 Entscheidung 6) ------------------
    #[error("Model {model_id} local file does not match its expected hash")]
    ModelIntegrityMismatch {
        model_id: String,
        expected_sha256: String,
        actual_sha256: String,
    },

    #[error("Model {model_id} has no verifiable expected hash")]
    ModelIntegrityUnknown {
        model_id: String,
        expected_sha256: Option<String>,
    },

    #[error("Model {model_id} could not be hashed: {reason}")]
    ModelIntegrityError { model_id: String, reason: String },

    // --- Voice control (spec 008) -----------------------------------------
    #[error("Microphone permission is required")]
    PermissionDenied,

    #[error("A voice recording is already in progress")]
    AlreadyRecording,

    #[error("No microphone input device is available")]
    DeviceUnavailable,

    #[error("No voice recording is in progress")]
    NotRecording,

    #[error("Transcription failed: {reason}")]
    TranscriptionFailed { reason: String },

    // --- Password manager (spec 034, contracts/tauri-commands.md §Fehlerarten) ---
    // None of these carries the value of a secret in a field.
    /// The entry, attachment or passkey does not exist, or the caller may not know that it does
    /// (rule Z5 of `contracts/access.md`).
    #[error("Not found")]
    PasswordsNotFound,

    /// The caller has no grant for this operation (rules Z3 and Z11).
    #[error("Not permitted")]
    PasswordsForbidden,

    /// A save found the entry changed or deleted since it was read (`reason`: `changed` or
    /// `deleted`).
    #[error("Conflict: {reason}")]
    PasswordsConflict { reason: String },

    #[error("The attachment is too large ({bytes} bytes, limit {limit})")]
    PasswordsAttachmentTooLarge {
        #[ts(type = "number")]
        bytes: u64,
        #[ts(type = "number")]
        limit: u64,
    },

    /// An import stopped as a whole (`unreadable`, `wrong_credentials`, `corrupt`,
    /// `unsupported_format`, `encrypted_export`).
    #[error("Import failed: {reason}")]
    PasswordsImportFailed { reason: String },

    /// An extension bundle was refused or could not be installed (spec 017,
    /// contracts/bundle-format.md §Prüfung: `reason` is the error kind, e.g. `signature_invalid`,
    /// `file_mismatch`, `legacy_signature_format`). Never carries content of the bundle.
    #[error("Extension not installed: {reason}")]
    ExtensionInstall { reason: String },

    /// No installed extension with this id.
    #[error("Extension not found")]
    ExtensionNotFound,

    /// The extension cannot start on this device yet (`status`: `transferring`,
    /// `signature_failed`, `migration_failed`).
    #[error("Extension not ready: {status}")]
    ExtensionNotReady { status: String },

    /// The extension is disabled (FR-007).
    #[error("Extension disabled")]
    ExtensionDisabled,
}

pub type Result<T> = std::result::Result<T, HolziError>;

impl From<CrdtError> for HolziError {
    /// Maps a `haex-crdt` error into the stable error contract exposed by holzi.
    fn from(err: CrdtError) -> Self {
        match err {
            CrdtError::Sqlite(e) => HolziError::CrdtSqlite {
                reason: e.to_string(),
            },
            CrdtError::Io(e) => HolziError::CrdtIo {
                reason: e.to_string(),
            },
            CrdtError::Hlc(reason) => HolziError::CrdtHlc { reason },
            CrdtError::SignatureVerificationFailed {
                first_failed_change,
            } => HolziError::CrdtSignatureVerificationFailed {
                first_failed_change,
            },
            CrdtError::UnexpectedSignatureUnderNoop => HolziError::CrdtUnexpectedSignatureUnderNoop,
            CrdtError::MigrationMissingFromSource { journal, name } => {
                HolziError::MigrationMissingFromSource {
                    journal: journal_label(journal).to_string(),
                    name,
                }
            }
            CrdtError::MigrationContentDrift {
                name,
                expected,
                found,
            } => HolziError::MigrationContentDrift {
                name,
                expected,
                found,
            },
            CrdtError::MigrationCompatibility { reason } => {
                HolziError::MigrationCompatibility { reason }
            }
            CrdtError::CrdtAlreadyInstalled { table } => HolziError::CrdtAlreadyInstalled { table },
            CrdtError::Database(DatabaseError::VaultAlreadyOpenElsewhere { .. }) => {
                HolziError::VaultAlreadyOpenElsewhere
            }
            CrdtError::Database(DatabaseError::TransactionTooLarge { bytes, limit }) => {
                HolziError::TransactionTooLarge { bytes, limit }
            }
            // A `HolziError` returned from inside a `Database::write` / `Database::read`
            // closure travels as `Consumer` and comes back out unchanged.
            CrdtError::Consumer(source) => match source.downcast::<HolziError>() {
                Ok(holzi) => *holzi,
                Err(source) => HolziError::CrdtInit {
                    reason: source.to_string(),
                },
            },
            // RemoteHlcDriftTooLarge and the remaining database-layer variants have no
            // dedicated holzi variant yet; sync (spec 024) adds the ones it surfaces.
            other => HolziError::CrdtInit {
                reason: other.to_string(),
            },
        }
    }
}

impl From<HolziError> for CrdtError {
    /// Carries a holzi error out of a `Database::write` / `Database::read` closure; the
    /// reverse mapping above unwraps it again.
    fn from(err: HolziError) -> Self {
        CrdtError::consumer(err)
    }
}

impl From<std::io::Error> for HolziError {
    /// Wraps application-level I/O failures for transport across the Tauri boundary.
    fn from(e: std::io::Error) -> Self {
        HolziError::Io {
            reason: e.to_string(),
        }
    }
}

/// Returns the database table name associated with a migration journal.
fn journal_label(j: MigrationJournal) -> &'static str {
    match j {
        MigrationJournal::CrateOwned => "haex_crdt_migrations_no_sync",
        MigrationJournal::ConsumerOwned => "haex_app_migrations_no_sync",
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
