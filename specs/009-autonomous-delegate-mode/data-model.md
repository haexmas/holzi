# Phase 1 Data Model: Autonomous Delegate Mode

## Changed entities (existing schema, additive migration 0017)

### `chat_messages` (new `autonomy_mode` column)

Migration `0017_chat_messages_add_autonomy_mode` adds a nullable `autonomy_mode TEXT` column;
`tool_source` remains free-text `TEXT NULL` (`chat/tools/mod.rs:69`'s own doc comment: extensible
by design). 007-cli-delegate already added `cli_delegate:claude` / `cli_delegate:codex` for tool
activity a delegate reports as its own. This feature adds one more piece of information those rows
need to carry: which `AutonomyMode` the turn ran under, so spec
FR-005's "make clear which autonomy mode a given turn ran under" is satisfiable from stored data,
not re-derived. Two viable shapes, both additive to the existing column set:

| Shape                       | Description                                                                                                                                                          |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A — encode in `tool_source` | `cli_delegate:claude:gated_permissive` / `cli_delegate:codex:gated_permissive` (mode suffix appended). No column change; string parsing gets one more segment.       |
| B — new nullable column     | `autonomy_mode TEXT NULL` alongside `tool_source`, values `"standard"` / `"ungated"` / `"gated_permissive"`, `NULL` for non-delegate rows (built-in tool loop, MCP). |

**Decision**: B. Rationale: `tool_source` already conflates two independent facts today
(where a call came from: `mcp`/`cli`/`cli_delegate:<vendor>`); adding a third dimension (which
autonomy posture governed it) by further string-encoding would make `tool_source` a
three-part composite parsed ad hoc at every call site, whereas the codebase's own established
pattern for genuinely new, independent facts is a new nullable column (mirrors how `tool_call_id`,
`tool_input`, `tool_is_error` were each added as their own column rather than folded into an
existing one). A new column also lets `ungated` turns be queried/filtered independently of
`gated_permissive` ones without string parsing. **Migration note**: this is the one schema change
in an otherwise additive feature; it is additive (new nullable column, `ON DELETE`/existing
row behavior unaffected) and does not touch 007-cli-delegate's existing columns.

Note also (spec FR-006): for `ungated` runs, there is deliberately **no per-tool-call row at all** —
only the delegate's final response is persisted as the ordinary assistant message. The new
`autonomy_mode` column is set on that assistant message row itself (so "which mode did this turn use"
is answerable even when no tool-call rows exist for it), not only on tool-call rows.

The nullable value is carried through `ChatMessage`, the history SQL projection, `row_to_message`,
and the `MessagePayload` DTO to the frontend `Message` type. The existing history endpoint and its
overall shape remain unchanged; legacy and non-delegate rows expose `autonomy_mode: null` internally
and `autonomyMode: null` on the camelCase frontend payload.

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
An absent key, SQL `NULL`, or a valid empty array means "no deny rules configured" — spec's own
documented edge case, fully permissive. Malformed JSON or an array containing an unknown category
is an error; it must not be converted to an empty list because that would silently allow every call.

### `evaluate_deny_rules` — the per-vendor matching function

```rust
pub fn evaluate_deny_rules(
    enabled: &[DenyCategory],
    request: &ApprovalRequestPayload,
    workspace_root: &Path,
) -> ApprovalDecision
```

Where `ApprovalRequestPayload` is a small enum capturing exactly what each vendor's approval
callback actually provides (not a lowest-common-denominator struct that would hide the asymmetry).
The payload variants already identify the vendor, so there is no separate `DelegateVendor`
parameter. `workspace_root` is selected for this invocation and threaded explicitly through the
approval flow; the evaluator never derives it from the ambient process working directory, and it is
not added to `ApprovalRequestPayload`.

```rust
pub enum ApprovalRequestPayload {
    ClaudeToolCall { tool_name: String, input: Value },       // has file_path when input is a file-editing tool's input
    CodexCommandExecution { command: String, cwd: Option<String>, network: Option<NetworkContext> },
    CodexFileChange,                                          // deliberately no fields — schema exposes none (research.md §3)
}
```

Matching logic per category (research.md §3/§4 for the underlying evidence):

| Category          | `ClaudeToolCall`                                                                                                                                                                | `CodexCommandExecution`                               | `CodexFileChange`                                                              |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------ |
| `WorkspaceEscape` | Compare a `file_path`-bearing input field against the explicit invocation `workspace_root`                                                                                      | Compare `cwd`/`command` text against `workspace_root` | **Cannot evaluate** — no path field exists (spec FR-015 applies: deny)         |
| `NetworkAccess`   | Deny recognized `WebFetch`/`WebSearch` tools and known network-command patterns (`curl`, `wget`, …); when a command's network intent cannot be classified, fail closed and deny | `network.is_some()` — structured, reliable            | Not a network-triggered approval kind; category does not apply to this variant |
| `CredentialPaths` | Compare `file_path`/`command` text against a fixed glob list (`.ssh/`, `.aws/`, `.env`, `id_rsa`, …)                                                                            | Compare `command` text against the same glob list     | **Cannot evaluate** — no path field exists (spec FR-015 applies: deny)         |

`evaluate_deny_rules` returns `ApprovalDecision::Deny` if any enabled category matches (including
the FR-015 unevaluable-but-enabled case above), `ApprovalDecision::Allow` otherwise. This function
has no `Ask` outcome — `gated-permissive` never produces a human-facing pause (spec FR-004), so it
returns the same two-variant `ApprovalDecision` type `approval_bridge.rs` already defines, simply
never constructing the case that would trigger a UI prompt.

`NetworkAccess` enforcement is intentionally limited to approval callback payloads that reach
`approval_bridge::request_approval`: the structured Codex network signal, recognized Claude
`WebFetch`/`WebSearch` tools, and the documented command patterns. An unclassifiable Claude command
payload fails closed when `NetworkAccess` is enabled. FR-007 and SC-003 apply to actions presented
through that callback boundary and matching those documented signals; this phase does not expand
evaluator routing or claim detection and blocking of every possible network-capable command outside
that boundary.

## Changed entity: `preferences` (existing table, one new key, no schema change)

| Key                       | Scope               | Value shape                                                                              | Written by                                             | Read by                                                                                             |
| ------------------------- | ------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------ | --------------------------------------------------------------------------------------------------- |
| `cli_delegate.deny_rules` | `PrefScope::Device` | JSON array of `DenyCategory` string values, e.g. `["workspace_escape","network_access"]` | `DelegateDenyRulesSetting.vue` via existing `set_pref` | `approval_bridge::request_approval`, only when the invocation's `AutonomyMode` is `GatedPermissive` |

Follows exactly the `chat.permission_mode` precedent (`PrefScope::Device`, own locally-declared
`const PREF_DENY_RULES: &str = "cli_delegate.deny_rules";` in `autonomy.rs`, no shared constants
module — matching the codebase's existing convention of not centralizing preference keys, per
research.md §5).

The backend helpers use fallible APIs:

```rust
get_deny_rules(conn, device_id) -> Result<Vec<DenyCategory>>
set_deny_rules(conn, device_id, categories: &[DenyCategory]) -> Result<()>
```

An absent preference or a valid empty JSON array returns an empty list. Malformed JSON returns an
error; it must not silently disable the operator's deny rules. The gated-permissive approval flow
maps that parsing error to `ApprovalDecision::Deny` under FR-010.

## State/lifecycle notes

- The `AutonomyMode` selection has no persisted lifecycle — it exists only for one `ChatRequest` and
  its `AdapterStream` (spec FR-008: never carried into a later request). The mode actually used is
  still recorded on that request's message rows for history.
- `DenyCategory` set has no versioning/migration concern beyond an absent preference
  (`preferences::get` returning `None`, per `preferences.rs:89-99`), which is treated identically to
  a valid empty array. A present malformed value is an error and fails closed in the approval flow.
- No new state machine, no new entity relationships beyond the ones `chat_messages`/`preferences`
  already have with `known_devices`/`chat_threads`.
