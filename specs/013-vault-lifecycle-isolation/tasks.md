---
description: 'Task list for Vault Lifecycle Isolation'
---

# Tasks: Vault Lifecycle Isolation

**Input**: Design documents from `specs/013-vault-lifecycle-isolation/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-commands.md,
contracts/frontend-surface.md, quickstart.md

**Tests**: Included as mandatory, not optional. The spaex constitution requires test code in files
separate from production code and one runnable check per piece of non-trivial logic. Rust tests live
in sibling `*_tests.rs` files declared with the repository's existing
`#[cfg(test)] #[path = "x_tests.rs"] mod x_tests;` idiom (see `src-tauri/src/voice.rs`) or under
`src-tauri/tests/`. Frontend tests are cases in the replay harness (`pnpm check:chat-state` for the
existing cases, `pnpm check:vault-lifecycle` for the new ones, both CI gates). No new test runner.
Async tests use paused time and channels, never arbitrary sleeps.

**Organization**: The plan's seven delivery stages, in the operator's order. Every stage compiles and
passes its own checks. Stage 1 (secret hygiene) does not depend on the gate and comes first; Stage 2
is the foundation that Stages 3 to 5 need. User story labels: US1 close locks everything, US2 a new
vault sees nothing of the old one, US3 the passphrase never lingers, US4 two app processes side by
side, US5 reads never fail.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on another unchecked task)
- **[Story]**: US1 to US5 (only in story phases; the setup, foundational and polish phases have none)
- All paths are repository-relative and written in full: Rust files start with `src-tauri/`, frontend
  files with `src/` or `scripts/`.
- Commits follow Conventional Commits, one per checkpoint, **without** any agent or model trailer
  (constitution MUST NOT). Work lands on `main` through a PR from this branch; no squash-merge.
- Inside the Nix dev shell: `nix develop --command scripts/with-nix-host-bridge.sh cargo ...` for
  Rust, `nix develop --command pnpm ...` for the frontend.
- `cargo test` rewrites `src/types/bindings/*.ts` with trailing whitespace. After ordinary test runs,
  `git checkout -- src/types/bindings/`. When a task intentionally changes a binding, run
  `pnpm generate:ts-types` instead (it strips the whitespace) and commit the result.
- **Oversized files must not grow** (line counts recorded in T003): `models/commands.rs`,
  `chat/commands.rs`, `chat/model_loading.rs`, `providers/mod.rs`, `src/pages/chat/[instance].vue`,
  `scripts/check-chat-state.ts`. Edit call sites in place; new logic goes into new small files.

---

## Phase 1: Setup

- [x] T001 Work happens in `.worktrees/013-vault-lifecycle-isolation` on branch
      `013-vault-lifecycle-isolation` with a real `pnpm install` (no symlinked `node_modules`).
- [x] T002 Already on this branch and not to be redone: the tokio-util `rt` feature (`f4814b6`), the
      `active_model_info` fix without the operation slot (FR-025, `fedcfa8`), and the `tauri` `test`
      dev-dependency (own commit). The spike lives only on the local branch `spike/vault-gateway` and
      never merges; read it with `git show spike/vault-gateway:src-tauri/tests/spike_vault_gateway.rs`.
- [ ] T003 Record the baseline in the "Baseline" section at the end of this file before any edit:
      `cargo test --manifest-path src-tauri/Cargo.toml` pass count, `pnpm check:chat-state` test count,
      and the results of `pnpm typecheck`, `pnpm typecheck:scripts`, `pnpm lint`,
      `pnpm format:check`, plus `wc -l` of the six oversized files listed above.
- [x] T004 Decided by the operator on 2026-09-21: the fix commit `fedcfa8` travels in PR A together
      with the docs, and any available `gh` account may be used for pushes and PRs, including the
      haex-crdt repository. The `haexhub` account is read-only on this repository, so pushes use
      `haexmas`; restore the previously active account afterwards. Recorded in the Baseline section.
- [ ] T005 [P] Extract the replay harness. Create `scripts/lib/chat-state-harness.ts` and move into
      it, from `scripts/check-chat-state.ts`, the sandbox machinery: the transpile cache,
      `runComposable`, `createTauriDouble`, `DEFAULT_INVOKE_HANDLERS`, `RETURN_STATEMENT`,
      `createChatState`, `flush` and any helper they need. Export what the test bodies use. Test
      bodies, names and count stay unchanged (compare with T003). Both files stay under 500 lines
      where possible; `check-chat-state.ts` must get shorter. Update its header comment (extraction
      done). `pnpm typecheck:scripts` already covers `scripts/**/*.ts`. Commit
      `refactor(scripts): extract the chat-state replay harness`.
- [ ] T006 Depends on T005. Create `scripts/check-vault-lifecycle.ts` importing the shared harness,
      with one smoke case that boots `createChatState()`. Add `"check:vault-lifecycle"` to
      `package.json` next to `check:chat-state` and a step after it in `.github/workflows/ci.yml`
      (same shape as the `check:chat-state` step). New frontend cases in later phases go in this file,
      never into the oversized `check-chat-state.ts`.

---

## Phase 2: Stage 1 — Secret hygiene in Rust (US3, FR-013 to FR-015, SC-004, SC-005)

**Goal**: The passphrase is held in an erasing, redacted type and is never cloned. Independent of the
gate; the existing open flow keeps working.

**Independent test**: `cargo test` proves redaction and erasure; a manual unlock and wrong-passphrase
attempt still behave as before.

### Tests for Stage 1 (write first; expected to fail until the implementation tasks land)

- [ ] T007 [P] [US3] Create `src-tauri/src/instances/passphrase_tests.rs` with cases: (a) `{:?}` and
      `{:#?}` of `Passphrase`, `OpenInstanceArgs` and `CreateInstanceArgs`, built from a distinctive
      literal, never contain the literal and do contain `<redacted>`; (b) `Passphrase` deserializes
      from a JSON string; (c) calling `zeroize()` on a `Passphrase` leaves `as_str()` empty; (d) a
      static assertion that `Passphrase` is `ZeroizeOnDrop`; (e) `From<&str>` and `From<String>`.

### Implementation for Stage 1

- [ ] T008 [US3] In `src-tauri/Cargo.toml` add `zeroize` 1.x as a direct dependency with the `serde`
      feature. Verify with `cargo metadata` that it still resolves to 1.9.0 and that the
      `Cargo.lock` diff is only holzi's own dependency list (no new package). Commit
      `build(deps): depend on zeroize directly`.
- [ ] T009 [US3] Create `src-tauri/src/instances/passphrase.rs` (declare it in `src-tauri/src/instances/mod.rs`): a
      newtype `Passphrase(Zeroizing<String>)`. Data-model rule verbatim: "Erased on drop; never
      cloned; excluded from `Debug` (prints `<redacted>`)". No `Clone`. `Deserialize` from a string,
      `as_str()`, `From<&str>`, `From<String>`, `Zeroize` delegating to the inner string, and
      `ZeroizeOnDrop`. Declare `passphrase_tests.rs` at the bottom with the `#[path]` idiom.
- [ ] T010 [US3] Change `OpenInstanceArgs` (`src-tauri/src/instances/open.rs`, around line 34) and
      `CreateInstanceArgs` (`src-tauri/src/instances/create.rs`) to `passphrase: Passphrase`, keeping the wire
      shape `{ name, passphrase }` and adding `#[ts(type = "string")]` so the binding stays `string`.
      Keeping `#[derive(Debug)]` is now safe because the field type redacts.
- [ ] T011 [US3] Remove every passphrase `String` clone. In `src-tauri/src/instances/open.rs` move
      `args.passphrase` into an `Arc<Passphrase>` shared by the validation task and the open task
      (Arc clones are handles, not copies) and pass `as_str()` to `pragma_update` and
      `open_existing_database`. In `src-tauri/src/instances/create.rs` move the passphrase into the blocking
      open task (the clone near line 105). No `.clone()` on the secret remains.
- [ ] T012 [US3] Compiler-driven sweep of tests and fixtures that build these args: run
      `rg -n "OpenInstanceArgs|CreateInstanceArgs" src-tauri/src src-tauri/tests` and switch the
      literals to `Passphrase::from(...)`.
- [ ] T013 [US3] Review every use of the passphrase for leaks (FR-015): a case-insensitive `rg`
      search for passphrase over `src-tauri/src`, checking `log::`, `format!`, `to_string`,
      `Display` and error construction. Record "no leak found" or the fixes made in the Baseline
      section. Errors such as `HolziError::WrongPassphrase` must stay fieldless.
- [ ] T014 [US3] Run `pnpm generate:ts-types` and confirm `src/types/bindings/OpenInstanceArgs.ts`
      and `CreateInstanceArgs.ts` are unchanged (still `string`). Any other diff is investigated,
      not committed blindly.
- [ ] T015 [US3] **Checkpoint Stage 1**: `cargo fmt --check`, `pnpm lint:rust` (both feature sets),
      `cargo test`, then `git checkout -- src/types/bindings/`. Commit
      `feat(instances): keep the vault passphrase in an erasing, redacted type`.

---

## Phase 3: Stage 2 — Foundational: the gate, the tracker, the wrapper (blocks Stages 3 to 5)

**Goal**: One gateway that every request passes, one tracker that knows what is still running, and a
database handle that carries a tracker token. Nothing changes for users yet, because no code asks for
a close until Stage 3.

**Independent test**: `cargo test` covers the phase machine, the drain ladder, the handle guard and
the wrapper over the Tauri mock runtime; the app behaves exactly as before.

### Preparation

- [ ] T016 Graphify consultation before authoring the new named artifacts (`VaultGate::run`,
      `CloseEffects`, `retry_while_locked`, `VaultDb`). The worktree has no `graphify-out/`, so run
      the queries from the primary checkout snapshot (read only): "run a future until a cancellation
      token fires", "retry an operation until a file lock is free", "abstract the side effects of
      closing so tests can record them", "wrap a shared handle with a drop guard counter". Add the
      candidates and decisions to the table in `research.md` R10. If a query fails or returns
      nonsense, warn, continue, and flag the skipped consultation in the same table (constitution).
- [ ] T017 Confirm the app-scoped allow-list by reading each command (do not trust the list). Locate
      each with `rg -n "fn <name>" src-tauri/src`: `close_instance`, `list_instances`,
      `get_hardware_info`, `list_catalog`, `catalog_recommend_tiers`, `list_stt_catalog`,
      `stt_recommend_tiers`. For each, check that neither it nor any helper it calls reaches
      `active_database`, `ChatState` or vault data. Record a verdict per command as a table under
      `research.md` R3. Remove a command from the list in `contracts/tauri-commands.md` if it fails.
      Also read `list_installed_stt_models` and `download_stt_model`; they stay default-deny unless
      they are clearly app-scoped and needed on the unlock screen, and the decision is recorded.

### Tests for Stage 2 (write first; expected to fail until the implementation tasks land)

- [ ] T018 [P] Create `src-tauri/src/vault_gate/gate_tests.rs`, quoting the data-model rules: "`phase` only
      moves forward (`Idle` → `Active` → `Closing`), and `Idle` → `Closing` is allowed"; "`cancel`
      fires at most once, on the first transition into `Closing`"; "A second close request changes
      nothing and reports that a close is already running (FR-002)". Also: `begin_session` while
      `Active` is `VaultAlreadyActive`, while `Closing` is `VaultClosed`; `begin_session` from `Idle`
      succeeds exactly once when two threads race.
- [ ] T019 [P] Create `src-tauri/src/vault_gate/drain_tests.rs` with `#[tokio::test(start_paused = true)]` and a
      blocking closure that waits on a channel the test releases, so no wall-clock sleeps are needed:
      a cooperative task yields `Drained`; a task that ignores the token yields `DrainedAfterAbort`;
      a blocking closure still running at the limit yields `Stuck`, and the call returns within the
      total limit (about 3 s of virtual time). Add a plain `#[test]` for the forced end: the action
      given to `hard_end_after` runs once after the grace period and never before it (a few
      milliseconds of real time, waiting on a channel, not a sleep).
- [ ] T020 [P] Create `src-tauri/src/vault_gate/db_tests.rs`: a `VaultDb` clone keeps the tracker non-empty
      while any clone is alive and lets it empty after the last drop; `Deref` reaches the `Database`.
      Open a throwaway SQLCipher database the way `src-tauri/src/instances/startup_tests.rs` does (`tempfile`).
- [ ] T021 [P] Create `src-tauri/tests/vault_gateway.rs` over the Tauri mock runtime (the `tauri` `test`
      dev-dependency is already present): in `Idle` and `Active` every command passes; in `Closing`
      a non-allow-listed command is rejected at once with `{"kind":"VaultClosed"}` and its body never
      runs (assert with a counter), an allow-listed command passes, and a command that is not in
      either list (a "new" command) is denied, proving default-deny.

### Implementation for Stage 2

- [ ] T022 Create `src-tauri/src/vault_gate/mod.rs` (declare `mod vault_gate;` in `src-tauri/src/lib.rs`): `VaultGate`,
      cheap to `Clone` (fields behind `Arc`), with `phase: Mutex<VaultPhase>` (short-held std
      mutex), `cancel: CancellationToken`, `tasks: TaskTracker`, `aborts: Mutex<Vec<AbortHandle>>`.
      `VaultPhase` is the enum `Idle | Active | Closing`. API: `begin_session()`,
      `ensure_can_open()`, `request_close()` (reports whether this call was the first), `is_closing()`,
      `token()`, `spawn(fut)` and `spawn_blocking(f)` (both tracked; `spawn` uses
      `TaskTracker::spawn_on` with `tauri::async_runtime::handle().inner()` so tasks run on Tauri's
      runtime, registers the abort handle and prunes finished handles), `run(fut)` (races the future
      against the token and returns `Err(HolziError::VaultClosed)` when the token wins), and a way to
      hand out a tracker token. Also `ClosePolicy` (`Relaunch` | `Exit`) and `close_policy()` (debug
      builds return `Exit` until T046 says otherwise). Keep every new file under 500 lines.
- [ ] T023 Create `src-tauri/src/vault_gate/drain.rs`: the ladder from research R5 (fire the token, wait up to
      the cooperative window, abort registered tasks, wait until the total limit) returning
      `DrainOutcome` (`Drained` | `DrainedAfterAbort` | `Stuck`, exactly the data-model table). The
      deadlines are named constants with a `ponytail:` comment: fixed 1 s and 3 s, ceiling "not
      adaptive to slow disks or busy machines", upgrade path "make them configurable in one place".
      Add `hard_end_after(grace, action)`: it starts a plain `std::thread` (independent of tokio and
      of the window event loop) that waits the grace period and then runs `action`. Its named
      constant is 500 ms, with a `ponytail:` comment: fixed 0.5 s, ceiling "a stuck exit path costs
      half a second more", upgrade path "none needed".
- [ ] T024 Create `src-tauri/src/vault_gate/db.rs`: `VaultDb { db: Arc<Database>, _token: TaskTrackerToken }`
      with `Clone` and `Deref<Target = Database>`, exactly the data-model entity.
- [ ] T025 Create `src-tauri/src/vault_gate/invoke.rs`: `APP_SCOPED_COMMANDS` (the list confirmed in T017) and
      `VaultGate::wrap`, returning a closure `Fn(Invoke<R>) -> bool`. It reads the name with
      `invoke.message.command()`; in `Closing` and not allow-listed it calls
      `invoke.resolver.reject(HolziError::VaultClosed)` and returns `true`, otherwise it calls the
      inner handler.
- [ ] T026 Add `VaultClosed` and `VaultAlreadyActive` to `src-tauri/src/error.rs` (fieldless; messages "The
      vault is closed" and "A vault is already open in this app process"). Do not remove
      `CloseFailed` yet. Run `pnpm generate:ts-types` and commit the regenerated bindings.
- [ ] T027 Encapsulate the vault state. In `src-tauri/src/state.rs` make `active_instance` private and add the
      methods from the data-model table (`database`, `install`, `take`); `AppState::new` takes a
      `VaultGate` clone so `active_database(&State<AppState>)` in `src-tauri/src/state_utils.rs` keeps its
      signature and now returns a `VaultDb` (or `VaultClosed` / `NoActiveInstance`). `install` will
      call `begin_session` under the same lock in Stage 4; for now it publishes only.
- [ ] T028 In `src-tauri/src/lib.rs` build the gate once, pass a clone to `AppState::new`, `.manage(gate)`, and
      register `invoke_handler(gate.wrap(tauri::generate_handler![...]))`. The handler list itself
      is unchanged.
- [ ] T029 Compiler-driven sweep to `VaultDb`: the callers in `src-tauri/src/chat/model_loading.rs`,
      `src-tauri/src/chat/default_model.rs`, `src-tauri/src/chat/commands.rs`, `src-tauri/src/chat/thread_commands.rs`,
      `src-tauri/src/models/commands.rs`, `src-tauri/src/providers/mod.rs`, `src-tauri/src/providers/connect.rs`,
      `src-tauri/src/storage/preferences_commands.rs`, `src-tauri/src/device/commands.rs` and `src-tauri/src/voice.rs`. Change
      types only (for example `Arc<Database>` parameters); no logic change and no net line growth in
      the oversized files. `src-tauri/src/instances/{open,create,close}.rs` keep reading the state through the
      new methods.
- [ ] T030 [P] Update `data-model.md` where the implementation differs from it (for example
      `AppState` holds a gate clone, so `active_database` keeps its signature; `VaultDb` handling).
- [ ] T031 **Checkpoint Stage 2**: `cargo fmt --check`, `pnpm lint:rust`, `cargo test`,
      `git checkout -- src/types/bindings/` (after committing the intended T026 bindings). Behavior is
      unchanged for users. Commit `feat(vault-gate): route every request through one gateway`.

---

## Phase 4: User Story 1 — Closing locks everything at once, every time (P1) 🎯 MVP

**Goal**: FR-001 to FR-009, SC-001, SC-002. `close_instance` cannot fail, cancels the work, replaces
the page at once, drains within about 3 s and ends the process.

**Independent test**: quickstart scenarios 1, 2, 3 and 8: close with a streaming reply and a running
download; press close repeatedly; close by window; the process ends within about 4 s with no error.

### Manual checks that decide the design (record each outcome)

- [ ] T032 [US1] Research R4: find where a static file must live so it is served in `pnpm dev` and
      present in the built output. Try `public/closing.html` at the repository root and
      `src/public/closing.html` (the Nuxt config sets `srcDir: 'src/'`). Check
      `http://localhost:3030/closing.html` under `pnpm dev` and `.output/public/closing.html` after the
      build command named in `src-tauri/tauri.conf.json` (`beforeBuildCommand`). Keep the location
      that works. Replace **Open** in `research.md` R4 with the **Outcome**. If neither works, record
      that `about:blank` is used.
- [ ] T033 [US1] Create `closing.html` at the location found in T032: a centered spinner and nothing
      else, **no text of any kind** (operator decision 2026-09-21, so nothing needs translating).
      Pure CSS: a bordered ring turned by a `@keyframes` rotation in an inline `<style>`
      (`style-src` allows `'unsafe-inline'`), no script (the CSP is `script-src 'self'`), no image,
      GIF or external resource, no vault data. The background follows `prefers-color-scheme` and
      matches the app's light and dark backgrounds so dark mode gets no white flash;
      `prefers-reduced-motion` may slow the rotation but must not hide the spinner. Update the
      "Closing page" section of `contracts/frontend-surface.md` accordingly.

### Tests for User Story 1 (write first; expected to fail until the implementation tasks land)

- [ ] T034 [P] [US1] In `src-tauri/src/chat/session_tests.rs` add cases for `ChatState::reset_for_close`: it
      clears the loaded session, both approval maps, the tool cancellation slot, the current
      generation handle and the tool registry, and is idempotent.
- [ ] T035 [P] [US1] Create `src-tauri/tests/vault_lifecycle_close.rs` using a recorder implementation of the
      close effects (defined in T038), a gate, a `ChatState`, a `VoiceState` and a throwaway
      database: (a) `close_instance` returns `Ok` at once while a tracked task that never finishes
      and a held operation slot exist (the old "operation still in progress" failure, FR-002);
      (b) a second call is `Ok` and repeats no effect; (c) phase 1 effects happen once and in order:
      gate closing, token fired, turn aborted, page navigated, list-changed emitted; (d) the process
      end is requested exactly once with the configured policy, for `Drained`, `DrainedAfterAbort` and
      `Stuck`, and the forced end is armed once after that request and fires when the fake effect
      does not end the process; (e) the database is dropped only after every `VaultDb` clone is dropped, and always
      before the process end; (f) after the close, the wrapper rejects a vault command over the mock
      runtime; (g) a turn started before the close is cancelled through the token.
- [ ] T036 [P] [US1] Verify FR-008 by reading, then test any gap: `src-tauri/src/chat/turn/persist.rs` for how
      a cancelled turn persists (it must not look complete), and the staging cleanup in
      `src-tauri/src/models/download.rs` and `cleanup_staging_in_dir` in `src-tauri/src/models/import.rs` for partial
      downloads. Add a case to the existing sibling test files if a gap exists, and record the
      finding either way in the Baseline section.

### Implementation for User Story 1

- [ ] T037 [US1] Add `ChatState::reset_for_close` in `src-tauri/src/chat/session.rs` (currently 345 lines)
      reusing the existing fields and `abort_turn` semantics; do not duplicate the abort logic.
- [ ] T038 [US1] Define the close effects: a small trait `CloseEffects` in
      `src-tauri/src/vault_gate/mod.rs` with four methods (show the closing page, emit
      `instance-list-changed` for a closed vault, request the end of the process for a
      `ClosePolicy`, force the end of the process for a `ClosePolicy`). Implement it
      for the real app in a new file `src-tauri/src/instances/close_effects.rs`: navigate the main
      webview with `Webview::navigate` to the current URL joined with `closing.html` (fallback
      `about:blank`), emit the event, and end the process with `AppHandle::request_restart()` for
      `Relaunch` or `AppHandle::exit(0)` for `Exit` (never `restart()`, research R2). The forced end
      is `tauri::process::restart(&app.env())` for `Relaunch` and `std::process::exit(0)` for
      `Exit`. Failures are logged, never returned.
- [ ] T039 [US1] Rewrite `src-tauri/src/instances/close.rs` per `contracts/tauri-commands.md`. Phase
      1 is synchronous and infallible: `gate.request_close()`, fire the token, `abort_turn` (reuse
      the existing function in `src-tauri/src/chat/commands.rs`, do not copy it), the page effect,
      the event, start phase 2, return `Ok(())`. Phase 2 (background, on a plain
      `tauri::async_runtime::spawn`, not tracked, because it is the task that drains the tracker):
      cancel the preload and wait, run the drain ladder, take the database out with
      `AppState::take`, drop it, call `ChatState::reset_for_close` and
      `VoiceState::invalidate_whisper_cache`, then request the end of the process and arm the forced
      end with `hard_end_after` and `CloseEffects::force_end`. The command no longer calls
      `acquire_operation`.
- [ ] T040 [US1] Delete `CloseFailed` from `src-tauri/src/error.rs`, remove the stale doc comment in
      `src-tauri/src/state.rs`, and regenerate bindings with `pnpm generate:ts-types`.
- [ ] T041 [US1] Register session-scoped tasks with the gate and add the token where work runs long:
      the chat turn task (`src-tauri/src/chat/commands.rs`, the `spawn` around line 607) and the preload
      (`src-tauri/src/chat/default_model.rs`, keep its own token and join handle and also register it), both
      voice tasks (`src-tauri/src/voice.rs`, around lines 179 and 211) and the provider connect completion
      (`src-tauri/src/providers/connect.rs`, around line 245) through `gate.spawn`. Read
      `src-tauri/src/adapters/cli_delegate/connect_claude.rs` (the `std::thread::spawn` near line 145): convert
      to `gate.spawn_blocking` only if trivial; otherwise leave it and record it as a residual that the
      3 s limit covers.
- [ ] T042 [US1] Make long-running commands stop on close with `gate.run(...)`, returning
      `VaultClosed`: `load_model_command` in `src-tauri/src/chat/model_loading.rs`, the transfer in
      `download_from_hf_inner` and `import_model_from_file` (`src-tauri/src/models/commands.rs`, the copy or
      download loop in `src-tauri/src/models/download.rs`), `refresh_provider_models` in `src-tauri/src/providers/mod.rs`,
      voice start and stop in `src-tauri/src/voice.rs`, and the connect commands in `src-tauri/src/providers/connect.rs`.
      These are one-line call-site edits; the oversized files must not grow.
- [ ] T043 [US1] In `src-tauri/src/lib.rs` switch from `.run(context)` to `.build(context)?.run(callback)` and
      handle FR-007: on `WindowEvent::CloseRequested` and on `RunEvent::ExitRequested` (unless its
      code is `RESTART_EXIT_CODE` or the gate is already closing), prevent the default, run the same
      close task with policy `Exit`, and exit when it finishes. Wire the real `CloseEffects`.
- [ ] T044 [US1] Frontend: make the lock flows do one thing. `lock()` in
      `src/pages/chat/[instance].vue` (around line 499) and `onLock()` in
      `src/pages/federation/[instance].vue` call `closeAsync()`, swallow a rejection, and no longer
      call `store.setActiveInstance(null)` or `navigateTo('/')`. The backend replaces the page
      (research R4), so the frontend keeps no closing state and shows no overlay (operator decision
      2026-09-21). The chat page must end up with no more lines than before.
- [ ] T045 [US1] Add the script-setup sandbox to the harness: a helper in
      `scripts/lib/chat-state-harness.ts` (or a sibling file) that loads a small `.vue` file's
      `<script setup>` block, injecting `defineProps`, `defineEmits`, `ref`, `computed`, `watch`,
      `onBeforeUnmount`, `useI18n`, `useInstance`, `useInstancesStore` and `navigateTo`, and returns
      the bindings named by the caller. Then add cases in `scripts/check-vault-lifecycle.ts`: the
      chat page `lock()` (add `lock` to the returned bindings) and the federation `onLock()` call
      `close_instance` once, swallow a rejected close, never navigate and never clear the active
      instance.
- [ ] T046 [US1] Manual check, research R2: run `pnpm tauri:dev`, unlock a scratch vault, close it with
      relaunch forced, and observe whether the dev runner keeps going, restarts the app or stops.
      Record the outcome in `research.md` R2 and set `close_policy()` accordingly (debug builds stay
      `Exit` unless the relaunch works).
- [ ] T047 [US1] Manual check, research R5: with a small local model, start a long generation, close,
      and record how long until the process ends and whether the engine stopped on its own. Record it
      in `research.md` R5. If it exceeds 3 s the limit still ends the process; note the residual.
- [ ] T048 [US1] Run quickstart scenarios 1, 2, 3 and 8 and record pass/fail with dates in a new
      "Validation record" section at the end of `quickstart.md`.
- [ ] T049 [US1] Retire the spike: map each spike test on `spike/vault-gateway` to its replacement
      (extractor accept and reject to `src-tauri/tests/vault_gateway.rs`, in-flight end to
      `src-tauri/tests/vault_lifecycle_close.rs`, drain ladder to
      `src-tauri/src/vault_gate/drain_tests.rs`; the epoch test is not needed) and confirm every
      behavior it proved is covered. The file is not on this branch. Once PR D is merged, delete the
      local branch with `git branch -D spike/vault-gateway`. Keep the `tauri` `test` dev-dependency.
- [ ] T050 [US1] **Checkpoint Stage 3 (MVP)**: full CI parity (see T085), revert the binding
      whitespace churn (format notes at the top) after committing the intended T040 bindings.
      Commits: `feat(vault): close is immediate, infallible and ends the process`,
      `feat(ui): replace the page with a closing spinner`.

---

## Phase 5: User Story 2 — A new vault never sees anything from the previous one (P1)

**Goal**: FR-010 to FR-012, SC-003. One app process serves at most one vault session; opening or
creating while one exists is refused.

**Independent test**: quickstart scenario 4: markers in vault A are absent in vault B, and
`open_instance` while A is active returns `VaultAlreadyActive` with no change.

### Tests for User Story 2 (write first)

- [ ] T051 [P] [US2] Create `src-tauri/tests/vault_single_session.rs` over the mock runtime: with the gate
      `Active`, `open_instance` and `create_instance` return `VaultAlreadyActive`; with the gate
      `Closing` they return `VaultClosed`; in both cases no file or directory is created (call with
      a temporary data location and assert it stays empty, so the gate check must run before any path
      is resolved). Add a case for FR-010: a failed open (wrong passphrase) leaves the gate `Idle`, and
      an open that follows succeeds.
- [ ] T052 [P] [US2] In `src-tauri/src/vault_gate/gate_tests.rs` add: `AppState::install` publishes the handle
      and calls `begin_session` atomically, so two concurrent installs leave exactly one winner and
      the loser's handle is dropped.

### Implementation for User Story 2

- [ ] T053 [US2] `src-tauri/src/instances/open.rs`: check `gate.ensure_can_open()` first, before path
      resolution. Delete the "already active with the same name" credential branch with its second
      SQLite connection, and the atomic-switch block that drops the previous handle. Publish with
      `AppState::install` (which calls `begin_session` under the same lock). The passphrase is now a
      plain move into the blocking open task (drop the `Arc` from T011). The file must be shorter
      than before.
- [ ] T054 [US2] `src-tauri/src/instances/create.rs`: the same early gate check and `AppState::install`.
- [ ] T055 [US2] `AppState::install` in `src-tauri/src/state.rs`: hold the state lock, call
      `gate.begin_session()`, and publish only on success; return `VaultAlreadyActive` or
      `VaultClosed` otherwise.
- [ ] T056 [US2] Confirm no frontend path opens a vault while one exists: search `src` for
      `openAsync` and read `src/pages/index.vue` and `src/components/onboarding/UnlockSheet.vue`.
      Record the finding in the Baseline section; change nothing unless a path exists.
- [ ] T057 [P] [US2] Write `docs/adr/0003-one-vault-session-per-app-process.md` in the format of the
      existing ADRs (Status accepted, Date, Decision, Rationale): one app process serves at most one
      vault session, closing ends the process, supersedes spec 001 FR-022. Include a short "adding
      work later" note: session-scoped work uses `gate.spawn` and `gate.run`; a new command is gated
      by default and is only allow-listed if it never touches the vault.
- [ ] T058 [P] [US2] Add a supersession note to `specs/001-frontend-onboarding/contracts/tauri-commands.md`
      (the `open_instance` and `close_instance` sections) and to FR-022 in
      `specs/001-frontend-onboarding/spec.md`, pointing to spec 013 FR-010 and ADR 0003. Keep the
      files Prettier-clean.
- [ ] T059 [US2] Run quickstart scenario 4 and record it in the Validation record.
- [ ] T060 [US2] **Checkpoint Stage 4**: full CI parity. Commit
      `refactor(instances): one vault session per app process`.

---

## Phase 6: User Story 4 — Two app processes side by side (P2)

**Goal**: FR-018 to FR-023, FR-026, SC-006, SC-007, SC-010. A clear message for a vault held
elsewhere, a short retry while that process finishes closing, a process-safe model install, a current
vault list, and a startup cleanup that never deletes another process's work in progress.

**Independent test**: quickstart scenario 6 with two processes and two vaults.

### Tests for User Story 4 (write first)

- [ ] T061 [P] [US4] Create `src-tauri/src/instances/lock_retry_tests.rs` with paused time: the helper retries
      while the error is `VaultAlreadyOpenElsewhere`, returns the value once the error clears, returns
      the error after the window, returns `VaultClosed` if the gate starts closing during the wait,
      and does not retry any other error.
- [ ] T062 [P] [US4] Create `src-tauri/src/models/paths_tests.rs` (declare it in `src-tauri/src/models/paths.rs` with the
      `#[path]` idiom): a second acquisition of the same slug waits while the first lock is held,
      succeeds after it drops, and returns `VaultClosed` when the cancellation token fires while
      waiting. Data-model rule verbatim: "Holds the in-process mutex guard for the slug **and** an
      exclusive lock on `<models>/.locks/<slug>.lock`. Both release on drop. Acquiring polls
      asynchronously so a close can cancel the wait."
- [ ] T063 [P] [US4] Add cases to `src-tauri/src/models/commands_tests.rs` and the existing import tests: the
      installed-slug scan (`src-tauri/src/models/commands.rs`, around line 597) and `cleanup_staging_in_dir`
      (`src-tauri/src/models/import.rs`, around line 61) ignore a `.locks` directory and never treat its files as
      models or staging leftovers.
- [ ] T064 [P] [US4] In `scripts/check-vault-lifecycle.ts` add: `useErrorString` maps
      `VaultAlreadyOpenElsewhere`, `VaultAlreadyActive` and `VaultClosed` to their `errors.*` keys;
      `UnlockSheet` shows the dedicated message for `VaultAlreadyOpenElsewhere` and still shows the
      generic `errors.openFailed` for `WrongPassphrase` and `NotFound` (spec 001 FR-021); `pages/index.vue`
      re-syncs the list on window focus and when the unlock sheet opens.
- [ ] T065 [P] [US4] Create `src-tauri/src/instances/presence_tests.rs` (declare it in
      `src-tauri/src/instances/presence.rs` with the `#[path]` idiom) over a temporary data location:
      the first `announce` runs its callback while it is the only process; a second `announce` while
      the first handle is alive does not run its callback; after the first handle drops, a new
      `announce` runs its callback again; the callback runs before the lock is downgraded, so a
      concurrent starter cannot begin work in between. Add a case to
      `src-tauri/src/instances/startup_tests.rs`: with a `.pending` marker, its `.db` and a model
      staging leftover in place, running startup cleanup through `announce` removes them when alone
      and removes nothing while another presence is alive (FR-026, SC-010).

### Implementation for User Story 4

- [ ] T066 [US4] Create `src-tauri/src/instances/lock_retry.rs` with `retry_while_locked` (an attempt closure, a
      window, a poll interval and the gate token). Add a `ponytail:` comment: fixed 100 ms poll
      inside a 3 s window, ceiling "adds up to one poll interval of latency", upgrade path "none
      needed". Declare `lock_retry_tests.rs` with the `#[path]` idiom.
- [ ] T067 [US4] Use it in `src-tauri/src/instances/open.rs`: attempts that fail with
      `HolziError::VaultAlreadyOpenElsewhere` are retried for up to 3 s, polling with an async sleep
      between blocking attempts so a close can interrupt, then the error is returned.
- [ ] T068 [US4] Implement `PublicationLock` in `src-tauri/src/models/paths.rs`: the guard returned by
      `acquire_model_publication_lock`, taking `(app, slug, cancellation token)`. It holds the
      in-process mutex guard and an exclusive lock on `<models>/.locks/<slug>.lock` (create the
      directory and file), taken with `std::fs::File::try_lock` and polled with an async sleep; the
      wait ends with `VaultClosed` if the token fires. Add a `ponytail:` comment on the poll (fixed
      50 ms; ceiling "latency of one interval"; upgrade "none needed"). Update the two call sites in
      `src-tauri/src/models/commands.rs` (around lines 393 and 535) without net line growth.
- [ ] T069 [US4] Make the scans skip dot directories: `src-tauri/src/models/commands.rs` around line 597 and
      `src-tauri/src/models/import.rs` around line 61.
- [ ] T070 [US4] Create `src-tauri/src/instances/presence.rs` with `ProcessPresence` (data-model.md):
      `announce(dir, on_alone)` opens `presence.lock` in the app local data directory (the same
      directory `create_instance` uses for the installation id file) and tries an exclusive
      `File::try_lock`. When it succeeds it runs `on_alone` while still holding the lock, so nobody
      else can start work in between. It then holds a shared lock on the same handle for the rest of
      the process; when the exclusive attempt fails it waits for the shared lock, which ends as soon
      as the other starter finishes its cleanup. The handle lives in managed state, and the OS drops
      the lock when the process ends or crashes. Add a `ponytail:` comment: cleanup only runs when
      the process is alone; ceiling "leftovers of a crashed process stay while other processes
      overlap"; upgrade path "per-item locks". Keep the file under 500 lines.
- [ ] T071 [US4] Gate the startup cleanup: in `src-tauri/src/lib.rs` `setup`, pass the existing
      `cleanup_orphans_on_startup` (which stays as it is in `src-tauri/src/instances/startup.rs`,
      covering both `cleanup_orphans_in_dir` and `cleanup_staging_in_dir`) to
      `ProcessPresence::announce` as the callback and manage the returned handle. The relaunch after
      a close runs while the old process is still draining, so it skips the cleanup; that is
      intended. No other production code may call the two cleanup functions.
- [ ] T072 [US4] Frontend errors: add the three kinds to `src/composables/useErrorString.ts`, add
      `errors.vaultAlreadyOpenElsewhere`, `errors.vaultAlreadyActive` and `errors.vaultClosed` with
      the texts from `contracts/frontend-surface.md` to `src/i18n/locales/de.json` and `en.json`, and
      make `UnlockSheet.vue` show the dedicated message for `VaultAlreadyOpenElsewhere` only.
- [ ] T073 [US4] `src/pages/index.vue`: re-sync the vault list on window `focus` and on
      `visibilitychange`, and when the unlock sheet opens; remove the listeners on unmount.
- [ ] T074 [US4] Run quickstart scenario 6 (two processes, same vault twice, list refresh, concurrent
      model install, and a third start during a download and a vault creation) and record it in the
      Validation record.
- [ ] T075 [US4] **Checkpoint Stage 5**: full CI parity. Commit
      `feat(instances): support independent app processes side by side`.

---

## Phase 7: Stage 6 — passphrase lifetime in the interface and reads that never fail (US3, US5)

**Goal**: FR-016 and FR-025. The forms keep the passphrase only as long as the spec allows, and the
chat view never errors on overlapping reads.

**Independent test**: `pnpm check:vault-lifecycle` and quickstart scenarios 5 and 7.

### Tests (write first)

- [ ] T076 [P] [US3] In `scripts/check-vault-lifecycle.ts`, using the script-setup sandbox from T045:
      for `UnlockSheet` and `CreateSheet`, the field is cleared after a successful unlock or create
      (before the `unlocked` or `created` event is emitted), cleared when the sheet is dismissed,
      and kept after a failed attempt; and the serialized state of every Pinia store never contains
      the passphrase marker. A close needs no case here: the backend discards the page.
- [ ] T077 [P] [US5] In the same file: with a backend double that answers the first
      `active_model_info` slowly, `initialize()` issues overlapping reads, resolves, re-reads the
      providers and leaves the effort control selectable. Add a comment naming the incident (a read
      that took the exclusive operation slot aborted `initialize()`).

### Implementation

- [ ] T078 [US3] Add the explicit clears in `src/components/onboarding/UnlockSheet.vue` and
      `CreateSheet.vue`: on success before emitting. The existing `reset()` on dismissal stays, and a
      close needs no clear because the page is discarded. Keep `v-model` on `UiInputPassword` (an
      external component) unchanged.
- [ ] T079 [US3] Run quickstart scenarios 5 and 7 and record them in the Validation record.
- [ ] T080 [US3] **Checkpoint Stage 6**: full CI parity. Commit
      `test(ui): cover passphrase lifetime and overlapping model reads`.

---

## Phase 8: Stage 7 — upstream key wipe in haex-crdt (US3, FR-014)

**Goal**: The `SqlCipherKey` copy inside haex-crdt is erased on drop. It lives in another repository
and does not block Stages 1 to 6.

**Independent test**: the haex-crdt tests pass, and holzi builds and passes its suite against the
pinned commit.

- [ ] T081 [US3] In the haex-crdt repository (separate; the operator allows any available `gh`
      account, T004): change `SqlCipherKey` to wrap
      `Zeroizing<String>`, remove `Clone` or make it clone into another `Zeroizing`, add `zeroize` to
      its `Cargo.toml`, and add a test that the key type erases on drop and that `as_str()` still
      returns the key. Get it merged and note the **full 40-character commit SHA**.
- [ ] T082 [US3] In holzi's `src-tauri/Cargo.toml` bump the `haex-crdt` `rev` to that full SHA (constitution:
      immutable references), run `cargo update -p haex-crdt`, and confirm the `Cargo.lock` diff is only
      that entry. Fix compile fallout, most likely in `src-tauri/src/instances/vault_config.rs` if `Clone` was
      removed.
- [ ] T083 [US3] Run the whole suite including `src-tauri/tests/vault_upgrade.rs` and
      `src-tauri/tests/preferences_roundtrip.rs` against the new revision, and quickstart scenario 5.
- [ ] T084 [US3] **Checkpoint Stage 7**: commit `build(deps): pin haex-crdt with an erasing SqlCipherKey`.

---

## Phase 9: Polish and cross-cutting

- [ ] T085 Full CI parity from a clean state: `cargo fmt --check`, `pnpm lint:rust` (both feature
      sets, `-D warnings`), `cargo test`, `pnpm check:chat-state`, `pnpm check:vault-lifecycle`,
      `pnpm check:templates`, `pnpm typecheck`, `pnpm typecheck:scripts`, `pnpm lint`,
      `pnpm format:check`. Then `git checkout -- src/types/bindings/` unless a binding change is
      intended.
- [ ] T086 Compare `wc -l` of the six oversized files with the T003 baseline: none may have grown, and
      every new file must be under 500 lines. Record the numbers in the Baseline section.
- [ ] T087 [P] Bring `plan.md`, `research.md`, `data-model.md` and `contracts/` in line with what was
      built (signatures, the `CloseEffects` trait, outcomes of T032, T046 and T047). Run
      `pnpm exec prettier --write specs/013-vault-lifecycle-isolation` twice and confirm it is stable.
- [ ] T088 Traceability: for FR-001 to FR-026 and SC-001 to SC-010, write down the test or the recorded
      manual result that covers it in the Validation record. A gap is either closed or reported.
- [ ] T089 Run `/speckit-analyze` for a cross-artifact consistency check (the constitution requires it
      to check plans against the constitution) and fix findings.
- [ ] T090 Open the PRs in the structure below, after asking the operator (account and push). Rebase-
      merge or merge-commit, never squash. Merge-commit subjects use a Conventional Commits header.

---

## Dependencies & Execution Order

- **Setup first.** T005 and T006 are prerequisites for every frontend test (T045, T064, T076, T077).
- **Stage 1 (Phase 2)** has no dependency on the gate and may land before, or in parallel with,
  Phase 3. It is ordered first because the operator asked for the stages in that order.
- **Phase 3 (Stage 2)** blocks Stages 3, 4 and 5: they use `VaultGate`, `VaultDb` and the wrapper.
- **US1 (Stage 3)** needs Phase 3. Within it: T032 and T033 (page) and the tests T034 to T036 come
  first; T037 to T043 are backend and mostly sequential because they share `close.rs`, `lib.rs` and
  the gate; T044 and T045 (frontend) can run in parallel with T041 to T043. T046 and
  T047 need the implementation. T049 (spike retirement) needs T035 and T021 green.
- **US2 (Stage 4)** needs Stage 3's `AppState` and gate. T053 and T054 shrink `open.rs` and
  `create.rs`; T057 and T058 (docs) are independent.
- **US4 (Stage 5)** needs Stage 4 for the open-time retry (T067) but its `PublicationLock` and scan
  changes (T068, T069) and the presence lock with the gated cleanup (T070, T071) only need Phase 3
  and can start earlier.
- **Stage 6** needs T045 (sandbox).
- **Stage 7** is independent of holzi's stages; only T082 to T084 need the upstream commit.
- Tasks touching `src-tauri/src/vault_gate/mod.rs`, `src-tauri/src/instances/open.rs`, `src-tauri/src/lib.rs` or
  `scripts/check-vault-lifecycle.ts` are sequential with each other.

## Parallel Example

```bash
# Phase 3 — tests, all different files:
Task: "T018 src-tauri/src/vault_gate/gate_tests.rs"
Task: "T019 src-tauri/src/vault_gate/drain_tests.rs"
Task: "T020 src-tauri/src/vault_gate/db_tests.rs"
Task: "T021 src-tauri/tests/vault_gateway.rs"

# User Story 1 — tests, different files:
Task: "T034 src-tauri/src/chat/session_tests.rs"
Task: "T035 src-tauri/tests/vault_lifecycle_close.rs"

# User Story 4 — tests, different files:
Task: "T061 src-tauri/src/instances/lock_retry_tests.rs"
Task: "T062 src-tauri/src/models/paths_tests.rs"
Task: "T063 src-tauri/src/models/commands_tests.rs and import tests"
Task: "T065 src-tauri/src/instances/presence_tests.rs"
```

## Implementation Strategy

1. **MVP = Phase 1 + Phase 3 + User Story 1.** After T050 a close cannot fail, cancels the work,
   replaces the page and ends the process within seconds. Validate and demo here. Stage 1 (secret
   hygiene) can ship before it as a small, independent PR.
2. **Increment 2 = US2** (one vault per process, ADR, supersession notes).
3. **Increment 3 = US4** (several processes side by side).
4. **Increment 4 = Stage 6** (frontend lifetime tests, overlapping reads), then **Stage 7** when the
   upstream change is merged.
5. **PR structure** (about 1000 net lines is a prompt to split), stacked and merged in order with
   rebase-merge or a merge-commit, never squash:
   - PR A: spec, plan and tasks, plus the fix commit `fedcfa8` (operator decision, T004).
   - PR B: T005 to T006 (harness extraction) and Stage 1, T007 to T015.
   - PR C: Stage 2, T016 to T031 (behavior-preserving foundation).
   - PR D: User Story 1, T032 to T050 (MVP; one PR because the frontend lock flow and the backend
     close must ship together).
   - PR E: User Story 2, T051 to T060.
   - PR F: User Story 4, T061 to T075.
   - PR G: Stage 6, T076 to T080.
   - PR H: Stage 7, T081 to T084, after the upstream merge.
6. One commit per checkpoint (T015, T031, T050, T060, T075, T080, T084), Conventional Commits, no
   agent trailers. Each checkpoint compiles and passes its own commands.

## Baseline

_Filled in during T003, T004, T013, T036, T056 and T086._

- Test counts and command results: (pending)
- Line counts of the oversized files before and after: (pending)
- Fix commit routing and account decision (2026-09-21): `fedcfa8` goes into PR A with the docs; any
  available `gh` account may be used, restoring the previously active one afterwards.
- Passphrase leak review: (pending)
- FR-008 finding: (pending)
- Frontend open-path finding: (pending)

## Validation record

_Filled in by T048, T059, T074, T079, T083 and T088._
