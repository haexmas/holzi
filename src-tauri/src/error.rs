//! Top-level error type surfaced across the Tauri boundary. Contract:
//! [`specs/001-frontend-onboarding/contracts/tauri-commands.md`](../../../specs/001-frontend-onboarding/contracts/tauri-commands.md).

use serde::Serialize;
use ts_rs::TS;

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

    #[error("Vault file is locked by another process on this host")]
    VaultAlreadyOpenElsewhere,

    #[error("Instance '{name}' is active; close it before this operation")]
    InstanceActive { name: String },

    #[error("File is not a valid holzi instance: {reason}")]
    NotAValidInstance { reason: String },

    #[error("Failed to close active instance: {reason}")]
    CloseFailed { reason: String },

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

    #[error("Catalog entry not found: {id}")]
    CatalogEntryNotFound { id: String },

    #[error("Model download failed: {reason}")]
    ModelDownload { reason: String },

    #[error("Model import failed: {reason}")]
    ModelImport { reason: String },

    #[error("Model not found: {id}")]
    ModelNotFound { id: String },

    #[error("Invalid input: {reason}")]
    InvalidInput { reason: String },
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
            // RemoteHlcDriftTooLarge is not part of the MVP mapping — sync
            // is deferred. Catch-all Message covers it until sync lands.
            other => HolziError::CrdtInit {
                reason: other.to_string(),
            },
        }
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
