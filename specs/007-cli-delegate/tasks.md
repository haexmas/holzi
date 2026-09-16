---
description: 'Actionable, dependency-ordered task list for the CLI delegate backend feature'
---

# Tasks: CLI Delegate Backend (Claude Code / Codex)

**Input**: Design documents from `/specs/007-cli-delegate/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-commands.md

**Tests**: Backend tests are REQUIRED (repo convention: every module gets its own `*_tests.rs`, never
inline `#[cfg(test)] mod tests`, plus integration tests under `src-tauri/tests/`). Frontend
verification is manual per quickstart.md (kein Playwright in diesem Repo).

**Revision notes**:

- **2026-09-16, post `/speckit.analyze`**: fixed two issues the analysis pass found in the first
  draft: (1) the Claude Code command built in Phase 3 (US1) referenced
  `--mcp-config`/`--permission-prompt-tool` before the server implementing that tool existed (US3) —
  Phase 3 now builds a safe, fail-closed default instead and US3 upgrades it; (2) "MVP First" wrongly
  scoped the MVP to US1+US2 alone, contradicting spec.md's own priority rationale that US3 (the
  approval gate) ships alongside US1 for safety — the MVP now explicitly includes US3. Four coverage
  gaps were also added as new tasks (T018, T019, T023, T032 below).
- **2026-09-16, during implementation start**: discovered MCP's stdio transport means `claude` spawns
  the `--mcp-config` server as _its own child process_ — not something reachable in-process the way
  the first draft of `permission_mcp_server.rs` assumed. Per operator direction, no HTTP-server-based
  fix (a pure-Rust IPC mechanism is preferred, independent of the fact that `cli_delegate` is already
  desktop-only regardless of this choice, since subprocess spawning itself is impossible on mobile).
  Fixed by adding one new task (T034, a hidden internal CLI entrypoint) and rewriting T035 (was T034)
  to describe the actual child-process-plus-local-socket-relay design — see data-model.md's
  "claude.rs"/"approval_bridge.rs" sections for the corrected architecture. Everything from the old
  T034 onward shifted by one (old T034→T035, old T035→T036, ..., old T045→T046).
- **2026-09-16, mid-implementation reconciliation**: a large chunk of implementation (T033-T036,
  T039-T040, T042) landed in a part of this session that got context-compacted before the phase
  checkpoints below were updated to match. Reconciled by reading the actual code and running the
  full test suite rather than trusting stale checkboxes. Notable divergences from the original plan,
  kept because they're better, not reverted to match the plan:
  - `list_models()` (T016) returns one real `ProviderModel` per connected vendor (flowing through the
    existing `do_refresh`/`replace_provider_models` cache, composite id `<providerId>:claude` or
    `<providerId>:codex`), not an empty `Vec` as data-model.md originally specified. The frontend
    (T017/T018) was corrected to match — no separate `:delegate`-suffixed synthetic id.
  - Cancellation (T041/T042) needed no delegate-specific tracking on `ChatState`/`session.rs` at all:
    `AdapterStream` gained an optional `CancellationToken` (`adapters/types.rs`,
    `AdapterStream::new_with_cancellation`) that the existing generic `abort_current_generation` /
    `ChatState.current_generation` mechanism from 003 already drives unchanged. T042 is done through
    this, not new session-level state.
  - The Phase 3 "safe fail-closed stub, no approval bridge yet" design (T013's original scope) was
    superseded before it shipped separately — `claude.rs`/`codex.rs` went straight to the real
    approval bridge (T033-T036) in the same pass. Functionally strictly better (nothing ever shipped
    in the degraded stub state), but it means the Phase 3/4 checkpoints below, which promised a
    blunt-denial intermediate state, describe something that was skipped rather than something a
    reviewer can currently observe — read them as historical intent, not current behavior.
  - `codex.rs` was initially missing `req.system_prompt` entirely (no Codex equivalent of
    `--append-system-prompt`) — found and fixed in this reconciliation pass (T039's Codex half),
    using `ThreadStartParams.developerInstructions` (research.md §2 schema).
  - Genuinely still open despite the above: T021-T032 (US2's whole connect flow; US3's *dedicated*
    approval integration tests — `approval_bridge_tests.rs`'s one unit test covers the shared
    live-round-trip mechanism both vendors call into, but not literally "a stub `claude`
    binary"/"a stub `codex app-server`" end-to-end, nor the `decide()`-skips-the-live-round-trip and
    posture-blocks-without-a-prompt cases specifically), T037/T038 (US4's dedicated isolation/cleanup
    integration tests — the mechanisms are implemented and were verified manually during planning,
    not by an automated test), T041 (dedicated cancellation integration test), T043-T046 (polish).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1..US5)

## Path Conventions

- Backend: `src-tauri/src/` and `src-tauri/tests/` (Rust)
- Frontend: `src/` (Nuxt 4 SPA), i18n at `src/i18n/locales/{de,en}.json`
- Docs: `specs/007-cli-delegate/` (this feature's design docs)

---

## Phase 1: Setup (Shared Infrastructure)

- [x] T001 [P] In `src-tauri/Cargo.toml`: move `rmcp`'s `server` feature from the test-only
      `[dev-dependencies]` entry to the real `[dependencies]` entry, add the `transport-io` feature
      alongside it (needed for a stdio MCP server running on a process's own inherited stdin/stdout,
      not just the existing in-memory test transport), still pinned `=3.3.0` (research.md §1). Also
      add the `net` feature to the main `[dependencies]` `tokio` entry (Unix domain sockets / Windows
      named pipes for the approval-bridge IPC hop, data-model.md — no new crate needed).
- [x] T002 [P] Create `src-tauri/src/adapters/cli_delegate/mod.rs` with `DelegateVendor::{Claude,
Codex}` (`parse`/`as_str`, mirroring `ProviderKind` in `storage/providers.rs`) and an empty
      `CliDelegateAdapter` struct; register `pub mod cli_delegate;` in `src-tauri/src/adapters/mod.rs`
      and update its module doc comment (lines 9-10, currently: "`cli_delegate` adapters require a
      separate design pass and are not yet represented here").

**Checkpoint**: Module skeleton and dependency changes in place. No behavior changes yet.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Schema/validation changes, adapter-construction wiring, and two verification spikes
every user story builds on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T003 Update `add_provider`'s validation in `src-tauri/src/providers/mod.rs` (~lines 112-123):
      replace the current rejection of `adapter` for `ProviderKind::CliDelegate` with a check that
      `adapter ∈ {"claude", "codex"}` (data-model.md).
- [x] T004 [P] Update the doc comment on `credentials` in `src-tauri/src/storage/providers.rs`
      (lines 13-17): remove "For local and cli_delegate providers it is `None`" for the
      `cli_delegate` half — it now holds the encrypted Claude OAuth token / Codex `auth.json` bytes.
- [x] T005 Thread `pending_tool_approvals: Arc<Mutex<HashMap<Uuid, oneshot::Sender<ApprovalDecision>>>>`
      (already on `ChatState`, `session.rs:120`) and `app: &AppHandle` (already available in
      `load_model_inner`, `model_loading.rs:132`) through `load_api_key_model` into
      `build_adapter(provider: &Provider, ...)` in `src-tauri/src/providers/mod.rs` and
      `src-tauri/src/chat/model_loading.rs` (data-model.md "Signatur-Änderung").
- [x] T006 Implement `build_adapter`'s `ProviderKind::CliDelegate` branch (`providers/mod.rs`,
      currently `Err(HolziError::InvalidInput { reason: "cli_delegate refresh is not yet
implemented" })`): parse `DelegateVendor` from `provider.adapter`, decrypt `provider.credentials`,
      construct `CliDelegateAdapter` with the handles from T005.
- [x] T007 [P] Create `src-tauri/src/adapters/cli_delegate/mod_tests.rs`: `DelegateVendor::parse`/
      `as_str` round-trip for both vendors and reject unknown strings; `build_adapter` constructs a
      `CliDelegateAdapter` for a well-formed `cli_delegate` row and still errors for a malformed
      `adapter` value. Register `#[cfg(test)] #[path = "mod_tests.rs"] mod mod_tests;` appropriately.
- [x] T008 [P] **Spike** (research.md §2): against a real authenticated `codex` installation, drive
      `codex app-server --stdio` through a task that triggers a command execution and confirm an
      `ExecCommandApprovalRequest` is delivered live and the process genuinely blocks until answered
      — the same way the Claude Code round-trip was already confirmed manually. Also determine
      Codex's safe fail-closed default for an unhandled `ServerRequest` approval (used by T014 before
      T036 wires real approval — e.g. does an unanswered request time out with `ReviewDecision::
timed_out` on its own, or must the client actively respond `denied`?). Record both outcomes as
      an addendum in research.md §2. **Gates**: T014, T030, T036 (all Codex-specific work) — do not
      start those until this spike's outcome is recorded.
- [x] T009 [P] **Spike** (research.md §3): with an isolated `CLAUDE_CONFIG_DIR`, confirm a host-level
      skill or plugin (not just hooks/credentials/settings, already confirmed) is also not discovered
      by a `-p` run. Record the outcome as an addendum in research.md §3. **Gates**: T037.

**Checkpoint**: Schema, adapter-construction wiring, and both verification spikes complete. User
story implementation can begin.

---

## Phase 3: User Story 1 - Use an existing coding-assistant subscription as the chat backend (Priority: P1) 🎯 MVP (part 1 of 3, ships with US2 + US3)

**Goal**: `CliDelegateAdapter` drives `claude -p`/`codex app-server` end-to-end and streams a real
answer back through the existing, unchanged turn/step loop; a delegate backend is selectable from the
UI, shown as "not connected" until Phase 4 (US2) provides a way to connect one. **Sensitive actions
are cleanly denied by default in this phase** (no live approval bridge yet, see below) — Phase 5
(US3) replaces that blanket denial with real user control, which is required before this is safe to
enable for general use (spec.md US3 "Why this priority").

**Independent Test**: With a delegate credential present (inserted directly for this phase's testing
purposes — the real connect flow is US2), pick it as the backend for a message and confirm the answer
is grounded in the delegate's own tool use for actions that don't need approval; an action needing
approval is cleanly declined (not hung, not silently allowed) (spec.md Acceptance Scenario 1).

### Tests for User Story 1

- [x] T010 [P] [US1] Integration test `src-tauri/tests/cli_delegate_claude.rs`: point
      `CliDelegateAdapter` (Claude vendor) at a small stub script standing in for the real `claude`
      binary that emits a canned `--output-format stream-json` transcript (shape captured in
      research.md §1); assert `stream_chat` yields the expected `StreamChunk::Delta`/`Done` sequence
      and never `ToolCalls`.
- [x] T011 [P] [US1] Unit test `src-tauri/src/adapters/cli_delegate/claude_tests.rs`: the NDJSON
      `stream-json` parser turns `content_block_delta`/`text_delta` events into `Delta` chunks and a
      terminal `result` event into `Done`; a malformed/truncated line surfaces as
      `AdapterError::Parse` rather than panicking.
- [x] T012 [P] [US1] Unit test `src-tauri/src/adapters/cli_delegate/codex_tests.rs`: the hand-rolled
      JSON-RPC framing serializes a request and parses a response/`ServerRequest` correctly, using
      fixtures based on `codex app-server generate-json-schema`'s output (research.md §2) as
      reference shapes.

### Implementation for User Story 1

- [x] T013 [US1] Implement `spawn_claude_invocation` in `src-tauri/src/adapters/cli_delegate/claude.rs`:
      build the `claude -p` `tokio::process::Command` (argv per data-model.md) **with
      `--permission-prompts none` and without `--mcp-config`/`--permission-prompt-tool`** — a safe,
      fail-closed default (anything needing approval is denied, never hangs waiting for a host that
      doesn't exist yet). T035 (US3) removes `--permission-prompts none` and adds the real
      `--mcp-config`/`--permission-prompt-tool` bridge. Also: fresh temp `CLAUDE_CONFIG_DIR` +
      disposable `cwd`, `CLAUDE_CODE_OAUTH_TOKEN` from decrypted credentials, guaranteed cleanup on
      every exit path — mirror `chat/tools/cli.rs`'s existing process-group setup and kill-on-cancel
      pattern.
- [x] T014 [US1] (depends on T008) Implement `spawn_codex_app_server` in
      `src-tauri/src/adapters/cli_delegate/codex.rs`: `codex app-server --stdio` process spawn, a
      fresh temp `CODEX_HOME` pre-populated with the decrypted `auth.json` bytes, disposable `cwd`,
      guaranteed cleanup; implement the JSON-RPC initialize handshake and turn/response parsing,
      mapping Codex's own answer events to `StreamChunk::Delta`/`Done` (research.md §2, §4). **Any
      incoming `ServerRequest` needing an approval decision gets T008's determined safe fail-closed
      response** (a stub, not real bridging) until T036 (US3) replaces it.
- [x] T015 [US1] Implement the `stream-json` NDJSON parser in `claude.rs` per T011's expected
      behavior.
- [x] T016 [US1] Wire `CliDelegateAdapter::stream_chat`/`list_models` (`cli_delegate/mod.rs`) to
      dispatch to `claude.rs`/`codex.rs` by `DelegateVendor`; `list_models` returns `Ok(vec![])` for
      both (data-model.md — no per-refresh model catalog for delegates).
- [x] T017 [US1] Add delegate backend selection to the chat UI (`src/components/settings/
DefaultModelSetting.vue` or a new sibling component, per plan.md — first real caller of
      `src/composables/useProviders.ts`'s existing `list`/`add` methods for this provider kind),
      showing a `cli_delegate` provider as "not connected" when it has no stored credential (spec.md
      Acceptance Scenario 2).
- [x] T018 [US1] **(new, analyze finding G1)** Verify or wire that a response answered by a delegate
      backend visibly identifies which one (spec.md FR-005, Acceptance Scenario 3): confirm whether
      the existing `provider_id`/model linkage already surfaced on `chat_messages` is rendered by the
      current message-list UI; if not, add the minimal rendering needed (a label such as "via Claude
      Code"/"via Codex"). No new backend field expected — this is a frontend-display check first.
- [x] T019 [US1] **(new, analyze finding G2)** In `claude.rs`/`codex.rs`, catch a process-spawn
      failure (binary not found / ENOENT) and map it to a distinct "backend not installed" error,
      separate from `AdapterError::InvalidCredentials` (spec.md FR-008, SC-006) — T026 (US2) only
      covers credential-related unavailability, not a missing binary.
- [x] T020 [P] [US1] Add `de`/`en` i18n strings (`src/i18n/locales/{de,en}.json`) for delegate backend
      labels, the "not connected" state, the backend-identity label (T018), and the "not installed"
      message (T019).

**Checkpoint**: A manually-connected delegate credential answers a request end-to-end through the
existing turn loop, with sensitive actions safely (if bluntly) denied. Still not the MVP alone —
Phases 4 and 5 (US2, US3) are both required.

---

## Phase 4: User Story 2 - Connect an existing subscription once, use it anywhere (Priority: P1) 🎯 MVP (part 2 of 3)

**Goal**: A user connects a Claude/Codex subscription from inside holzi once; the resulting encrypted
credential works from any machine holding that vault, with no host-level login state.

**Independent Test**: Complete the connection flow, copy the vault to a machine that has never run
`claude login`/`codex login`, and confirm the delegate backend works there immediately (spec.md
Acceptance Scenario 2).

### Tests for User Story 2

- [ ] T021 [P] [US2] Integration test `src-tauri/tests/cli_delegate_connect.rs`: `connect_cli_delegate`
      against a stub login binary that prints a fake token/`auth.json` to stdout — assert a
      `providers` row is inserted with `kind = CliDelegate`, `adapter = <vendor>`, non-`None`
      `credentials`; a second call for the same vendor updates that row in place instead of inserting
      a duplicate (upsert semantics, contracts/tauri-commands.md).
- [ ] T022 [P] [US2] Integration test: a connected delegate credential, read back and used with a
      freshly-isolated `CLAUDE_CONFIG_DIR`/`CODEX_HOME` (simulating "never logged into that provider
      before"), authenticates successfully using only the stored credential bytes.
- [ ] T023 [P] [US2] **(new, analyze finding G3)** Integration test: start a delegate-backed response,
      then delete/disconnect its `providers` row while that response is still in flight (spec.md
      FR-013, Edge Cases) — assert the in-flight response completes normally and only a _subsequent_
      request is affected by the disconnect.

### Implementation for User Story 2

- [ ] T024 [US2] Implement the `connect_cli_delegate` Tauri command in `src-tauri/src/providers/
mod.rs`: spawn `claude setup-token`/`codex login` in an isolated temp dir, capture the
      resulting token/`auth.json`, upsert the `providers` row via the existing insert path, emit
      `delegate-connect-progress` events (contracts/tauri-commands.md) including the OAuth URL to
      open; guarantee temp-dir cleanup on every exit path.
- [ ] T025 [US2] Register `connect_cli_delegate` in `src-tauri/src/lib.rs`'s `invoke_handler`.
- [ ] T026 [US2] Map a delegate auth failure (subprocess reports not logged in / a non-zero exit tied
      to credentials) to `AdapterError::InvalidCredentials` in `claude.rs`/`codex.rs`, reusing the
      existing error taxonomy (spec.md FR-014) — no new error variant.
- [ ] T027 [US2] Build `src/components/settings/ConnectDelegateProvider.vue` (new — first "add/connect
      provider" UI in the app, per plan.md): shows the OAuth URL from `delegate-connect-progress`,
      success/error states, and a "reconnect" action reachable when a request surfaces
      `InvalidCredentials` for a `cli_delegate` provider (spec.md FR-014).
- [ ] T028 [P] [US2] Add `de`/`en` i18n strings for the connect flow (URL prompt, success, error,
      reconnect messaging) — fixed reason strings from the backend, localized text only in the
      frontend (`CONTEXT.md` i18n boundary, same rule 003 followed).

**Checkpoint**: US1+US2 together let a user connect and use a delegate backend, with sensitive
actions safely denied by default. **Still not the full MVP** — Phase 5 (US3) is required before
enabling this for general use (spec.md's own priority rationale for US3).

---

## Phase 5: User Story 3 - Delegate actions stay under the user's approval control (Priority: P1) 🎯 MVP (part 3 of 3)

**Goal**: Replace Phase 3's safe-but-blunt "deny everything sensitive" default with a live
per-tool-call round-trip through holzi's existing approval gate, for both backends (research.md §1,
§2 — the batch-approval design this feature's spec originally proposed was dropped once live approval
was confirmed working).

**Independent Test**: Set an approval posture, trigger a delegate action needing approval, confirm
the delegate call visibly pauses until `respond_tool_permission` answers it, and confirm a
posture-blocked action is denied without ever surfacing a prompt (spec.md Acceptance Scenarios 1-4).

### Tests for User Story 3

- [ ] T029 [P] [US3] Integration test `src-tauri/tests/cli_delegate_approval.rs` (Claude path): a stub
      `claude` binary requests a `tools/call` against the permission MCP tool; assert
      `approval_bridge.rs` registers a `pending_tool_approvals` entry, emits `tool-permission-request`,
      and the stub only resumes after `respond_tool_permission` answers — the exact live round-trip
      manually verified in research.md §1. Exercises the real child-process-plus-socket path (T034,
      T035), not a simplified in-process stand-in.
- [ ] T030 [P] [US3] (depends on T008) Integration test (Codex path): a stub `codex app-server` sends
      an `item/commandExecution/requestApproval` request; assert the same bridge behavior and that
      the reply carries the correct `CommandExecutionApprovalDecision` (`accept` or `decline`).
- [ ] T031 [P] [US3] Unit test: `chat/tools/permission.rs`'s existing `decide()` is evaluated before
      any live request is made — `Decision::Allow`/`Deny` resolve without ever reaching
      `approval_bridge.rs`'s live round-trip (data-model.md).
- [ ] T032 [P] [US3] **(new, analyze finding G4)** Integration test, per backend: with the posture set
      to block sensitive actions (or Plan mode), a delegate action `decide()` resolves to `Deny` is
      answered immediately with no `tool-permission-request` ever emitted, and the delegate never
      gets to perform it (spec.md FR-007, SC-003) — distinct from T029/T030's `Ask`-path coverage.

### Implementation for User Story 3

- [x] T033 [US3] Implement `src-tauri/src/adapters/cli_delegate/approval_bridge.rs`, running in the
      **main** process: `request_approval` (data-model.md) — register into
      `ChatState.pending_tool_approvals`, emit `tool-permission-request` (`events.rs:28`, unchanged
      payload shape), await the oneshot, fail-safe-deny if the sender is dropped (bridge/process
      crash — spec.md Edge Cases). Also `start_approval_socket_listener`: create a fresh temp Unix
      domain socket (Windows: named pipe) via `tokio::net`, listen for the duration of one delegate
      invocation, and for each incoming `{tool_name, input}` message run `chat/tools/permission.rs`'s
      `decide()` first — `Allow`/`Deny` reply immediately (T032), only `Ask` calls `request_approval`
      (T029) — then remove the socket path on completion (RAII, like the temp directories).
- [x] T034 [US3] **(new — architecture fix, see revision notes)** Add a hidden internal-entrypoint
      branch to `src-tauri/src/lib.rs`/`main.rs`, checked first thing in `main()` before Tauri
      initializes: if invoked as `<self> --internal-cli-delegate-approval-bridge --socket <path>`,
      run only `cli_delegate::permission_mcp_server::run_bridge_process(path)` (T035) and exit —
      never start the GUI/Tauri runtime for this invocation. Smoke-test by running the built binary
      directly with the flag and confirming it exits cleanly with no window.
- [x] T035 [US3] (depends on T034) Implement `src-tauri/src/adapters/cli_delegate/
permission_mcp_server.rs`'s `run_bridge_process(socket_path)`, running in the **separate child
      process** that `claude` itself spawns (per MCP's stdio transport model — not in-process, see
      revision notes): an `rmcp` `server`+`transport-io` stdio MCP server (using this process's own
      inherited stdin/stdout, which is what `claude` actually talks to) exposing one `approve` tool;
      its `tools/call` handler connects to `socket_path` (`tokio::net`), sends `{tool_name, input}` as
      one JSON line, reads back the decision, and returns the corresponding MCP tool result. **Update
      T013's `claude -p` command**: remove `--permission-prompts none`, add
      `--mcp-config`/`--permission-prompt-tool` pointed at `std::env::current_exe()` with the
      `--internal-cli-delegate-approval-bridge --socket <path>` args from T033's listener.
- [x] T036 [US3] (depends on T008) Replace T014's fail-closed `ServerRequest` stub in `codex.rs` with
      real handling: run `decide()` first (immediate `Allow`/`Deny`, T032), calling
      `approval_bridge::request_approval` only for `Ask` (T030), translating the decision into the
      matching `CommandExecutionApprovalDecision` response (`accept`/`decline`). No child-process/socket hop needed here — holzi already
      owns this process's stdio directly (unlike the Claude Code path, T034/T035).

**Checkpoint**: spec.md SC-003 (100% of blocked sensitive actions actually blocked) now holds for
both delegate backends via live approval, not blanket denial. **US1+US2+US3 together are the MVP** —
the first point at which enabling a delegate backend for general use is safe per spec.md's own
priority rationale.

---

## Phase 6: User Story 4 - Delegate sees only what holzi gives it, not the host machine (Priority: P2)

**Goal**: Prove and harden the isolation T013/T014 already build in by construction — no host
config/instructions leak in, no residue leaks out (spec.md FR-009/FR-010).

**Independent Test**: On a machine with existing host-level Claude Code/Codex configuration and
instruction files, trigger a delegate request and confirm none of it affected the response (spec.md
Acceptance Scenarios 1-3).

### Tests for User Story 4

- [ ] T037 [P] [US4] (depends on T009) Integration test: with a fake `~/.claude/settings.json` (a
      `SessionStart` hook plus a permissive `defaultMode`) and a fake project-local
      `.claude/settings.json`/`CLAUDE.md` planted outside the delegate's temp `cwd`, assert a delegate
      invocation's isolated `CLAUDE_CONFIG_DIR`/`cwd` never triggers the hook and never reads that
      content — the exact leak scenario found manually in research.md §3.
- [ ] T038 [P] [US4] Integration test: after a delegate invocation completes (success and failure
      paths), assert its temp `CLAUDE_CONFIG_DIR`/`CODEX_HOME`/`cwd` directories **and** T033's
      approval-bridge socket path no longer exist on disk (spec.md FR-003).

### Implementation for User Story 4

- [x] T039 [US4] Pass holzi-supplied context explicitly via `--append-system-prompt` (`claude.rs`) and
      Codex's equivalent turn-context field (`codex.rs`) rather than any file-based discovery (spec.md
      FR-010) — confirm/implement the Codex-side equivalent as part of this task (research.md §2
      flags Codex's system-prompt-equivalent flag as still unconfirmed).
- [x] T040 [US4] Add an RAII/drop-guard wrapper around each delegate invocation's temp directory in
      both `claude.rs` and `codex.rs`, and around T033's approval-bridge socket, so cleanup runs on
      every exit path (success, error, panic unwind), not only the happy path (spec.md FR-003) —
      satisfies T038. Apply the same guard to `connect_cli_delegate`'s (T024) temp dir.

**Checkpoint**: T037/T038 pass — host machine state verified to never leak in or out.

---

## Phase 7: User Story 5 - Stop a delegate response mid-run (Priority: P3)

**Goal**: Stopping a delegate-backed response terminates the underlying subprocess immediately, the
same way `chat/tools/cli.rs`'s host-command tool already does.

**Independent Test**: Start a delegate-backed response doing something observable, stop it, confirm
the OS process is actually gone (spec.md Acceptance Scenario 1).

### Tests for User Story 5

- [ ] T041 [P] [US5] Integration test: stop a running delegate-backed response mid-call and assert the
      underlying OS process is terminated (not just the stream dropped) — check process exit,
      matching `chat/tools/cli.rs`'s existing process-group-kill test pattern. For the Claude Code
      path, also assert the T035 bridge child process (if still running) is not left orphaned.

### Implementation for User Story 5

- [x] T042 [US5] Track the delegate subprocess handle (both vendors, plus the Claude Code bridge child
      process from T035 if currently running) on the in-flight session/turn state in
      `src-tauri/src/chat/session.rs`, mirroring how `chat/tools/cli.rs` tracks its child for
      cancellation.
- [ ] T043 [US5] Extend `abort_current_generation` (`src-tauri/src/chat/commands.rs`) to kill the
      tracked delegate process group(s) (matching `cli.rs`'s existing SIGKILL/`taskkill` pattern) when
      the active session's backend is a delegate.

**Checkpoint**: All five user stories independently functional.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T044 [P] Verify `de`/`en` i18n lockstep for every string added across T020/T028 (`CONTEXT.md`
      i18n boundary requirement), the same check 003's T041 performed.
- [ ] T045 [P] Run `specs/007-cli-delegate/quickstart.md` end-to-end manually (all 5 scenarios) with
      whichever of `claude`/`codex` is installed, before merge.
- [ ] T046 `cargo test --lib` and the new `src-tauri/tests/cli_delegate_*.rs` suites green; `pnpm
typecheck` exit 0.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **Foundational (Phase 2)**: Depends on Setup. BLOCKS all user stories. Includes both verification
  spikes (T008, T009) — see their specific gating notes below.
- **User Stories (Phase 3-7)**: All depend on Foundational completion. Priority order is
  US1 → US2 → US3 → US4 → US5, matching spec.md; **US1, US2, and US3 together form the MVP** — US3
  is not optional/deferrable the way US4/US5 are, per spec.md's own priority rationale (a tool-capable
  backend is only acceptable alongside real approval control).
- **Polish (Phase 8)**: Depends on all desired user stories being complete.

### Specific Task Dependencies (beyond phase order)

- **T008** (Codex live-approval spike) gates T014, T030, T036 — every Codex-specific
  implementation/test task. Claude-only work (T013, T015, T020 etc.) is unaffected.
- **T009** (Claude skills/plugins isolation spike) gates T037.
- **T005** (signature threading) must land before T006 (adapter construction) and before any of
  T013/T014 (which need the handles T006 passes to `CliDelegateAdapter::new`).
- **T034** (hidden internal entrypoint) must land before **T035** (the process it invokes).
- **T013** (Claude base command, `--permission-prompts none`) is _modified in place_ by **T035**
  (removes that flag, adds the live `--mcp-config`/`--permission-prompt-tool` bridge) — not a fresh
  file, an edit to the same command-building code. Same relationship between **T014**'s fail-closed
  stub and **T036**'s real bridging for Codex.
- **T013**/**T014** (process spawning) must land before **T042** (needs a process handle to track).

### Parallel Opportunities

- T001, T002 (Setup) in parallel.
- T004, T007, T008, T009 (Foundational, different files/independent verification) in parallel with
  each other and with T003/T005/T006 where file-independent.
- Within each user story phase, tasks marked [P] (different files) run in parallel; sequential tasks
  within a phase generally touch the same file (e.g. `claude.rs` across T013/T015) or have a stated
  dependency.
- US4 and US5 (Phases 6-7) can proceed in parallel with each other once Phase 5 (US3) is done, since
  neither depends on the other.

---

## Implementation Strategy

### MVP First (User Stories 1 + 2 + 3)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational — including both spikes (T008, T009), even though their downstream
   tasks are still a few phases away; doing them early derisks the rest of the feature.
3. Complete Phase 3: User Story 1 (delegate answers requests; sensitive actions denied by default).
4. Complete Phase 4: User Story 2 (connect flow, vault portability).
5. Complete Phase 5: User Story 3 (live approval replaces the blanket denial).
6. **STOP and VALIDATE**: run quickstart.md Scenarios 1-3 end-to-end.
7. Ship the MVP — a user can connect and use a subscription backend, with real approval control over
   its sensitive actions. Host-isolation hardening (US4) and stop/cancellation parity (US5) follow.

### Incremental Delivery

1. Setup + Foundational → foundation ready, both spikes' outcomes known.
2. US1 → delegate can answer, safely but bluntly (everything sensitive denied).
3. US2 → connect flow, vault portability.
4. US3 → blanket denial replaced with live, per-action approval — **this is the actual MVP
   checkpoint**, not US1+US2 alone.
5. US4 → host-isolation guarantees verified by tests, not just by construction.
6. US5 → stop/cancellation parity with other backends.
7. Polish → i18n lockstep, full quickstart pass, green test suite.
