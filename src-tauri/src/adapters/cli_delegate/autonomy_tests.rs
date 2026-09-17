use std::path::Path;

use serde_json::json;

use super::*;

const WORKSPACE: &str = "/tmp/holzi-invocation-abc123";

fn workspace_root() -> &'static Path {
    Path::new(WORKSPACE)
}

fn claude_call(tool_name: &str, input: serde_json::Value) -> ApprovalRequestPayload {
    ApprovalRequestPayload::ClaudeToolCall {
        tool_name: tool_name.to_string(),
        input,
    }
}

fn codex_command(command: &str, cwd: Option<&str>, network: Option<NetworkContext>) -> ApprovalRequestPayload {
    ApprovalRequestPayload::CodexCommandExecution {
        command: command.to_string(),
        cwd: cwd.map(|s| s.to_string()),
        network,
    }
}

#[test]
fn empty_rule_set_always_allows() {
    let request = claude_call("Bash", json!({"command": "curl https://example.com"}));
    assert_eq!(
        evaluate_deny_rules(&[], &request, workspace_root()),
        ApprovalDecision::Allow
    );
    assert_eq!(
        evaluate_deny_rules(&[], &ApprovalRequestPayload::CodexFileChange, workspace_root()),
        ApprovalDecision::Allow
    );
}

// --- WorkspaceEscape ---

#[test]
fn workspace_escape_claude_denies_path_outside_workspace() {
    let request = claude_call(
        "Write",
        json!({"file_path": "/etc/passwd", "content": "x"}),
    );
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::WorkspaceEscape], &request, workspace_root()),
        ApprovalDecision::Deny
    );
}

#[test]
fn workspace_escape_claude_allows_path_inside_workspace() {
    let request = claude_call(
        "Write",
        json!({"file_path": format!("{WORKSPACE}/notes.txt"), "content": "x"}),
    );
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::WorkspaceEscape], &request, workspace_root()),
        ApprovalDecision::Allow
    );
}

#[test]
fn workspace_escape_claude_allows_relative_path_that_stays_inside() {
    let request = claude_call("Write", json!({"file_path": "notes.txt"}));
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::WorkspaceEscape], &request, workspace_root()),
        ApprovalDecision::Allow
    );
}

#[test]
fn workspace_escape_claude_denies_relative_parent_traversal() {
    let request = claude_call("Write", json!({"file_path": "../../etc/passwd"}));
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::WorkspaceEscape], &request, workspace_root()),
        ApprovalDecision::Deny
    );
}

#[test]
fn workspace_escape_claude_allows_when_no_file_path_present() {
    // Not a file-editing tool call — the category has nothing to evaluate,
    // so it does not apply (data-model.md).
    let request = claude_call("Read", json!({"query": "hi"}));
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::WorkspaceEscape], &request, workspace_root()),
        ApprovalDecision::Allow
    );
}

#[test]
fn workspace_escape_codex_command_denies_cwd_outside_workspace() {
    let request = codex_command("ls", Some("/etc"), None);
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::WorkspaceEscape], &request, workspace_root()),
        ApprovalDecision::Deny
    );
}

#[test]
fn workspace_escape_codex_command_allows_cwd_inside_workspace() {
    let request = codex_command("ls", Some(WORKSPACE), None);
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::WorkspaceEscape], &request, workspace_root()),
        ApprovalDecision::Allow
    );
}

#[test]
fn workspace_escape_codex_command_allows_when_no_cwd_present() {
    // Not evaluable — mirrors Claude's "no file_path present" case: the
    // category has nothing to check, so it does not apply (data-model.md).
    // Distinct from `CodexFileChange`, which has no `cwd` field at all on
    // its real schema and fails closed for that reason (FR-015); this is
    // a defensive `Option` never expected to be absent in practice for a
    // `CommandExecutionRequestApprovalParams` payload.
    let request = codex_command("ls", None, None);
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::WorkspaceEscape], &request, workspace_root()),
        ApprovalDecision::Allow
    );
}

#[test]
fn workspace_escape_codex_file_change_fails_closed() {
    // FR-015: no path field exists on this payload at all.
    assert_eq!(
        evaluate_deny_rules(
            &[DenyCategory::WorkspaceEscape],
            &ApprovalRequestPayload::CodexFileChange,
            workspace_root()
        ),
        ApprovalDecision::Deny
    );
}

// --- NetworkAccess ---

#[test]
fn network_access_claude_denies_recognized_webfetch() {
    let request = claude_call("WebFetch", json!({"url": "https://example.com"}));
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::NetworkAccess], &request, workspace_root()),
        ApprovalDecision::Deny
    );
}

#[test]
fn network_access_claude_denies_unclassifiable_bash_command() {
    // A Bash call whose command text matches no recognized network
    // pattern still fails closed: Claude's approval payload has no
    // structured network field, so an unclassifiable command cannot be
    // positively ruled out (data-model.md). Distinct from T019's
    // evaluator-error mapping — this is the evaluator's own documented
    // behavior for a well-formed but ambiguous request.
    let request = claude_call("Bash", json!({"command": "ls -la /tmp"}));
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::NetworkAccess], &request, workspace_root()),
        ApprovalDecision::Deny
    );
}

#[test]
fn network_access_claude_allows_non_command_non_network_tool() {
    let request = claude_call("Read", json!({"file_path": format!("{WORKSPACE}/a.txt")}));
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::NetworkAccess], &request, workspace_root()),
        ApprovalDecision::Allow
    );
}

#[test]
fn network_access_codex_command_denies_structured_network_context() {
    let request = codex_command(
        "curl https://example.com",
        Some(WORKSPACE),
        Some(NetworkContext {
            host: Some("example.com".to_string()),
            protocol: Some("https".to_string()),
        }),
    );
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::NetworkAccess], &request, workspace_root()),
        ApprovalDecision::Deny
    );
}

#[test]
fn network_access_codex_command_allows_without_network_context() {
    let request = codex_command("ls", Some(WORKSPACE), None);
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::NetworkAccess], &request, workspace_root()),
        ApprovalDecision::Allow
    );
}

#[test]
fn network_access_codex_file_change_does_not_apply() {
    // Not a network-triggered approval kind — unlike WorkspaceEscape/
    // CredentialPaths, this category simply does not apply to a file
    // change at all (data-model.md), so it allows rather than fails closed.
    assert_eq!(
        evaluate_deny_rules(
            &[DenyCategory::NetworkAccess],
            &ApprovalRequestPayload::CodexFileChange,
            workspace_root()
        ),
        ApprovalDecision::Allow
    );
}

// --- CredentialPaths ---

#[test]
fn credential_paths_claude_denies_ssh_file_path() {
    let request = claude_call(
        "Read",
        json!({"file_path": "/home/user/.ssh/id_rsa"}),
    );
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::CredentialPaths], &request, workspace_root()),
        ApprovalDecision::Deny
    );
}

#[test]
fn credential_paths_claude_denies_command_touching_env_file() {
    let request = claude_call("Bash", json!({"command": "cat .env"}));
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::CredentialPaths], &request, workspace_root()),
        ApprovalDecision::Deny
    );
}

#[test]
fn credential_paths_claude_allows_unrelated_file() {
    let request = claude_call(
        "Write",
        json!({"file_path": format!("{WORKSPACE}/report.md")}),
    );
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::CredentialPaths], &request, workspace_root()),
        ApprovalDecision::Allow
    );
}

#[test]
fn credential_paths_codex_command_denies_aws_pattern() {
    let request = codex_command("cat ~/.aws/credentials", Some(WORKSPACE), None);
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::CredentialPaths], &request, workspace_root()),
        ApprovalDecision::Deny
    );
}

#[test]
fn credential_paths_codex_command_allows_unrelated_command() {
    let request = codex_command("ls", Some(WORKSPACE), None);
    assert_eq!(
        evaluate_deny_rules(&[DenyCategory::CredentialPaths], &request, workspace_root()),
        ApprovalDecision::Allow
    );
}

#[test]
fn credential_paths_codex_file_change_fails_closed() {
    assert_eq!(
        evaluate_deny_rules(
            &[DenyCategory::CredentialPaths],
            &ApprovalRequestPayload::CodexFileChange,
            workspace_root()
        ),
        ApprovalDecision::Deny
    );
}

#[test]
fn multiple_enabled_categories_deny_if_any_matches() {
    let request = claude_call("Read", json!({"file_path": "/home/user/.ssh/id_rsa"}));
    assert_eq!(
        evaluate_deny_rules(
            &[DenyCategory::WorkspaceEscape, DenyCategory::CredentialPaths],
            &request,
            workspace_root()
        ),
        ApprovalDecision::Deny
    );
}

// --- classify_autonomy_spawn_error ---

#[test]
fn classifies_claude_unsupported_permission_mode() {
    let raw = "error: option '--permission-mode <mode>' argument 'bypassPermissions' is invalid.";
    let result = classify_autonomy_spawn_error(DelegateVendor::Claude, AutonomyMode::Ungated, raw);
    assert_eq!(
        result,
        Some(AutonomyUnavailable {
            vendor: DelegateVendor::Claude,
            mode: AutonomyMode::Ungated,
        })
    );
}

#[test]
fn does_not_classify_unrelated_claude_error() {
    let raw = "claude exited without producing a result: network error";
    assert_eq!(
        classify_autonomy_spawn_error(DelegateVendor::Claude, AutonomyMode::Ungated, raw),
        None
    );
}

#[test]
fn classifies_codex_unsupported_approval_policy() {
    let raw = r#"codex thread/start failed: {"code":-32602,"message":"unknown field `approvalPolicy`"}"#;
    let result =
        classify_autonomy_spawn_error(DelegateVendor::Codex, AutonomyMode::GatedPermissive, raw);
    assert_eq!(
        result,
        Some(AutonomyUnavailable {
            vendor: DelegateVendor::Codex,
            mode: AutonomyMode::GatedPermissive,
        })
    );
}

#[test]
fn does_not_classify_unrelated_codex_error() {
    let raw = "codex app-server exited before responding to thread/start";
    assert_eq!(
        classify_autonomy_spawn_error(DelegateVendor::Codex, AutonomyMode::Ungated, raw),
        None
    );
}

// --- deny-rule preference value parsing (pure — DB behavior for the
// underlying generic `preferences` table has its own integration coverage
// in `tests/preferences_roundtrip.rs`; `get_deny_rules` is a thin wrapper
// over that already-tested storage, following the `chat.permission_mode`
// precedent — no Rust-side setter, the frontend writes this preference
// directly through the existing generic `set_pref` command) ---

#[test]
fn deny_rules_absent_preference_means_no_rules() {
    assert_eq!(parse_deny_rules(None).unwrap(), Vec::new());
}

#[test]
fn deny_rules_empty_array_means_no_rules() {
    assert_eq!(
        parse_deny_rules(Some("[]".to_string())).unwrap(),
        Vec::new()
    );
}

#[test]
fn deny_rules_parses_known_categories() {
    assert_eq!(
        parse_deny_rules(Some(r#"["workspace_escape","network_access"]"#.to_string())).unwrap(),
        vec![DenyCategory::WorkspaceEscape, DenyCategory::NetworkAccess]
    );
}

#[test]
fn deny_rules_malformed_json_is_an_error() {
    assert!(parse_deny_rules(Some("not json".to_string())).is_err());
}

#[test]
fn deny_rules_unknown_category_is_an_error() {
    assert!(parse_deny_rules(Some(r#"["not_a_real_category"]"#.to_string())).is_err());
}
