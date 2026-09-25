---
description: 'Task list for spec 022-session-restore'
---

# Tasks: Sitzung wiederherstellen (wählbar)

**Input**: Design documents from `/specs/022-session-restore/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/wm-session.md](./contracts/wm-session.md), [quickstart.md](./quickstart.md)

**Tests**: Included. The constitution requires an executable check for non-trivial logic: Rust tests next to the new modules (`*_tests.rs`, declared via `#[path]` like the existing storage tests) and `scripts/check-wm-persistence.ts` in `pnpm check:wm-state`. Write each test task before its implementation task and see it fail first.

**Organization**: Grouped by user story. The branch stacks on `020-tab-navigation` (after the `wm` rename, `b4abfb6`). Modules under `src/lib/**` import siblings relatively with a `.ts` suffix. Every file stays ≤ 500 lines. Commits follow Conventional Commits and carry no agent attribution. Rust commands go through `nix develop --command scripts/with-nix-host-bridge.sh …`; after `cargo test`, run `git checkout -- src/types/bindings/` for bindings that only gained trailing whitespace, and keep the ones that really changed (strip trailing whitespace with the `generate:ts-types` `sed`).

**Migration rule**: Migration `0020_wm_session_no_sync` is written once, complete, in T006. Never edit it afterwards: a vault that already ran it (for example the operator's test vault) would refuse to open with `MigrationContentDrift`. A later fix needs a new migration `0021_…`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: User story from spec.md (US1–US5)

---

## Phase 1: Setup

- [ ] T001 Prepare `.worktrees/022-session-restore`: the real `pnpm install` is done; reflink the Rust build cache from the 020 worktree (`cp -a --reflink=always ../020-tab-navigation/src-tauri/target src-tauri/target`, only if `Cargo.lock` matches), then record the baseline counts of `pnpm check:wm-state`, `check:wm-navigation`, `check:chat-state` and `cargo test` in the T001 note
- [ ] T002 [P] Add the 022 row to `plans/README.md` after the 020 row: priority P1, effort S, status "Spezifiziert 2026-09-25, Umsetzung ab 2026-09-26", gate "setzt Spec 015 voraus; PR stapelt auf 020 (#142)"

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: New storage, commands and store plumbing. With the default "off", US1 works at the end of this phase.

**⚠️ CRITICAL**: No user-story work starts before this phase is complete

### Tests first

- [ ] T003 [P] Add `get_scoped_bool` tests to `src-tauri/src/storage/preferences_tests.rs`: device `'true'` over vault `'false'`; device unset → vault; both unset → `None` for each and effective `false`; a value other than `'true'`/`'false'` counts as unset; returns `(device: Option<bool>, vault: Option<bool>)`
- [ ] T004 [P] Create `src-tauri/src/storage/wm_session_tests.rs` (storage level, real `Database` like `wm_windows_tests.rs`): `save` inserts then overwrites the device's single row; `load` returns the parsed JSON or `None`; `delete` removes only the own device's row, another device's row stays; `save` rejects non-object JSON and a serialized size above 4 MiB with `InvalidInput`
- [ ] T005 [P] Add migration tests to `src-tauri/src/identity/migrations_tests.rs` using `migration_source_before("0020_wm_session_no_sync")`: (a) fresh vault: `workspaces`, `shell_windows`, `shell_window_tabs` absent, `wm_sessions_no_sync` and `holzi_maintenance_no_sync` present, the maintenance row `vacuum_after_legacy_wm_drop` present; (b) upgraded vault: open at 0019, insert a workspace, window and tab through the old SQL, reopen with the full set: old tables gone, no `haex_deleted_rows` entries for them were added by the upgrade, open succeeds; replace the two 0019 FK-cascade spike tests, which test tables that no longer exist

### Implementation

- [ ] T006 Add migration `0020_wm_session_no_sync` to `src-tauri/src/identity/migrations.rs` exactly per research R5 and data-model.md: `DROP TABLE shell_window_tabs;` `DROP TABLE shell_windows;` `DROP TABLE workspaces;` (children first, no `DELETE`), `CREATE TABLE wm_sessions_no_sync (vault_device_uuid TEXT PRIMARY KEY NOT NULL, session_json TEXT NOT NULL, updated_at TEXT NOT NULL)`, `CREATE TABLE holzi_maintenance_no_sync (task TEXT PRIMARY KEY NOT NULL)`, `INSERT INTO holzi_maintenance_no_sync (task) VALUES ('vacuum_after_legacy_wm_drop')`, statements separated by `--> statement-breakpoint`; comment why there is no `HOLZI_TRIGGER_VERSION` bump. Make T005 pass. **Never edit this migration after this task.**
- [ ] T007 [P] Implement `get_scoped_bool(conn, device, key)` in `src-tauri/src/storage/preferences.rs` on top of the existing `get`; make T003 pass
- [ ] T008 Implement `src-tauri/src/storage/wm_session.rs` (`load`, `save` with `INSERT … ON CONFLICT(vault_device_uuid) DO UPDATE`, `delete`, `MAX_SESSION_BYTES = 4 << 20`), with a `ponytail:` comment on the missing FK to `known_devices` (research R1); make T004 pass
- [ ] T009 Create `src-tauri/src/storage/wm_session_commands_tests.rs` (command core level, `open_test_state` like the former `wm_commands_tests.rs`): `restore_get` reports device/vault/effective; `restore_set` device `true` → effective; `restore_set(null)` resets; turning effective off deletes the own session in the same call; `load` with effective off deletes an existing own session and returns `session: None`; `load` with effective on returns the saved session; `save` with effective off returns `saved: false` and writes nothing
- [ ] T010 Implement `src-tauri/src/storage/wm_session_commands.rs`: the four commands and ts-rs wire types `SessionRestoreState { device, vault, effective }`, `WmSessionLoad { restore, session }`, `WmSessionSaved { saved }` per contracts §1; move `current_device_uuid` here from `wm_commands.rs`; `restore_set` writes the preference and deletes the own row in one transaction when effective ends up `false`; make T009 pass
- [ ] T011 Remove `src-tauri/src/storage/wm_commands.rs`, `wm_windows.rs`, `wm_workspaces.rs` and their `*_tests.rs`; update `src-tauri/src/storage/mod.rs`; in `src-tauri/src/lib.rs` replace the six `wm_*` registrations with `wm_session_restore_get`, `wm_session_restore_set`, `wm_session_load`, `wm_session_save`; delete the stale bindings `src/types/bindings/{WmLayoutDto,WindowDto,TabDto,WorkspaceDto,DeleteWorkspaceResult}.ts` and generate the new ones; `cargo fmt`, `pnpm lint:rust` and `cargo test` green
- [ ] T012 [P] Rewrite `scripts/check-wm-persistence.ts` for `useWmSession` (fake `invokeFn`): debounce coalesces to one `wm_session_save` with the latest snapshot; `saveNow` is immediate; a failed save stays dirty and retries; no two calls run at once; `flushAsync` waits for the queue; `load` passes through `wm_session_load`; a save rejected as too large (`InvalidInput`) is retried once with every tab's history reduced to its current entry, and not retried again after that
- [ ] T013 Rename `src/composables/useWmLayout.ts` to `src/composables/useWmSession.ts` and reduce it to `saveNow(snapshot)`, `saveSoon(snapshot)`, `load()`, `setRestore(scope, enabled)`, `flushAsync()` over the four commands; keep the serialized queue and `SAVE_DEBOUNCE_MS = 400`; add the one-time size fallback (data-model.md "Größe"); make T012 pass
- [ ] T014 Replace `PersistedLayout` with `WmSession` (`version: 1`, data-model.md) in `src/lib/wm/types.ts` and `src/lib/wm/layoutState.ts` (`hydrate`); every tab carries `history: TabHistory` (spec 020, `src/lib/wm/navigation.ts`); add `parseWmSession(unknown): WmSession | null` in `src/lib/wm/session.ts` (wrong version, missing or mistyped fields → `null`; a single invalid tab history — empty, more than 50 entries, `index` out of range, a path without leading `/` — is replaced by `createHistory()` instead of rejecting the session) with tests in `scripts/check-wm-state.ts`; update `scripts/lib/wm-fixtures.ts`
- [ ] T015 Refactor `src/stores/windowManager.ts` per contracts §4: `sessionRestore` ref (default `false`); `restoreSessionAsync()` replaces `hydrateFromBackendAsync` (`load` → `parseWmSession` → `hydrate`, then seed the history map in `src/stores/wmNavigation.ts` from each surviving tab's saved history instead of clearing it; else the empty start `hydrate({ workspaces: [], windows: [], activeWorkspaceId: '' })`); the snapshot reads each tab's history from that map, and `navigate`, `goTab` and `skipCurrent` also call `saveSessionSoon()`; `persistWindowNow`/`persistWindowDebounced`/`closeWindowNow` call sites become `saveSessionNow()`/`saveSessionSoon()` that snapshot the whole state and return early while `sessionRestore` is `false`; `createWorkspace` becomes synchronous with `crypto.randomUUID()`; `switchWorkspace`/`deleteWorkspace` save the snapshot instead of calling the removed commands; `applySessionRestore(state)` sets `sessionRestore` and saves immediately when it turns `true`; keep `flushAsync`; stay ≤ 500 lines. Adjust callers in `src/stores/wmLayoutHandlers.ts` (`wm.workspace.create` no longer awaits a backend id) and `src/composables/useWmTab.ts`
- [ ] T016 Run `pnpm typecheck`, `lint`, `format:check`, `check:wm-state`, `check:wm-navigation`, `check:chat-state`, `check:templates`, `check:vault-lifecycle`; fix fallout (for example `scripts/check-vault-lifecycle.ts` stubs of the store)

**Checkpoint**: The app opens, starts with one empty workspace, and never writes a session (default off).

---

## Phase 3: User Story 1 - Standardmäßig beginnt jeder Start leer (P1) 🎯 MVP

**Goal**: Without the setting, every vault session starts empty and nothing is saved.

**Independent Test**: quickstart S1, S2, S10.

- [ ] T017 [US1] Add store-level tests to `scripts/check-wm-persistence.ts` (or a new `scripts/check-wm-session-store.ts` added to `check:wm-state` if the file would pass 500 lines): with `sessionRestore = false`, opening apps, moving windows, creating and deleting workspaces issue no `wm_session_save`; `restoreSessionAsync` with `session: null` yields exactly one workspace and no windows; a rejected `wm_session_load` yields the same empty start
- [ ] T018 [US1] In `src/pages/workspace/[instance].vue`, call `restoreSessionAsync()` inside `try/catch` (log with `console.error`, keep the empty start) before handling `?open=`/`&at=`, so a deep link works even when loading fails (FR-012, FR-014); update the page's header comment
- [ ] T019 [US1] Manual: quickstart S1, S2, S10; record results in the T019 note

---

## Phase 4: User Story 2 - Wiederherstellung einschalten (P1)

**Goal**: The user turns saving on for this device in the settings; it takes effect at once and the next start restores the session.

**Independent Test**: quickstart S3, S4, S11, S12.

- [ ] T020 [P] [US2] Add the actions `settings.sessionRestore.set` (`{ scope: 'device' | 'vault', enabled: boolean }`) and `settings.sessionRestore.clear` (`{ scope }`) to `src/lib/actions/settingsActions.ts` (scope `settings.device`, effect `write`, result schema of `SessionRestoreState`); the catalog invariants in `scripts/check-wm-actions.ts` must stay green
- [ ] T021 [US2] Implement both handlers in `src/stores/settingsActionHandlers.ts`: call `useWmSession().setRestore`, then `useWindowManagerStore().applySessionRestore(state)`, return the state; add `sessionRestore` to `settings.get` via `wm_session_restore_get`
- [ ] T022 [P] [US2] Add i18n keys to `src/i18n/locales/de.json` and `en.json`: `settings.sessionRestore.{title,description,device,vault,effective,on,off,unset,scope.device,scope.vault,enable,reset}` and `actions.settings.sessionRestore.{set,clear}`; German text uses "Sitzung wiederherstellen" and explains that open workspaces, windows and tabs are saved on this device only
- [ ] T023 [US2] Create `src/components/settings/SessionRestoreSetting.vue` per contracts §5 (device, vault and effective values; scope choice like `DefaultModelSetting.vue`; switch; reset disabled when the chosen scope is unset; all writes via `useActionOrThrow`); mount it in `src/components/apps/SettingsApp.vue` after `SettingsAliasSetting` with an `<hr>`
- [ ] T024 [US2] Add store tests: `applySessionRestore({ effective: true })` saves the current snapshot immediately; afterwards changes save again (debounced or immediate as before); navigating back and forth in a tab schedules a save; a session restored from a snapshot gives every tab the saved history, index and titles (`historyOf(tabId)` equals the saved `TabHistory`), and back/forward work across the restart
- [ ] T025 [US2] Manual: quickstart S3, S4, S11, S12, S14; record results

---

## Phase 5: User Story 4 - Ausschalten entfernt die gespeicherte Sitzung (P1)

**Goal**: Turning saving off deletes the saved session at once; open windows stay.

**Independent Test**: quickstart S5 and P1 after S5.

- [ ] T026 [US4] Add a store test: `applySessionRestore({ effective: false })` keeps all workspaces, windows and tabs and stops saving; a debounced save that fires afterwards is a no-op in the store, and the backend's `saved: false` path is covered by T009
- [ ] T027 [US4] Manual: quickstart S5 and P1 (no own row in `wm_sessions_no_sync`); record results

---

## Phase 6: User Story 3 - Für alle Geräte oder nur für dieses (P2)

**Goal**: The vault value applies to all devices; a device value overrides it; reset falls back.

**Independent Test**: quickstart S6, S7, S8.

- [ ] T028 [US3] Extend T009's tests (in `src-tauri/src/storage/wm_session_commands_tests.rs`) with the vault scope: vault `true` + device unset → effective; vault `true` + device `false` → not effective and own session deleted; vault `false` + device `true` → effective; two devices with effective on keep separate rows; resetting the device value falls back to the vault value
- [ ] T029 [US3] Verify `SessionRestoreSetting.vue` shows the vault value and the effective value correctly after switching scope and after reset (component logic only; no new component)
- [ ] T030 [US3] Manual: quickstart S6, S7, S8; record results

---

## Phase 7: User Story 5 - Ungefragt gespeicherte Sitzungen verschwinden beim Update (P1)

**Goal**: Sessions saved by spec 015 are gone from the vault after the first open with this version, without content left in the file.

**Independent Test**: quickstart S9, S13 and P1.

- [ ] T031 [P] [US5] Create `src-tauri/src/storage/maintenance_tests.rs`: after `run_after_open`, `PRAGMA secure_delete` returns 1; preference rows with key `shell.active_workspace_id` for two devices are gone and other keys untouched; the maintenance row is gone and `PRAGMA freelist_count` is 0 after a vault with dropped tables; a second run is a no-op; a failing step (simulated) leaves the maintenance row for the next run and does not return an error to the caller
- [ ] T032 [US5] Implement `src-tauri/src/storage/maintenance.rs::run_after_open(db)` per contracts §2 (secure_delete, legacy preference delete, one-time `VACUUM` + `wal_checkpoint(TRUNCATE)`, logging each failure); make T031 pass
- [ ] T033 [US5] Call `run_after_open` in `src-tauri/src/instances/open.rs` after a successful `Database::open` and in `src-tauri/src/instances/create.rs` after creating a vault, before the vault is handed to the frontend; errors are logged, never returned
- [ ] T034 [US5] Manual: quickstart S9 (vault from `main` before the merge with a saved session), S13 and P1; record results

---

## Phase 8: Polish & Cross-Cutting

- [ ] T035 [P] Mark in `specs/015-workspace-shell/spec.md` (FR-016): at User Story 5, FR-023, FR-024, FR-025 and the restart part of User Story 7 a note "Gilt seit Spec 022 nur bei eingeschalteter Einstellung „Sitzung wiederherstellen“" with a link to `specs/022-session-restore/spec.md`; in `specs/020-tab-navigation/spec.md` at FR-011 the note "Seit Spec 022 bleiben Ort und Historie bei eingeschalteter Einstellung „Sitzung wiederherstellen“ erhalten" with the same link
- [ ] T036 [P] Add the term "Sitzung (wm session)" to `CONTEXT.md` next to the window manager entry: what it contains, that it is saved only with the setting, and that it differs from the vault session (spec 013) and the chat's active session
- [ ] T037 Run the full quickstart §1 (all automated checks incl. `cargo fmt --check`, `lint:rust`, `cargo test`) and the e2e suite (`pnpm test:e2e`); record counts
- [ ] T038 Run `/speckit-analyze` for 022 and resolve findings in the docs
- [ ] T039 Open the PR with base `020-tab-navigation` (retarget to `main` after #142 merges), as a draft until the manual scenarios are recorded

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → stories. T006 before T005 passes; T008 needs T006; T010 needs T007 and T008; T011 needs T010; T013 needs T011 (bindings); T015 needs T013 and T014.
- US1 (Phase 3) needs only Phase 2. US2 needs Phase 2. US4 needs US2 (it turns off what US2 turned on). US3 needs US2. US5 needs only T006 and can run in parallel with US1–US4.
- Polish after all stories.

## Parallel Opportunities

- T003, T004, T005 together (different Rust test files).
- T007 alongside T006; T012 and T014 alongside the Rust work.
- T020 and T022 together; T031 alongside Phase 4–6.
- T035 and T036 together.

## Implementation Strategy

1. MVP = Phase 1–3: the default "off" behavior with the old storage removed. Already satisfies the operator's main request (no restored windows by default).
2. Add US2 + US4 (turn on, turn off), then US3 (vault scope), then US5 (legacy cleanup; can be done earlier in parallel).
3. Polish, analyze, PR.
