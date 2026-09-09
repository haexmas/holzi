//! Frontend-facing metadata for a single managed instance.
//!
//! Exported to TS via ts-rs (`src/types/bindings/InstanceInfo.ts`). Field
//! names are camelCase per the wire contract; internal Rust code uses
//! snake_case through serde rename.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    /// Basename without the `.db` extension.
    pub name: String,
    /// Human-readable alias if set (from `known_devices`); `None` for
    /// list results, filled later when needed.
    pub alias: Option<String>,
    /// Milliseconds since UNIX epoch; sourced from the DB file's mtime.
    #[ts(type = "number")]
    pub last_access: u64,
}
