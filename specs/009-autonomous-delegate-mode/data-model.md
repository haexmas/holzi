# Phase 1 Data Model: Autonomous Delegate Mode

## Changed entities (existing schema, no new migration)

### `chat_messages` (existing, `tool_source` column)

No migration — `tool_source` is already free-text `TEXT NULL` (`chat/tools/mod.rs:69`'s own doc
comment: extensible by design). 007-cli-delegate already added `cli_delegate:claude` /
`cli_delegate:codex` for tool activity a delegate reports as its own. This feature adds one more
piece of information those rows need to carry: which `AutonomyMode` the turn ran under, so spec
FR-005's "make clear which autonomy mode a given turn ran under" is satisfiable from stored data,
not re-derived. Two viable shapes, both additive to the existing column set:

| Shape | Description |
|---|---|
| A — encode in `tool_source` | `cli_delegate:claude:gated_permissive` / `cli_delegate:codex:gated_permissive` (mode suffix appended). No column change; string parsing gets one more segment. |
| B — new nullable column | `autonomy_mode TEXT NULL` alongside `tool_source`, values `"standard"` / `"ungated"` / `"gated_permissive"`, `NULL` for non-delegate rows (built-in tool loop, MCP). |

**Decision**: B. Rationale: `tool_source` already conflates two independent facts today
(where a call came from: `mcp`/`cli`/`cli_delegate:<vendor>`); adding a third dimension (which
autonomy posture governed it) by further string-encoding would make `tool_source` a
three-part composite parsed ad hoc at every call site, whereas the codebase's own established
pattern for genuinely new, independent facts is a new nullable column (mirrors how `tool_call_id`,
`tool_input`, `tool_is_error` were each added as their own column rather than folded into an
existing one). A new column also lets `ungated` turns be queried/filtered independently of
`gated_permissive` ones without string parsing. **Migration note**: this is the one schema change
in an otherwise migration-free feature; it is additive (new nullable column, `ON DELETE`/existing
row behavior unaffected) and does not touch 007-cli-delegate's existing columns.

Note also (spec FR-006): for `ungated` runs, there is deliberately **no per-tool-call row at all** —
only the delegate's final response is persisted as the ordinary assistant message. The new
`autonomy_mode` column is set on that assistant message row itself (so "which mode did this turn use"
is answerable even when no tool-call rows exist for it), not only on tool-call rows.

## New Rust types (runtime, `adapters/cli_delegate/autonomy.rs`)

### `AutonomyMode`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyMode {
    #[default]
    Standard,
    Ungated,
    GatedPermissive,
}
```

Carried on `ChatRequest` (`adapters/mod.rs`, new field `autonomy_mode: AutonomyMode`, default
`Standard`). Serialized to/from the frontend the same way other `ChatRequest` fields already are
(existing Tauri command serialization, no new binding pipeline). Non-delegate adapters (`local`,
`api_key`) receive this field on `req` like every other field but never read it — enforced by
review, not by the type system, matching how `req.system_prompt` is already optional-and-ignored by
adapters that don't use it.

### `DenyCategory`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenyCategory {
    WorkspaceEscape,
    NetworkAccess,
    CredentialPaths,
}
```

Persisted as a JSON array of these (serde string values) inside the `cli_delegate.deny_rules`
preference value (a JSON-encoded string, per `preferences.rs`'s `value: String` column shape — the
existing pattern for any structured preference, there being no separate structured-value column).
Empty array (or the preference key entirely absent) means "no deny rules configured" — spec's own
documented edge case, fully permissive, not an error.

### `evaluate_deny_rules` — the per-vendor matching function

```rust
pub fn evaluate_deny_rules(
    enabled: &[DenyCategory],
    vendor: DelegateVendor,
    request: &ApprovalRequestPayload,
) -> ApprovalDecision
```

Where `ApprovalRequestPayload` is a small enum capturing exactly what each vendor's approval
callback actually provides (not a lowest-common-denominator struct that would hide the asymmetry):

```rust
pub enum ApprovalRequestPayload {
    ClaudeToolCall { tool_name: String, input: Value },       // has file_path when input is a file-editing tool's input
    CodexCommandExecution { command: String, cwd: Option<String>, network: Option<NetworkContext> },
    CodexFileChange,                                          // deliberately no fields — schema exposes none (research.md §3)
}
```

Matching logic per category (research.md §3/§4 for the underlying evidence):

| Category | `ClaudeToolCall` | `CodexCommandExecution` | `CodexFileChange` |
|---|---|---|---|
| `WorkspaceEscape` | Compare a `file_path`-bearing input field against the invocation's workspace root | Compare `cwd`/`command` text against the workspace root | **Cannot evaluate** — no path field exists (spec FR-015 applies: deny) |
| `NetworkAccess` | Heuristic: tool name is `WebFetch`/`WebSearch`, or `command` text matches a known network-tool pattern (`curl`, `wget`, …) | `network.is_some()` — structured, reliable | Not a network-triggered approval kind; category does not apply to this variant |
| `CredentialPaths` | Compare `file_path`/`command` text against a fixed glob list (`.ssh/`, `.aws/`, `.env`, `id_rsa`, …) | Compare `command` text against the same glob list | **Cannot evaluate** — no path field exists (spec FR-015 applies: deny) |

`evaluate_deny_rules` returns `ApprovalDecision::Deny` if any enabled category matches (including
the FR-015 unevaluable-but-enabled case above), `ApprovalDecision::Allow` otherwise. This function
has no `Ask` outcome — `gated-permissive` never produces a human-facing pause (spec FR-004), so it
returns the same two-variant `ApprovalDecision` type `approval_bridge.rs` already defines, simply
never constructing the case that would trigger a UI prompt.

## Changed entity: `preferences` (existing table, one new key, no schema change)

| Key | Scope | Value shape | Written by | Read by |
|---|---|---|---|---|
| `cli_delegate.deny_rules` | `PrefScope::Device` | JSON array of `DenyCategory` string values, e.g. `["workspace_escape","network_access"]` | `DelegateDenyRulesSetting.vue` via existing `set_pref` | `approval_bridge::request_approval`, only when the invocation's `AutonomyMode` is `GatedPermissive` |

Follows exactly the `chat.permission_mode` precedent (`PrefScope::Device`, own locally-declared
`const PREF_DENY_RULES: &str = "cli_delegate.deny_rules";` in `autonomy.rs`, no shared constants
module — matching the codebase's existing convention of not centralizing preference keys, per
research.md §5).

## State/lifecycle notes

- `AutonomyMode` has no persisted lifecycle — it exists only for the duration of one `ChatRequest`
  and the `AdapterStream` it produces (spec FR-008: never carried into a later request).
- `DenyCategory` set has no versioning/migration concern beyond the standard preference
  read-fallback (`preferences::get` returning `None` when absent, per `preferences.rs:89-99`) —
  treated identically to "empty array configured."
- No new state machine, no new entity relationships beyond the ones `chat_messages`/`preferences`
  already have with `known_devices`/`chat_threads`.
