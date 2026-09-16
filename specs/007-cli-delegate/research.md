# Phase 0 Research: CLI Delegate Backend (Claude Code / Codex)

Most of this feature's technical unknowns were resolved by live verification against installed CLIs
(`claude` v2.1.241, `codex-cli` 0.147.0) during this planning session, not from documentation alone —
documentation turned out to be wrong on the single most load-bearing point (§1 below).

## 1. Claude Code (`claude -p`) — live per-tool-call approval

**Decision**: Drive `claude -p` directly (no Agent SDK, no `claude-code-acp`), passing
`--permission-prompt-tool mcp__<server>__<tool>` pointed at a small stdio MCP server holzi spawns
per invocation and registers via `--mcp-config`. That server's one tool bridges each pending approval
into holzi's existing `ChatState.pending_tool_approvals` / `tool-permission-request` mechanism
(`turn.rs`, unchanged) instead of a Claude-Code-specific approval path.

**Rationale**: The design doc's original assumption (`docs/plans/2026-09-11-agent-tool-loop-design.md`
§8.2, written 2026-09-11) was that `-p` "has no live mid-run round-trip" and can only get a pre-call
tool whitelist — which drove this feature's spec toward a bespoke "upfront batch approval" design for
Claude Code specifically. That assumption was **empirically wrong**, discovered by testing directly:

- A minimal hand-rolled stdio MCP server (newline-delimited JSON-RPC: `initialize`, `tools/list`
  exposing one tool named `approve`, `tools/call` handling it) was registered via a `--mcp-config`
  JSON file.
- Running `claude -p "Run exactly this shell command using the Bash tool: touch
  verify_delegate_test3.txt" --mcp-config mcp-config.json --permission-prompt-tool
  mcp__approvalserver__approve --permission-mode default` produced a `tools/call` request to the test
  server: `{"name":"approve","arguments":{"tool_name":"Bash","input":{"command":"touch
  verify_delegate_test3.txt","description":"..."},"tool_use_id":"toolu_..."}}`.
- The test server's handler slept 4 seconds before responding
  `{"behavior":"allow","updatedInput":{...}}` (wrapped in an MCP text content block). The overall
  `claude -p` run's wall time reflected that full 4-second wait, and `verify_delegate_test3.txt` was
  only created after the response was sent — a genuine blocking, live, per-tool-call round-trip, not
  a fire-and-forget notification or a static pre-declared policy.
- A first attempt at this same test (without `--permission-mode default` explicitly set) showed the
  Bash call sail through with **no** call to the test server at all. Root cause: the operator's own
  `~/.claude/settings.json` sets `"permissions": {"defaultMode": "auto"}` globally, so any `-p`
  invocation without an explicit `--permission-mode` silently inherited that personal setting's
  model-classifier auto-approval instead of running in Manual/`default` mode — a concrete, observed
  instance of exactly the host-config leakage User Story 4 exists to prevent, not a flaw in the
  approval mechanism itself.

**Response schema used** (not fully documented anywhere found — reverse-engineered from the Agent
SDK's `canUseTool` return-type documentation, since `--permission-prompt-tool`'s own wire format
isn't separately documented): a `tools/call` result whose content is a text block containing
`{"behavior":"allow","updatedInput":<original or modified input>}` or
`{"behavior":"deny","message":"<reason>"}`. This worked end-to-end in the live test above. Treat the
exact field names as verified-working, not as officially guaranteed to be stable across Claude Code
versions — worth a smoke test in CI against whatever `claude` version ships.

**Alternatives considered**: The design doc's originally-rejected ACP/Agent-SDK path (§8.1, still
rejected — no new information changes that). The upfront-batch-approval design this feature's spec
itself proposed before this finding — rejected now that live approval is proven to work, since it
would have meant Claude Code's tool-use experience diverges from every other backend's live,
per-action approval for no remaining technical reason.

## 2. Codex (`codex app-server --stdio`) — approval protocol shape

**Decision**: Speak `codex app-server --stdio`'s JSON-RPC protocol directly (no `codex-acp`), holding
one process open per delegate invocation. Route its server-initiated approval requests
(`ExecCommandApprovalRequest`, `ApplyPatchApprovalRequest`, a `PermissionsRequestApprovalRequest`-
shaped request) through the same `pending_tool_approvals` bridge as the Claude Code path.

**Rationale**: `codex app-server generate-json-schema --out <dir> --experimental` (a built-in
subcommand of the installed CLI) dumps the actual protocol schema without needing external docs or a
live conversation. It confirms:

- `ServerRequest` (the union of requests the app-server sends *to* the client) includes
  `ExecCommandApprovalRequest`, `ApplyPatchApprovalRequest`, `Item/commandExecution/
  requestApprovalRequest`, `Item/fileChange/requestApprovalRequest`, and `Item/permissions/
  requestApprovalRequest` — i.e., approval requests are genuine server→client JSON-RPC requests
  (carrying `itemId`, `threadId`, `turnId`, `startedAtMs`), not notifications.
- The paired response type (`ExecCommandApprovalResponse`) carries a `ReviewDecision` enum:
  `approved`, `approved_for_session`, an execpolicy-amendment variant, a network-policy-amendment
  variant, `denied` (with a `rejection` reason, "agent will continue the turn"), `abort` ("turn will
  also be immediately interrupted"), and — notably — `timed_out` ("automatic approval review timed
  out before reaching a decision"). The existence of a timeout outcome is strong structural evidence
  of a real wait, not a static or fire-and-forget check.

This structurally confirms design doc §10 item 1's leading hypothesis. **Not yet exercised**: an
actual live round-trip the way §1's Claude Code test was (would need a real Codex-authenticated
session driving a task that triggers `ExecCommandApprovalRequest`) — a cheap, worthwhile
pre-implementation spike (first integration test written for `codex.rs`), not a blocking unknown for
planning purposes.

**Alternatives considered**: `codex exec` (headless mode) — rejected per the design doc's existing
finding ([openai/codex#24135](https://github.com/openai/codex/issues/24135)): closed stdin means any
approval auto-rejects, incompatible with live approval entirely. `codex-acp` — rejected as unnecessary
indirection now that the raw protocol's shape is directly confirmed from the installed CLI itself.

## 3. Host isolation and credential mechanism (Claude Code)

**Decision**: Use `CLAUDE_CONFIG_DIR=<fresh empty temp dir>` + a disposable `cwd` + `CLAUDE_CODE_
OAUTH_TOKEN` (env var, from `claude setup-token`) — **not** `--bare`.

**Rationale**: Current docs (`code.claude.com/docs/en/headless`, fetched 2026-09-16) recommend
`--bare` as "the recommended mode for scripted and SDK calls" for exactly this kind of host
isolation, but state explicitly: "bare mode doesn't use your subscription login... In bare mode,
Claude Code never reads OAuth credentials or the system keychain. For the Anthropic API, set
`ANTHROPIC_API_KEY`." Since this feature's entire premise (vs. the existing `api_key` provider) is
reusing an existing *subscription*, `--bare` is the wrong tool here regardless of its isolation
strength. The non-`--bare` mechanism was verified directly instead:

- Running `claude -p ... ` with `CLAUDE_CONFIG_DIR` pointed at a fresh empty directory produced
  `"apiKeySource":"none"` and `"Not logged in · Please run /login"` in the response, despite the host
  machine having a valid, working `claude` login — confirming credential redirection.
- The same isolated run's event stream showed no `hook_started`/`hook_response` system events, while
  an otherwise-identical non-isolated run showed two (the operator's own `~/.claude/settings.json`
  `SessionStart` hook firing before `system/init`). This confirms `CLAUDE_CONFIG_DIR` also blocks
  host-level *hook* execution, not just credential lookup.

**Not yet verified**: whether this mechanism blocks every kind of host-level discovery `--bare`
guarantees (skills, plugins, MCP auto-discovery specifically), or only what was directly tested
(credentials, hooks, and — by construction, since `cwd` is a fresh empty directory — project-local
`.claude/settings.json`/`.mcp.json`/`CLAUDE.md` discovery). Worth a narrow pre-implementation check
covering skills/plugins specifically; not expected to change the chosen mechanism.

**Alternatives considered**: `--bare` + `ANTHROPIC_API_KEY` — rejected, collapses this feature into
the existing `api_key` provider and abandons the "bring your subscription" value proposition (spec.md
User Story 1). Relying on `~/.codex`/`~/.claude` host state directly (`plans/001-desktop-mvp.md`'s
original, since-revised assumption) — already rejected in the 2026-09-11 design doc for breaking
vault portability; nothing here reopens that.

## 4. `ProviderAdapter` integration point — no `turn.rs` changes needed

**Decision**: `CliDelegateAdapter` (both Claude and Codex variants) implements the existing
`ProviderAdapter` trait exactly like `AnthropicAdapter`/`LocalAdapter`. It runs its subprocess to
completion internally (including its own tool use, bridged through the approval mechanism above) and
streams the final answer back as ordinary `StreamChunk::Delta` fragments followed by `StreamChunk::
Done` — never emitting `StreamChunk::ToolCalls`.

**Rationale**: `adapters/types.rs`'s existing `ChatRequest.tools` documentation already establishes
that an adapter "never errors on an empty list, it just never emits `StreamChunk::ToolCalls`" for
models that don't support tool calling (`LocalAdapter`'s existing behavior for non-tool-calling local
models). `turn.rs`'s `run_turn`/`run_step` only route through holzi's own `ToolRegistry`/permission-
per-round machinery when a step's result actually carries `tool_calls` — the same escape hatch that
already exists for plain-text local models applies unchanged to a delegate backend that does all of
its own tool use internally. Confirmed by reading `turn.rs` end-to-end (no `match ProviderKind` or
delegate-specific special-casing exists there today, nor is any needed). This is why this feature's
plan.md Summary can truthfully say `turn.rs` needs zero changes.

**Alternatives considered**: Exposing the delegate's internal tool_use/tool_result activity as
holzi's own `StreamChunk::ToolCalls` round-trips (i.e., making `turn.rs` drive the delegate's tools
one-by-one like it drives the built-in registry). Rejected — the whole point of a CLI delegate is
that it runs its own tool loop with its own tools (Claude Code's Bash/Read/Edit/etc., Codex's
equivalents); re-routing those through holzi's `ToolRegistry` would mean re-implementing each
delegate's own built-in tools a second time for no benefit. Tool activity the delegate performs is
instead surfaced as a labeled record (FR-005) via `tool_source` (data-model.md), not replayed through
the built-in tool-call machinery.

## Summary of resolved Technical Context unknowns

| Unknown | Resolution |
|---|---|
| Claude Code live approval capability | §1 — `--permission-prompt-tool` + holzi-run MCP server, verified live |
| Codex approval protocol shape | §2 — `ServerRequest` approval variants, schema-confirmed from installed CLI |
| Host isolation + subscription credential mechanism | §3 — `CLAUDE_CONFIG_DIR` + disposable `cwd`, not `--bare`; verified for credentials + hooks |
| Turn/step loop integration point | §4 — `ProviderAdapter::stream_chat`, no `turn.rs` changes |

No unresolved `NEEDS CLARIFICATION` markers remain in `plan.md`'s Technical Context. Two narrow,
explicitly-scoped pre-implementation spikes remain (Codex live round-trip, §2; Claude Code
skills/plugins isolation coverage, §3) — neither blocks planning, both are cheap first steps in
`tasks.md`.
