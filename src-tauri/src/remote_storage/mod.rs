//! holzi's own S3 (spec 038, plan.md "Structure Decision"): storage connections (provider,
//! endpoint, region, addressing; the credentials are an entry of the password manager owned by
//! `storage`, rule Z14 of spec 034) and the storages on them, one bucket each. holzi speaks the
//! protocol itself; extensions are one consumer of it (PR D), spec 029 another.
//!
//! [`RemoteStore`] is the one seam to the provider: [`s3::S3Store`] signs with `rusty-s3` and sends
//! with `reqwest` to the address [`address`] checked and pinned (research R1, R8); tests use a fake
//! (research R10). No error of this module carries an endpoint, a header or a body of the provider.

pub mod address;
#[cfg(test)]
mod address_tests;
pub mod commands;
pub mod credentials;
#[cfg(test)]
mod credentials_tests;
pub mod model;
pub mod probe;
#[cfg(test)]
mod probe_tests;
pub mod s3;
#[cfg(test)]
mod s3_tests;
pub mod service;
#[cfg(test)]
mod service_tests;
pub mod store;
#[cfg(test)]
mod store_tests;
#[cfg(test)]
pub(crate) mod test_support;

use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use ts_rs::TS;
use zeroize::Zeroizing;

/// The provider a connection was created for; it only prefills endpoint and addressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Aws,
    Rustfs,
    Other,
}

/// How a bucket appears in the address: `https://host/<bucket>/<key>` or
/// `https://<bucket>.host/<key>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum Addressing {
    Path,
    Virtual,
}

/// Who typed the endpoint (research R8): local addresses are only reached for the user's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum EndpointOrigin {
    User,
    Extension,
}

/// The result of a connection test (research R9), stored per device in `storage_tests_no_sync`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum TestOutcome {
    Passed,
    AccessDenied,
    Unreachable,
    BucketMissing,
    MissingRight,
}

/// Whether the credentials entry of a connection is on this device (data-model.md): `syncing`
/// when it has not arrived yet, `missing` when the user deleted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum CredentialsState {
    Present,
    Syncing,
    Missing,
}

macro_rules! text_enum {
    ($ty:ty { $($variant:ident = $text:literal),+ $(,)? }) => {
        impl $ty {
            /// The text stored in the vault.
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }

            /// The value of a stored text; `None` for an unknown one.
            pub fn parse(text: &str) -> Option<Self> {
                match text { $($text => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

text_enum!(ProviderKind { Aws = "aws", Rustfs = "rustfs", Other = "other" });
text_enum!(Addressing { Path = "path", Virtual = "virtual" });
text_enum!(EndpointOrigin { User = "user", Extension = "extension" });
text_enum!(TestOutcome {
    Passed = "passed",
    AccessDenied = "accessDenied",
    Unreachable = "unreachable",
    BucketMissing = "bucketMissing",
    MissingRight = "missingRight",
});

/// A row of `haex_storage_connections`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRow {
    pub id: String,
    pub provider_name: String,
    pub provider_kind: ProviderKind,
    /// Empty for `aws`: the address follows from the region.
    pub endpoint: String,
    pub endpoint_origin: EndpointOrigin,
    pub region: String,
    pub addressing: Addressing,
    pub credentials_item_id: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A row of `haex_storages`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRow {
    pub id: String,
    pub connection_id: String,
    pub name: String,
    pub bucket: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Credentials of a connection; `Debug` shows only the access key id.
#[derive(Clone)]
pub struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: Zeroizing<String>,
    pub session_token: Option<Zeroizing<String>>,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"<redacted>")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// Where one bucket lives: the connection's address data plus the bucket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub provider_kind: ProviderKind,
    pub endpoint: String,
    pub endpoint_origin: EndpointOrigin,
    pub region: String,
    pub addressing: Addressing,
    pub bucket: String,
}

impl Location {
    /// The location of `bucket` on `connection`.
    pub fn of(connection: &ConnectionRow, bucket: &str) -> Self {
        Self {
            provider_kind: connection.provider_kind,
            endpoint: connection.endpoint.clone(),
            endpoint_origin: connection.endpoint_origin,
            region: connection.region.clone(),
            addressing: connection.addressing,
            bucket: bucket.to_owned(),
        }
    }
}

/// What a call needs to reach a bucket.
#[derive(Debug, Clone)]
pub struct Access {
    pub location: Location,
    pub credentials: Credentials,
}

/// One object of a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
}

/// What went wrong at the provider, without its text (research R7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    /// The object or the bucket does not exist.
    #[error("not found")]
    NotFound,
    /// The provider refused the credentials (wrong key, wrong signature, expired token).
    #[error("access denied")]
    AccessDenied,
    /// The credentials are valid but lack the right for this action (research R9 `missingRight`).
    #[error("missing right")]
    MissingRight,
    /// The provider could not be reached, answered with a server error or a redirect, or its
    /// address is not allowed (research R8).
    #[error("network")]
    Network,
    /// An answer or a listing is larger than the limit.
    #[error("too large")]
    TooLarge,
    /// The deadline passed.
    #[error("timed out")]
    TimedOut,
}

/// The provider behind a storage (research R10): the S3 implementation, or a fake in tests. Every
/// call ends at `deadline`.
#[async_trait]
pub trait RemoteStore: Send + Sync {
    async fn put(
        &self,
        access: &Access,
        key: &str,
        body: Vec<u8>,
        deadline: Instant,
    ) -> Result<(), StorageError>;

    /// The object, read up to `max_bytes` (more is [`StorageError::TooLarge`]).
    async fn get(
        &self,
        access: &Access,
        key: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Vec<u8>, StorageError>;

    /// The objects under `prefix`, following the provider's pages up to `max` (more is
    /// [`StorageError::TooLarge`]).
    async fn list(
        &self,
        access: &Access,
        prefix: &str,
        max: usize,
        deadline: Instant,
    ) -> Result<Vec<ObjectInfo>, StorageError>;

    async fn delete(
        &self,
        access: &Access,
        key: &str,
        deadline: Instant,
    ) -> Result<(), StorageError>;
}
