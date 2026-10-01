//! Manual/Auto/Plan permission gate (spec.md FR-003–FR-006).
//!
//! [`decide`] is the gate's own Allow/Ask/Deny verdict, evaluated before a
//! human is ever asked. `Ask` still needs an actual approval response
//! ([`super::ApprovalDecision`]) from `respond_tool_permission` before the
//! turn loop proceeds (T026).

use super::RiskClass;
use crate::storage::preferences::{self, PrefScope};
use crate::vault_gate::VaultDb;

pub const PREF_PERMISSION_MODE: &str = "chat.permission_mode";

/// Parsed from the `chat.permission_mode` device preference (T028);
/// defaults to `Manual` when unset (spec.md Assumptions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionMode {
    #[default]
    Manual,
    Auto,
    Plan,
}

impl PermissionMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "manual" => Some(Self::Manual),
            "auto" => Some(Self::Auto),
            "plan" => Some(Self::Plan),
            _ => None,
        }
    }
}

/// Reads the vault-scoped permission mode and applies the documented Manual
/// fallback for missing or invalid values.
pub async fn read_from_vault(database: &VaultDb) -> PermissionMode {
    let raw = database
        .read(|r| preferences::get(r, PrefScope::Vault, PREF_PERMISSION_MODE))
        .await
        .ok()
        .flatten();
    raw.as_deref()
        .and_then(PermissionMode::parse)
        .unwrap_or_default()
}

/// The gate's verdict for one tool call, before any human is involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Execute immediately, no `tool-permission-request`.
    Allow,
    /// Mint a `tool-permission-request` and wait for
    /// `respond_tool_permission` (T026).
    Ask,
    /// Skip execution entirely — no request is ever emitted (Plan mode
    /// blocking a `Change` or `Risky` action, spec.md FR-006).
    Deny,
}

/// The Manual/Auto/Plan × Safe/Change/Risky decision matrix (spec.md
/// FR-003–FR-006; the `Change` stage is spec 032 FR-007, ADR-0006): Manual
/// asks for everything, Auto lets reading and changing run and asks for
/// `Risky`, Plan lets only reading run.
pub fn decide(mode: PermissionMode, risk: RiskClass) -> Decision {
    match (mode, risk) {
        (PermissionMode::Manual, _) => Decision::Ask,
        (PermissionMode::Auto, RiskClass::Safe | RiskClass::Change) => Decision::Allow,
        (PermissionMode::Auto, RiskClass::Risky) => Decision::Ask,
        (PermissionMode::Plan, RiskClass::Safe) => Decision::Allow,
        (PermissionMode::Plan, RiskClass::Change | RiskClass::Risky) => Decision::Deny,
    }
}
