---
description: 'Actionable, dependency-ordered task list for Autonomous Delegate Mode'
---

# Tasks: Autonomous Delegate Mode

**Input**: Design documents from `/specs/009-autonomous-delegate-mode/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-commands.md

**Tests**: Backend tests are REQUIRED (repo convention: every module gets its own `*_tests.rs`,
never inline `#[cfg(test)] mod tests`, plus integration tests under `src-tauri/tests/` — same
convention 007-cli-delegate's own tasks.md followed). Frontend verification is manual per
quickstart.md.

**Revision notes**:

- **2026-09-17, post `/speckit.analyze`**: fixed the three HIGH-severity findings from the analysis
  pass. (1) **G1** — FR-013/SC-006 ("native full-autonomy mechanism unavailable → specific error, no
  silent fallback") had zero task coverage; added T009 (a reactive spawn-error classifier, not a
  proactive version probe — matches how 007 already surfaces "backend unavailable" for other failure
  classes) and its wiring (T022), plus a dedicated test (T015). (2) **U1** — `GatedPermissive`'s
  synchronous `evaluate_deny_rules` call bypasses the `Ask`-branch's existing
  `receiver.await.unwrap_or(Deny)` fail-safe (approval_bridge.rs:65) entirely; T019 (was T016) now
  explicitly requires its own fail-safe-to-`Deny` wrapping, with a dedicated test (T016). (3) **G3** —
  T012 (was T011) now explicitly asserts host-isolation env vars survive the `Ungated`-mode edit to
  `build_command`, not just that the permission flags changed — this was the operator's own specific
  concern from this feature's design session, not a generic nice-to-have. Four new tasks in total;
  everything from old T009 onward shifted by the corresponding offset (see each task's own history if
  needed — old numbers are not repeated here to avoid a stale cross-reference table). Three
  MEDIUM/LOW findings from the same analysis pass (CredentialPaths integration-test parity,
  "gated-but-permissive" vs "gated-permissive" spelling drift, the unenumerated credential-path glob
  list) were intentionally left unfixed in this revision — not asked for, not blocking.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1..US5)

## Path Conventions

- Backend: `src-tauri/src/` and `src-tauri/tests/` (Rust)
- Frontend: `src/` (Nuxt 4 SPA), i18n at `src/i18n/locales/{de,en}.json`
- Docs: `specs/009-autonomous-delegate-mode/` (this feature's design docs)

---

## Phase 1: Setup (Shared Infrastructure)

- [ ] T001 Create `src-tauri/src/adapters/cli_delegate/autonomy.rs` as an empty module with a doc
      comment pointing at data-model.md; register `pub(crate) mod autonomy;` in
      `src-tauri/src/adapters/cli_delegate/mod.rs`.

**Checkpoint**: Module skeleton in place. No behavior changes yet.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: New types, the schema change, and request-plumbing every user story needs.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T002 [P] Define `AutonomyMode` enum (`Standard` default / `Ungated` / `GatedPermissive`,
      `#[serde(rename_all = "snake_case")]`) in `src-tauri/src/adapters/cli_delegate/autonomy.rs`
      (data-model.md).
- [ ] T003 Add `autonomy_mode: AutonomyMode` field (`#[serde(default)]`) to `ChatRequest` in
      `src-tauri/src/adapters/mod.rs` (research.md §1 — no config struct exists to extend; this is
      the one cross-cutting field every adapter now receives and non-delegate adapters ignore).
- [ ] T004 [P] Add migration `0017_chat_messages_add_autonomy_mode` in
      `src-tauri/src/identity/migrations.rs` (pattern: `0014_chat_messages_tool_columns` at
      migrations.rs:280-292): `ALTER TABLE chat_messages ADD COLUMN autonomy_mode TEXT;` and bump
      `HOLZI_TRIGGER_VERSION` (migrations.rs:59, currently `8`) to `9` per the doc comment convention
      at migrations.rs:20 (data-model.md's Decision B).
- [ ] T005 [P] Extend `SendMessageArgs` in `src-tauri/src/chat/commands.rs:50-66` with
      `autonomy_mode: Option<AutonomyMode>` (wire name `autonomyMode` via the struct's existing
      `#[serde(rename_all = "camelCase")]`); thread it into the `ChatRequest` built inside
      `send_message` (contracts/tauri-commands.md).
- [ ] T006 [P] Define `DenyCategory` enum (`WorkspaceEscape` / `NetworkAccess` / `CredentialPaths`,
      `#[serde(rename_all = "snake_case")]`) in `autonomy.rs` (data-model.md).
- [ ] T007 [P] Define `ApprovalRequestPayload` enum (`ClaudeToolCall { tool_name, input }` /
      `CodexCommandExecution { command, cwd, network }` / `CodexFileChange` with no fields) in
      `autonomy.rs` (data-model.md — `CodexFileChange` is deliberately field-less, matching the real
      schema gap found in research.md §3, not a placeholder to fill in later).
- [ ] T008 Implement `evaluate_deny_rules(enabled: &[DenyCategory], workspace_root: &Path, payload: &ApprovalRequestPayload) -> ApprovalDecision`
      in `autonomy.rs` per the matching table in data-model.md. `ApprovalRequestPayload` already
      identifies the vendor, so no redundant vendor parameter is accepted. Use only the explicit
      invocation `workspace_root` for workspace checks, never the ambient process directory;
      `Allow` if no enabled category
      matches, `Deny` otherwise, and **`Deny` for any enabled `WorkspaceEscape`/`CredentialPaths`
      category against a `CodexFileChange` payload regardless of content** (spec FR-015 — cannot
      evaluate, fail closed, not a bug to "fix" by adding fields that don't exist upstream).
- [ ] T009 [P] Define an internal `AutonomyUnavailable { vendor: DelegateVendor, mode: AutonomyMode }`
      classification in `autonomy.rs` (spec FR-013/SC-006) and a
      `classify_autonomy_spawn_error(vendor, mode, raw_error: &str) -> Option<AutonomyUnavailable>`
      helper. Recognize Claude Code's "unknown option"-shaped stderr for an unsupported
      `--permission-mode` value and Codex's `thread/start` JSON-RPC error response for an unrecognized
      `approvalPolicy`/`sandbox` field. T022 maps this classification to
      `AdapterError::Unavailable` using contracts/tauri-commands.md's exact reason. This is
      **reactive** (attempt the call, classify the failure) rather than a proactive CLI-version probe
      — matches how 007 already surfaces "backend unavailable" for its own other failure classes
      (spec FR-008 precedent), so this feature does not invent a second detection strategy.
- [ ] T010 [P] Define `const PREF_DENY_RULES: &str = "cli_delegate.deny_rules";` and the fallible
      helpers `get_deny_rules(conn, device_id) -> Result<Vec<DenyCategory>>` and
      `set_deny_rules(conn, device_id, categories) -> Result<()>` over
      `storage::preferences::{get, insert_or_update}`
      with `PrefScope::Device` in `autonomy.rs` (research.md §5 — JSON-encode/decode the array into
      the preference's `String` value; absent or valid `[]` values behave as an empty list, while
      malformed JSON returns an error so the approval flow can fail closed).
- [ ] T011 [P] Create `src-tauri/src/adapters/cli_delegate/autonomy_tests.rs` (register via
      `#[cfg(test)] #[path = "autonomy_tests.rs"] mod autonomy_tests;`): unit tests for
      `evaluate_deny_rules` covering all 3 categories × all 3 `ApprovalRequestPayload` variants from
      the data-model.md table, explicitly asserting the `CodexFileChange` fail-closed case for
      `WorkspaceEscape`/`CredentialPaths` (FR-015) and the "no enabled categories → always `Allow`"
      empty-rule-set edge case (spec's own documented edge case). Use distinct explicit workspace
      roots to prove evaluation does not depend on the ambient working directory. Test that an absent
      preference and valid `[]` return an empty list while malformed JSON returns an error. Also test
      `classify_autonomy_spawn_error` against representative Claude "unknown option" and Codex RPC
      error strings.

**Checkpoint**: Types, migration, and request-plumbing exist. Nothing in `claude.rs`/`codex.rs`/
`approval_bridge.rs` reads any of it yet — user story implementation can begin.

---

## Phase 3: User Story 1 - Let a trusted delegate work without pausing for every action (Priority: P1) 🎯 MVP (part 1 of 2, ships with US2)

**Goal**: `ungated` and `gated-permissive` both suppress the live human approval pause for both
vendors without weakening host isolation; `standard` is byte-for-byte unchanged; an installed CLI
too old to support the requested mode fails with a specific, honest error instead of silently doing
something else.

**Independent Test**: quickstart.md Scenario 1.

### Tests for User Story 1

- [ ] T012 [P] [US1] Integration test in `src-tauri/tests/cli_delegate_autonomy_ungated.rs` (new
      file, following the stub-binary approach 007's own `cli_delegate_*` tests already use):
      confirm `ungated` mode's Claude command omits `--mcp-config`/`--permission-prompt-tool`
      entirely and Codex's `thread/start` params carry `"approvalPolicy":"never"` +
      `"sandbox":"workspace-write"`; confirm zero `tool-permission-request` events fire for either
      vendor even when the stub simulates a tool call that would be `Risky` under `standard`. **Also
      explicitly assert that `CLAUDE_CONFIG_DIR`/disposable `cwd` (claude.rs:139-140) and
      `CODEX_HOME`/disposable `cwd` (codex.rs) are still set exactly as in `standard` mode** — this
      edit touches the same `build_command` function that provides 007's host-isolation guarantee
      (FR-011); the test must fail if isolation is ever weakened as a side effect of the permission
      flag change, not just check that the flag itself is correct.
- [ ] T013 [P] [US1] Integration test in `src-tauri/tests/cli_delegate_autonomy_gated_permissive.rs`
      (new file): with an empty deny-rule set, confirm `gated-permissive` fires zero
      `tool-permission-request` events for either vendor, but the approval bridge **is** still
      invoked (assert via a test-only counter/log hook on `evaluate_deny_rules`, not just "nothing
      visible happened") — this is the behavioral difference from `ungated` that US2 depends on.
- [ ] T014 [P] [US1] Regression test confirming `standard` mode (autonomy_mode omitted/default)
      is unchanged from 007: extend the existing `chat_tool_loop_permissions.rs`-style delegate
      coverage to assert identical `tool-permission-request` behavior with and without this
      feature's code present (i.e. explicitly construct a `ChatRequest` with default `autonomy_mode`
      and assert it takes the `permission::decide` path, not `evaluate_deny_rules`).
- [ ] T015 [P] [US1] Integration test: stub `claude`/`codex` binaries that reject the `Ungated`-mode
      flag/field the way an older installed version would (unknown-option stderr for Claude; an RPC
      error response for Codex), request `Ungated` mode, and confirm `send_message` returns the
      exact `HolziError::InvalidInput` envelope from contracts/tauri-commands.md, including the
      required vendor and mode — not a silent fallback to `Standard` and not a generic spawn-failure
      message (spec FR-013/SC-006 edge case; exercises T009's classifier end-to-end).
- [ ] T016 [P] [US1] Integration test: force the `cli_delegate.deny_rules` preference read to fail
      (or inject a failure at a test-only seam around `evaluate_deny_rules`) during a
      `GatedPermissive` invocation, and confirm the pending action resolves to `Deny` — not `Allow`,
      not an indefinite hang (spec FR-010, generalized beyond the `Ask`-path's existing
      `unwrap_or(Deny)` safety net, which this synchronous path does not go through).

### Implementation for User Story 1

- [ ] T017 [US1] `claude.rs::build_command` (claude.rs:117-145): branch on `req.autonomy_mode` —
      `Ungated` replaces `.arg("--permission-mode").arg("default")` with
      `.arg("--permission-mode").arg("bypassPermissions")` and omits the
      `--mcp-config`/`--permission-prompt-tool` args; `Standard`/`GatedPermissive` keep today's
      exact command unchanged (research.md §2).
- [ ] T018 [US1] `codex.rs::spawn_codex_app_server`'s `thread/start` params (codex.rs:299-316):
      branch on `req.autonomy_mode` — `Ungated` sets `"approvalPolicy": "never"` and adds
      `"sandbox": "workspace-write"` (a field 007 never sends today); `Standard`/`GatedPermissive`
      keep `"approvalPolicy": "on-request"` unchanged (research.md §2 — verified against the real
      `ThreadStartParams` schema, not the standalone `codex exec` CLI's flags).
- [ ] T019 [US1] `approval_bridge::request_approval` (approval_bridge.rs:25-68): add an
      `AutonomyMode` parameter and thread the invocation workspace root into this flow as a separate
      value (do not add it to `ApprovalRequestPayload`); for `GatedPermissive`, skip
      `permission::decide(mode, risk)` and
      instead build the appropriate `ApprovalRequestPayload` (T007) from `tool_name`/`input` and call
      `autonomy::evaluate_deny_rules` (T008) with T010's persisted deny rules and that explicit root.
      Never populate `pending_tool_approvals` or emit `tool-permission-request` for this mode;
      `Standard` keeps calling `permission::decide` exactly as today. **This path bypasses the
      `Ask`-branch's existing
      `receiver.await.unwrap_or(Deny)` fail-safe (approval_bridge.rs:65) entirely, so it MUST provide
      its own**: map T010 parsing/read errors and evaluator errors to `Deny`; a malformed persisted
      deny-rule value must never become an empty permissive rule set. This satisfies FR-010
      independently of the channel-based mechanism (T016 tests this explicitly).
- [ ] T020 [US1] `mod.rs::CliDelegateAdapter::stream_chat` (mod.rs:195-224) and the
      `spawn_claude_invocation`/`spawn_codex_app_server` signatures (claude.rs:148-153,
      codex.rs:240-245): add `req.autonomy_mode` as a new parameter and thread the invocation
      workspace root separately through every T019 caller (research.md §1 — no existing config
      struct to extend, both take positional scalars today).
- [ ] T021 [US1] In `mod.rs`, gate the `approval_bridge::bind_socket`/`start_listener`/MCP-config-file
      setup (currently unconditional in `spawn_claude_invocation`, claude.rs:179-199-ish) behind
      `req.autonomy_mode != AutonomyMode::Ungated` for the Claude path specifically — `Ungated` never
      needs a bridge process at all, not merely one that goes unused.
- [ ] T022 [US1] In `spawn_claude_invocation`/`spawn_codex_app_server` (claude.rs, codex.rs): when
      `req.autonomy_mode != Standard` and the underlying process/RPC call fails, run the failure
      through T009's `classify_autonomy_spawn_error` before falling through to the existing generic
      error path; on a match, return `AdapterError::Unavailable` with the exact
      `autonomy mode '<mode>' is unavailable for '<vendor>': <detail>` reason so the existing provider
      mapping produces contracts/tauri-commands.md's `HolziError::InvalidInput` envelope. Do not add a
      `HolziError` variant or regenerate bindings for this internal classification.

**Checkpoint**: User Story 1 fully functional and independently testable — both new modes suppress
approval for both vendors without weakening host isolation, an unsupported CLI version fails
honestly, and `standard` is regression-free.

---

## Phase 4: User Story 2 - Review what an unsupervised run actually did (Priority: P1) 🎯 MVP (part 2 of 2)

**Goal**: `gated-permissive` persists a full per-tool-call record labeled with its autonomy mode;
`ungated` persists only the final response, also labeled.

**Independent Test**: quickstart.md Scenario 2.

### Tests for User Story 2

- [ ] T023 [P] [US2] Integration test: a `gated-permissive` run with 2+ tool calls produces 2+
      persisted `chat_messages` rows, each with `autonomy_mode = 'gated_permissive'`.
- [ ] T024 [P] [US2] Integration test: an `ungated` run with delegate tool activity produces exactly
      one assistant row with `autonomy_mode = 'ungated'` and zero `tool_call`/`tool_result` rows for
      that turn (spec FR-006 — absence is the expected, tested behavior, not merely unverified).

### Implementation for User Story 2

- [ ] T025 [US2] Apply migration `0017` (T004); run the full `cargo test --lib` suite to confirm the
      `HOLZI_TRIGGER_VERSION` bump doesn't break existing CRDT sync/migration tests.
- [ ] T026 [US2] Wherever 007 already persists `cli_delegate:claude`/`cli_delegate:codex`
      `tool_source` rows and the delegate's final assistant message (the turn-persistence path a
      delegate invocation's `AdapterStream` feeds into): also set the new `autonomy_mode` column
      from `req.autonomy_mode` on every row for that turn, including the assistant-message row for
      `Ungated` (T024's requirement — the label must exist even with zero tool-call rows).
- [ ] T027 [US2] Frontend: wherever 007's FR-005 "make clear which backend answered" label is
      already rendered in chat history, add the `autonomy_mode` label alongside it (new i18n strings
      per T041 in Polish).

**Checkpoint**: User Stories 1 AND 2 (the full MVP) independently functional together.

---

## Phase 5: User Story 3 - Keep specific actions off-limits even while otherwise autonomous (Priority: P2)

**Goal**: operator-configured, persisted deny rules are enforced under `gated-permissive`, with the
documented per-vendor/per-action-type asymmetry (FR-015) holding exactly as specified.

**Independent Test**: quickstart.md Scenario 3.

### Tests for User Story 3

- [ ] T028 [P] [US3] Integration test: enabling `network_access` blocks a Codex call carrying
      `networkApprovalContext` and a Claude `WebFetch`-tool call, under `gated-permissive`, for both
      vendors.
- [ ] T029 [P] [US3] Integration test: enabling `workspace_escape`, a Codex **file-change** approval
      (not command execution) is denied per FR-015's fail-closed rule — the specific case
      quickstart.md Scenario 3 steps 6-7 call out. Must fail if a future change accidentally starts
      allowing it through.
- [ ] T030 [P] [US3] Integration test: no deny rules configured → every action proceeds under
      `gated-permissive` (end-to-end version of T011's unit-level empty-set case).

### Implementation for User Story 3

- [ ] T031 [US3] Create `src/components/settings/DelegateDenyRulesSetting.vue` (plan.md): a
      3-item checklist over `DenyCategory`, using `getPrefAsync`/`setPrefAsync` against
      `cli_delegate.deny_rules`, modeled on `DefaultModelSetting.vue`'s device-scope preference
      pattern; placed in settings alongside (not inside) `ConnectDelegateProvider.vue`.
- [ ] T032 [US3] Wire Codex's `respond_to_server_request` dispatch (codex.rs:123-155) to build
      `ApprovalRequestPayload::CodexCommandExecution { command, cwd, network }` from
      `CommandExecutionRequestApprovalParams` and `ApprovalRequestPayload::CodexFileChange` (no
      fields) from `FileChangeRequestApprovalParams`, passed with the invocation workspace root into
      T019's `evaluate_deny_rules` call.
- [ ] T033 [US3] Wire Claude's MCP `tools/call` handler (`permission_mcp_server.rs`'s
      `call_approval`/`approval_bridge.rs`'s `request_approval`) to build
      `ApprovalRequestPayload::ClaudeToolCall { tool_name, input }` from the incoming arguments,
      passed with the invocation workspace root into T019's `evaluate_deny_rules` call.

**Checkpoint**: User Stories 1-3 independently functional — deny rules enforceable with the
documented asymmetry, not a silent gap.

---

## Phase 6: User Story 4 - Autonomy never silently carries over to the next request (Priority: P2)

**Goal**: the structural guarantee behind spec FR-008.

**Independent Test**: quickstart.md Scenario 4.

### Tests for User Story 4

- [ ] T034 [P] [US4] Integration test: two consecutive `send_message` calls on the same thread,
      first with `autonomyMode: 'ungated'`, second with the field omitted — confirm the second is
      gated exactly like `standard` (i.e. re-run T014's assertion as a same-thread follow-up call,
      not just a fresh request).

### Implementation for User Story 4

- [ ] T035 [US4] Frontend `src/pages/chat/[instance].vue`: add a local `autonomyMode` ref that
      resets to `'standard'` after every send (never written via `setPrefAsync`, never read on
      mount) — mirrors `updatePermissionMode`'s update pattern (lines 483-500) but explicitly
      without persistence, satisfying FR-008 at the UI layer on top of T003/T005's structural
      per-request plumbing.
- [ ] T036 [US4] Create `src/components/chat/DelegateAutonomyControl.vue` (plan.md): a third
      `ChatComposerControl`, following `PermissionPrompt.vue`'s `mode`/`update:mode` prop shape
      exactly (PermissionPrompt.vue:13-14,24-29), rendered in the composer toolbar
      (`[instance].vue:1032-1060`) only when the active model resolves to a `cli_delegate` provider
      (reusing the existing `p.kind === 'cli_delegate'` lookup at `[instance].vue:461`); emits
      `update:mode` consumed by T035.

**Checkpoint**: User Stories 1-4 independently functional.

---

## Phase 7: User Story 5 - Stop an autonomous response mid-run (Priority: P3)

**Goal**: the existing stop/cancellation mechanism from 007 covers both new modes with no gap.

**Independent Test**: quickstart.md Scenario 5.

### Tests for User Story 5

- [ ] T037 [P] [US5] Integration test: start an `ungated` and a `gated-permissive` request each
      doing something slow/observable, call the existing stop path for each, confirm the underlying
      child process is actually terminated (extends 007's own cancellation test pattern, which only
      exercised `standard`, to both new modes explicitly).

### Implementation for User Story 5

- [ ] T038 [US5] Verify (fix only if T037 finds a gap) that 007's existing
      `AdapterStream::new_with_cancellation`/`CancellationToken` mechanism needs no new code for
      either new mode — neither `claude.rs`'s nor `codex.rs`'s process-spawn/kill path branches on
      `autonomy_mode` (T017/T018 only change command/params construction, not the spawn/kill
      lifecycle), so this task is expected to be verification-only.

**Checkpoint**: All 5 user stories independently functional.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T039 [P] Run `cargo test --lib` on both CI feature-matrix legs; run `pnpm typecheck`.
- [ ] T040 [P] i18n completeness: every new user-visible string (autonomy mode labels/descriptions on
      `DelegateAutonomyControl.vue`, deny-category labels/descriptions on
      `DelegateDenyRulesSetting.vue`, the "autonomy mode unavailable for this backend" message from
      contracts/tauri-commands.md/T022) exists in both `src/i18n/locales/de.json` and `en.json`, and
      `useErrorString` maps the contract's exact `InvalidInput` reason envelope to
      `errors.delegate.autonomyUnavailable` instead of displaying the backend's English reason
      (CONTEXT.md's lockstep requirement).
- [ ] T041 Execute quickstart.md's 5 scenarios manually end-to-end, on whichever of Claude
      Code/Codex is actually installed for review; record which vendor(s) were exercised.
- [ ] T042 [P] Update `docs/plans/2026-09-17-autonomous-delegate-mode-design.md`'s status header to
      point at this shipped feature, mirroring how 007-cli-delegate's own design doc predecessor was
      updated once its spec/tasks existed.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup. **BLOCKS all user stories.**
- **User Stories (Phase 3-7)**: All depend on Foundational completion.
  - US1 and US2 together are the MVP (spec marks both P1, same reasoning 007 used for bundling its
    own P1 stories) — implement together, not US1 alone.
  - US3, US4, US5 each build additively on US1+US2's plumbing but are independently testable per
    their own Independent Test in spec.md.
- **Polish (Phase 8)**: Depends on all desired user stories being complete.

### Specific Task Dependencies (beyond phase order)

- T003 (ChatRequest field) blocks T005, T017, T018, T019, T020 — every downstream task reads
  `req.autonomy_mode`.
- T008 (evaluate_deny_rules) blocks T019 (its only caller) and T011 (its tests).
- T009 (classify_autonomy_spawn_error) blocks T022 (its caller) and T015 (its test).
- T010 (deny-rule preference helpers) blocks T019 (reads persisted rules) and T031 (writes them).
- T004 (migration) blocks T025 (apply) and T026 (write the column) and T023/T024 (assert on it).
- T007 (ApprovalRequestPayload) blocks T008, T032, T033.
- T020 (threading `autonomy_mode` into the spawn functions) blocks T021, T022, and every US1 test
  (T012-T016).

### Parallel Opportunities

- T002, T004, T005, T006, T007, T009, T010 (Phase 2) touch disjoint files/regions and can run in
  parallel once T001 exists.
- Within each user story's Tests subsection, all `[P]`-marked tasks target different new test files
  and can run in parallel.
- US3's T031 (frontend settings) and T032/T033 (backend payload wiring) can run in parallel — no
  shared file.
- Different user stories cannot usefully run in parallel by different people beyond US1+US2 (the
  bundled MVP) — US3/US4/US5 each depend on US1's `evaluate_deny_rules`/autonomy-mode plumbing
  existing first.

---

## Parallel Example: Phase 2 (Foundational)

```bash
# After T001 (module file exists), launch together:
Task: "Define AutonomyMode enum in src-tauri/src/adapters/cli_delegate/autonomy.rs"
Task: "Add migration 0017_chat_messages_add_autonomy_mode in src-tauri/src/identity/migrations.rs"
Task: "Extend SendMessageArgs in src-tauri/src/chat/commands.rs"
Task: "Define DenyCategory enum in src-tauri/src/adapters/cli_delegate/autonomy.rs"
Task: "Define ApprovalRequestPayload enum in src-tauri/src/adapters/cli_delegate/autonomy.rs"
Task: "Define AutonomyUnavailable + classify_autonomy_spawn_error in src-tauri/src/adapters/cli_delegate/autonomy.rs"
Task: "Define deny-rule preference helpers in src-tauri/src/adapters/cli_delegate/autonomy.rs"
```

---

## Implementation Strategy

### MVP First (User Stories 1 + 2 together)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational (critical — blocks everything).
3. Complete Phase 3 (US1) + Phase 4 (US2) together — spec.md gives both P1 for the same reason: an
   autonomy mode with no review trail is a much harder thing to ship than one with a record.
4. **STOP and VALIDATE**: run quickstart.md Scenarios 1 and 2 against a real connected delegate.
5. This is a mergeable, demoable MVP on its own — US3/US4/US5 are real value-adds, not
   prerequisites for the core feature to be usable.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. US1 + US2 → validate → mergeable MVP.
3. US3 (deny rules) → validate independently → merge.
4. US4 (never-persists guarantee) → validate independently → merge.
5. US5 (stop) → validate independently → merge.
6. Polish.

### Notes

- Every implementation task in US1-US3 references an exact existing file:line from plan.md/
  research.md — none of them are "create X from scratch," they are all bounded edits to
  007-cli-delegate's existing code, matching this feature's additive-only Constitution Check result.
- Verify each user story's tests fail before its implementation tasks land, per repo convention.
- Commit after each task or logical group; stop at any checkpoint to validate a story independently.
