//! The host for haextensions (spec 017, ADR-0004, ADR-0008): signed web bundles that run in
//! sandboxed frames and reach holzi only through one checkpoint in Rust.
//!
//! This module grows with the deliveries of the spec (research R1). The foundation holds the
//! identifiers, the error codes of the bridge and the pure permission model.

pub mod error;
pub mod ids;
pub mod permissions;

/// Largest `.xt` file holzi opens (contracts/bundle-format.md).
pub const MAX_BUNDLE_BYTES: u64 = 64 * 1024 * 1024;
/// Largest single file inside a bundle, unpacked; the proven attachment size of spec 034.
pub const MAX_ENTRY_BYTES: u64 = 25 * 1024 * 1024;
/// Largest sum of all files of a bundle, unpacked.
pub const MAX_TOTAL_UNPACKED: u64 = 64 * 1024 * 1024;
/// Most entries a bundle may have.
pub const MAX_ENTRIES: usize = 2000;
/// Largest compression ratio of one entry (zip bombs).
pub const MAX_RATIO: u64 = 200;
/// Days an unreferenced blob is kept before it is freed, as for attachments in spec 034: another
/// device may be about to reference it.
pub const BLOB_ORPHAN_GRACE_DAYS: u64 = 7;

/// Default limits of an extension when `extension_limits` has no row (data-model.md).
pub mod default_limits {
    pub const MAX_ROWS: u64 = 10_000;
    pub const MAX_CONCURRENT: u64 = 20;
    pub const MAX_SQL_BYTES: u64 = 1_000_000;
    pub const TIMEOUT_MS: u64 = 5_000;
    pub const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
}
