//! What the settings send and get back (spec 038, contracts/tauri-commands.md). No answer carries
//! a secret; the secret only travels in [`CredentialsInput`], from the window to Rust.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use zeroize::Zeroizing;

use super::{Addressing, CredentialsState, EndpointScope, ProviderKind, TestOutcome};

/// Credentials as typed by the user; `Debug` shows only the access key id.
#[derive(Clone, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct CredentialsInput {
    pub access_key_id: String,
    #[ts(type = "string")]
    pub secret_access_key: Zeroizing<String>,
    #[ts(type = "string | null", optional)]
    #[serde(default)]
    pub session_token: Option<Zeroizing<String>>,
}

impl fmt::Debug for CredentialsInput {
    /// Formats credential input with only the access key ID, omitting secret fields.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialsInput")
            .field("access_key_id", &self.access_key_id)
            .finish_non_exhaustive()
    }
}

/// `storage_connection_save`: a new connection without `id`, otherwise a change of one.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInput {
    #[ts(optional)]
    #[serde(default)]
    pub id: Option<String>,
    pub provider_name: String,
    pub provider_kind: ProviderKind,
    /// Empty or absent only for `aws`.
    #[ts(optional)]
    #[serde(default)]
    pub endpoint: Option<String>,
    pub region: String,
    pub addressing: Addressing,
    /// New credentials; required for a new connection.
    #[ts(optional)]
    #[serde(default)]
    pub credentials: Option<CredentialsInput>,
    /// The bucket the test before saving uses (FR-003).
    pub bucket_for_test: String,
}

/// `storage_save`: a new storage without `id`, otherwise a change of one.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct StorageInput {
    #[ts(optional)]
    #[serde(default)]
    pub id: Option<String>,
    pub connection_id: String,
    pub name: String,
    pub bucket: String,
}

/// A connection as the settings show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ConnectionView {
    pub id: String,
    pub provider_name: String,
    pub provider_kind: ProviderKind,
    pub endpoint: String,
    pub region: String,
    pub addressing: Addressing,
    /// The endpoint sends without encryption (`http`).
    pub insecure: bool,
    /// Whether the endpoint lies in the local network or on this device (research R8).
    pub endpoint_scope: EndpointScope,
    pub credentials: CredentialsState,
}

/// The last test of a storage on this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct LastTest {
    pub at: String,
    pub outcome: TestOutcome,
}

/// A storage as the settings show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct StorageView {
    pub id: String,
    pub connection_id: String,
    pub name: String,
    pub bucket: String,
    pub last_test: Option<LastTest>,
    /// The extensions with a permission for this storage.
    pub extensions: Vec<String>,
}

/// `storage_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct StorageOverview {
    pub connections: Vec<ConnectionView>,
    pub storages: Vec<StorageView>,
}

/// `storage_test`: the outcome, and the key of a test object holzi could not delete (SC-004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub outcome: TestOutcome,
    pub leftover_key: Option<String>,
}

/// `storage_removal_preview`: one of the two ids.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct RemovalTarget {
    #[ts(optional)]
    #[serde(default)]
    pub connection_id: Option<String>,
    #[ts(optional)]
    #[serde(default)]
    pub storage_id: Option<String>,
}

/// What a removal takes with it (FR-007): the storages and the extensions that lose access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct RemovalPreview {
    pub storages: Vec<String>,
    pub extensions: Vec<String>,
}
