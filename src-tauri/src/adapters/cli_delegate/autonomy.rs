//! Autonomy Mode (spec 009-autonomous-delegate-mode): `AutonomyMode`,
//! `DenyCategory`, the per-vendor `evaluate_deny_rules` matcher, the
//! `cli_delegate.deny_rules` device preference, and the reactive
//! unsupported-mode error classifier. See
//! `specs/009-autonomous-delegate-mode/data-model.md` for the full design.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::chat::tools::ApprovalDecision;
use crate::storage::preferences::{self, PrefScope};

use super::DelegateVendor;

/// Per-request posture for a `cli_delegate` invocation (spec FR-001/FR-002).
/// Carried on `ChatRequest`, never persisted as a preference (FR-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyMode {
    #[default]
    Standard,
    Ungated,
    GatedPermissive,
}

impl AutonomyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AutonomyMode::Standard => "standard",
            AutonomyMode::Ungated => "ungated",
            AutonomyMode::GatedPermissive => "gated_permissive",
        }
    }
}

/// One operator-configured, persisted (spec FR-014) deny category enforced
/// under `GatedPermissive`. Fixed, small set (research.md §4) — no
/// general-purpose rule engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenyCategory {
    WorkspaceEscape,
    NetworkAccess,
    CredentialPaths,
}

/// Host/protocol context Codex's `networkApprovalContext` carries on a
/// `CommandExecutionRequestApprovalParams` when the call is network-
/// triggered. Only its presence (`Some`/`None`) drives matching today
/// (data-model.md); the fields exist so the caller can log/display them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkContext {
    pub host: Option<String>,
    pub protocol: Option<String>,
}

/// What a vendor's approval callback actually provides — deliberately not a
/// lowest-common-denominator struct, so the real per-vendor asymmetry
/// (data-model.md) is visible in the type rather than hidden behind
/// optional fields. Identifies the vendor itself, so no separate
/// `DelegateVendor` parameter is needed alongside it.
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalRequestPayload {
    /// `input` carries `file_path` directly when the tool is a file-editing
    /// tool (`Write`/`Edit`/...).
    ClaudeToolCall { tool_name: String, input: Value },
    CodexCommandExecution {
        command: String,
        cwd: Option<String>,
        network: Option<NetworkContext>,
    },
    /// Deliberately field-less: `FileChangeRequestApprovalParams` exposes
    /// no path (research.md §3) — not a placeholder to fill in later.
    CodexFileChange,
}

/// Fixed glob-like substrings for the `CredentialPaths` category
/// (research.md §4's named examples — not an exhaustive list by design).
const CREDENTIAL_PATH_PATTERNS: &[&str] = &[".ssh/", ".aws/", ".env", "id_rsa"];

fn matches_credential_pattern(value: Option<&str>) -> bool {
    value
        .map(|v| CREDENTIAL_PATH_PATTERNS.iter().any(|pattern| v.contains(pattern)))
        .unwrap_or(false)
}

/// Any command-execution-shaped or explicitly network-named Claude tool
/// call is denied outright while `NetworkAccess` is enabled: Claude's
/// approval payload carries no structured network signal, so even a
/// command that doesn't obviously touch the network cannot be positively
/// ruled out — data-model.md's documented fail-closed default for this
/// category (recognized cases like `curl`/`wget` are denied for the same
/// reason, not a different one).
fn claude_network_denied(tool_name: &str) -> bool {
    matches!(
        tool_name.to_ascii_lowercase().as_str(),
        "webfetch" | "websearch" | "bash"
    )
}

/// Resolves a lexically (no filesystem access — the path may not exist
/// yet) normalized form of `path`, joined onto `workspace_root` first when
/// relative.
fn resolve_lexically(path: &str, workspace_root: &Path) -> PathBuf {
    let path = Path::new(path);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn is_within_workspace(candidate: &str, workspace_root: &Path) -> bool {
    resolve_lexically(candidate, workspace_root).starts_with(workspace_root)
}

fn category_denies(
    category: DenyCategory,
    request: &ApprovalRequestPayload,
    workspace_root: &Path,
) -> bool {
    match (category, request) {
        (DenyCategory::WorkspaceEscape, ApprovalRequestPayload::ClaudeToolCall { input, .. }) => {
            input
                .get("file_path")
                .and_then(Value::as_str)
                .map(|path| !is_within_workspace(path, workspace_root))
                .unwrap_or(false)
        }
        (
            DenyCategory::WorkspaceEscape,
            ApprovalRequestPayload::CodexCommandExecution { cwd, .. },
        ) => cwd
            .as_deref()
            .map(|cwd| !is_within_workspace(cwd, workspace_root))
            .unwrap_or(false),
        // FR-015: no path field on this payload at all — cannot evaluate,
        // fail closed rather than allow an unverifiable call through.
        (DenyCategory::WorkspaceEscape, ApprovalRequestPayload::CodexFileChange) => true,

        (
            DenyCategory::CredentialPaths,
            ApprovalRequestPayload::ClaudeToolCall { input, .. },
        ) => {
            matches_credential_pattern(input.get("file_path").and_then(Value::as_str))
                || matches_credential_pattern(input.get("command").and_then(Value::as_str))
        }
        (
            DenyCategory::CredentialPaths,
            ApprovalRequestPayload::CodexCommandExecution { command, .. },
        ) => matches_credential_pattern(Some(command.as_str())),
        // FR-015: same unevaluable-payload gap as WorkspaceEscape.
        (DenyCategory::CredentialPaths, ApprovalRequestPayload::CodexFileChange) => true,

        (
            DenyCategory::NetworkAccess,
            ApprovalRequestPayload::ClaudeToolCall { tool_name, .. },
        ) => claude_network_denied(tool_name),
        (
            DenyCategory::NetworkAccess,
            ApprovalRequestPayload::CodexCommandExecution { network, .. },
        ) => network.is_some(),
        // Not a network-triggered approval kind; category does not apply.
        (DenyCategory::NetworkAccess, ApprovalRequestPayload::CodexFileChange) => false,
    }
}

/// The `GatedPermissive` approval-callback evaluator (data-model.md).
/// Returns `Deny` if any enabled category matches (including the FR-015
/// unevaluable-but-enabled case), `Allow` otherwise — an empty `enabled`
/// slice always allows (spec's documented edge case: no deny rules
/// configured means fully permissive).
///
/// This evaluates only approval callbacks the vendor actually emits; a
/// tool call for which no callback fires never reaches this function
/// (data-model.md).
pub fn evaluate_deny_rules(
    enabled: &[DenyCategory],
    request: &ApprovalRequestPayload,
    workspace_root: &Path,
) -> ApprovalDecision {
    if enabled
        .iter()
        .any(|category| category_denies(*category, request, workspace_root))
    {
        ApprovalDecision::Deny
    } else {
        ApprovalDecision::Allow
    }
}

/// A requested `Ungated`/`GatedPermissive` invocation could not be started
/// because the installed CLI does not support the native mechanism this
/// mode needs (spec FR-013/SC-006) — not a generic spawn failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutonomyUnavailable {
    pub vendor: DelegateVendor,
    pub mode: AutonomyMode,
}

impl std::fmt::Display for AutonomyUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "autonomy mode unavailable: vendor={}; mode={}",
            self.vendor.as_str(),
            self.mode.as_str()
        )
    }
}

impl std::error::Error for AutonomyUnavailable {}

/// Recognizes a vendor's error text as "this CLI version does not support
/// the requested autonomy mode" rather than a generic failure. Reactive
/// (classify an actual failure) rather than a proactive CLI-version probe
/// (research.md §2/§ — matches how 007 already surfaces "backend
/// unavailable" for its own other failure classes, spec FR-008
/// precedent).
pub fn classify_autonomy_spawn_error(
    vendor: DelegateVendor,
    mode: AutonomyMode,
    raw_error: &str,
) -> Option<AutonomyUnavailable> {
    let recognized = match vendor {
        // An old `claude` rejects an unknown `--permission-mode` enum
        // value on its own argument parser, e.g.:
        // "error: option '--permission-mode <mode>' argument
        // 'bypassPermissions' is invalid."
        DelegateVendor::Claude => {
            raw_error.contains("--permission-mode") && raw_error.contains("invalid")
        }
        // An old `codex app-server` rejects an unrecognized `thread/start`
        // field with a JSON-RPC error mentioning the field name.
        DelegateVendor::Codex => {
            raw_error.contains("approvalPolicy") || raw_error.contains("sandbox")
        }
    };
    recognized.then_some(AutonomyUnavailable { vendor, mode })
}

/// Device-scoped preference key for the persisted `DenyCategory` set
/// (spec FR-014), following the `chat.permission_mode` precedent
/// (research.md §5) — no shared constants module, no dedicated command.
/// Written by the frontend directly through the existing generic
/// `set_pref` command (`DelegateDenyRulesSetting.vue`), exactly like
/// `chat.permission_mode` has no Rust-side setter of its own either — only
/// this read side needs backend code.
pub const PREF_DENY_RULES: &str = "cli_delegate.deny_rules";

#[derive(Debug, thiserror::Error)]
pub enum DenyRulesError {
    #[error("stored deny rules are not valid JSON or contain an unknown category: {0}")]
    InvalidValue(#[from] serde_json::Error),
    #[error("preferences storage error: {0}")]
    Storage(#[from] haex_crdt::rusqlite::Error),
}

/// Pure parsing step for the persisted preference value — split out from
/// [`get_deny_rules`] so it is unit-testable without an open database
/// (matching `storage::preferences_tests`'s own no-I/O convention; DB
/// behavior for the underlying generic `preferences` table already has its
/// own integration coverage in `tests/preferences_roundtrip.rs`). An absent
/// preference, SQL `NULL`, or a valid empty JSON array all mean "no deny
/// rules configured" (fully permissive, spec's documented edge case).
/// Malformed JSON or an unknown category is an error (rejected by
/// `DenyCategory`'s own derived `Deserialize`, per its `snake_case` wire
/// values) — it must never be treated as an empty list, since that would
/// silently allow every call through.
fn parse_deny_rules(raw: Option<String>) -> Result<Vec<DenyCategory>, DenyRulesError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    Ok(serde_json::from_str(&raw)?)
}

/// Reads the persisted deny-rule set (see [`parse_deny_rules`] for the
/// value-shape rules).
pub fn get_deny_rules(
    conn: &haex_crdt::rusqlite::Connection,
    device_id: uuid::Uuid,
) -> Result<Vec<DenyCategory>, DenyRulesError> {
    let raw = preferences::get(conn, PrefScope::Device(device_id), PREF_DENY_RULES)?;
    parse_deny_rules(raw)
}

#[cfg(test)]
#[path = "autonomy_tests.rs"]
mod tests;
