# Tasks: Chat-Historie verwalten

**Input**: Design documents from `specs/006-chat-history-management/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-commands.md, quickstart.md
**Tests**: Backend contract and persistence tests are required by the plan; frontend behavior is covered by the existing state-check harness and manual quickstart.

**Organization**: Tasks are grouped by the three independently testable user
stories from `spec.md`. All task descriptions name the concrete files they
change or validate.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel when files and dependencies do not overlap
- **[Story]**: User story label, required for story-phase tasks
- Every task names the concrete file or files involved.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare the shared locale and regression-test surfaces.

- [x] T001 [P] Add German duration, history-action, confirmation, accessibility, and error message keys to `src/i18n/locales/de.json` according to `specs/006-chat-history-management/spec.md`.
- [x] T002 [P] Add the matching English duration, history-action, confirmation, accessibility, and error message keys to `src/i18n/locales/en.json` with the same key tree as `src/i18n/locales/de.json`.
- [x] T003 [P] Extend `scripts/check-chat-state.mjs` fixtures with complete `Thread` timestamps and deterministic rename/delete failure stubs for the history-management scenarios.

**Checkpoint**: Both locales and the existing frontend state harness can represent the new history interactions without changing runtime behavior.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Establish safe, shared persistence and command boundaries before story-specific UI work.

**⚠️ CRITICAL**: No user story implementation may begin until this phase is complete.

- [x] T004 Define the validated `RenameThreadArgs` and `DeleteThreadArgs` boundaries plus stable `InvalidInput`/`NotFound` error mapping in `src-tauri/src/chat/thread_commands.rs` and `specs/006-chat-history-management/contracts/tauri-commands.md`.
- [x] T005 Extend the existing thread storage boundary in `src-tauri/src/storage/chat_threads.rs` and `src-tauri/src/storage/chat_messages.rs` so title updates preserve `created_at` and confirmed deletion removes a thread with all related messages as one persistent action.
- [x] T006 Register the thread mutation commands in `src-tauri/src/lib.rs` and keep their public payloads aligned with `src-tauri/src/chat/thread_commands.rs` and `src/composables/useChat.ts`.
- [x] T007 Add shared backend assertions for unchanged `created_at`, no partial mutation on failure, and no orphaned messages in `src-tauri/tests/chat_thread_management.rs`.

**Checkpoint**: The backend contracts are validated, persistence invariants are explicit, and the frontend can call both mutations through registered commands.

---

## Phase 3: User Story 1 - Vergangene Thread-Dauer erkennen (Priority: P1) 🎯 MVP

**Goal**: Every visible history entry shows a compact, right-aligned duration such as `1min`, `2h`, or `5d`, based only on its opening time.

**Independent Test**: Provide threads aged under one minute, one minute, two hours, and five days; verify `0min`, `1min`, `2h`, and `5d`, boundary updates, right-edge visibility, and accessible full timestamp information.

### Tests for User Story 1

- [x] T008 [P] [US1] Add deterministic duration-format assertions for Unix-millisecond timestamps, minute/hour/day thresholds, future timestamps, and unusable timestamps falling back to `0min` in `scripts/check-chat-state.mjs` without using wall-clock sleeps.

### Implementation for User Story 1

- [x] T009 [US1] Add a pure history-duration projection in `src/pages/chat/[instance].vue` that floors elapsed time, selects `min`/`h`/`d`, and clamps negative values to `0min`.
- [x] T010 [US1] Add boundary-aware duration refresh state in `src/pages/chat/[instance].vue` so a visible entry updates at the next minute, hour, or day boundary and clears its scheduled work on unmount.
- [x] T011 [US1] Render the duration for every persisted history row in a right-aligned trailing column in `src/pages/chat/[instance].vue`, allowing the title to use the remaining width without being covered.
- [x] T012 [US1] Add localized accessible full-opening-time information and visible focus/hover semantics for the duration in `src/pages/chat/[instance].vue`, using the keys from `src/i18n/locales/de.json` and `src/i18n/locales/en.json`.

**Checkpoint**: User Story 1 is independently usable: every history row has a stable, compact duration that updates without reload and remains accessible.

---

## Phase 4: User Story 2 - Verlaufstitel umbenennen (Priority: P1)

**Goal**: A user can open an entry's Pencil/Edit action, save a valid title, or cancel editing without changing messages, duration, or active context.

**Independent Test**: Focus a history entry, save a valid title, reload, verify persistence and unchanged duration, then cancel an edit with `Escape` and reject blank/overlong titles.

### Tests for User Story 2

- [x] T013 [P] [US2] Add rename command integration tests for valid trimming, 1–120 visible-character validation, unchanged `created_at`, `InvalidInput`, `NotFound`, and persistence failure behavior in `src-tauri/tests/chat_thread_management.rs`.

### Implementation for User Story 2

- [x] T014 [US2] Implement the validated `rename_thread` command in `src-tauri/src/chat/thread_commands.rs` by reusing the existing title-update storage candidate and returning the refreshed `ThreadPayload`.
- [x] T015 [US2] Add the awaitable `renameThreadAsync` wrapper and precise `Thread` result typing to `src/composables/useChat.ts`.
- [x] T016 [US2] Add one-entry-at-a-time inline title editing, focus transfer, `Enter` save, `Escape` cancel, and trimmed-length validation to `src/pages/chat/[instance].vue`.
- [x] T017 [US2] Render the Pencil/Edit control directly left of the duration on hover and keyboard focus with localized accessible naming and localized inline errors in `src/pages/chat/[instance].vue`, `src/i18n/locales/de.json`, and `src/i18n/locales/en.json`.
- [x] T018 [US2] Refresh the history row from the successful rename result in `src/pages/chat/[instance].vue` without changing `createdAt`, loaded messages, or `activeThreadId`.

**Checkpoint**: User Story 2 is independently usable: titles can be safely renamed, persisted, cancelled, and reached by keyboard without affecting conversation data.

---

## Phase 5: User Story 3 - Verlaufseintrag löschen (Priority: P1)

**Goal**: A user can confirm deletion of a history entry and remove its complete thread history, while cancelling remains non-destructive and deleting the active thread returns to a new empty session.

**Independent Test**: Cancel a deletion and verify no changes; confirm deletion and verify the thread/messages stay absent after reload; repeat for the active thread and verify no other thread is auto-selected.

### Tests for User Story 3

- [x] T019 [P] [US3] Add delete command integration tests for confirmation-independent backend execution, exact thread targeting, atomic thread/message removal, `NotFound`, persistence failure, no orphaned messages, and abort-before-delete coordination in `src-tauri/tests/chat_thread_management.rs` and `scripts/check-chat-state.mjs`.

### Implementation for User Story 3

- [x] T020 [US3] Implement the validated `delete_thread` command in `src-tauri/src/chat/thread_commands.rs` using the atomic storage boundary from `src-tauri/src/storage/chat_threads.rs` and `src-tauri/src/storage/chat_messages.rs`.
- [x] T021 [US3] Add the awaitable `deleteThreadAsync` wrapper to `src/composables/useChat.ts` and preserve structured command errors for the page.
- [x] T022 [US3] Add the hover/focus Delete control directly left of the duration and the title-specific confirmation dialog to `src/pages/chat/[instance].vue` using localized text from `src/i18n/locales/de.json` and `src/i18n/locales/en.json`.
- [x] T023 [US3] Update `src/pages/chat/[instance].vue` only after successful deletion: remove the row and cached messages, clear the active thread when applicable, and show the new empty session without auto-selecting another thread.
- [x] T024 [US3] Handle delete failures and concurrent busy/approval/streaming states in `src/pages/chat/[instance].vue`; when deleting the active thread, abort the running turn first, await its terminal state, and leave the history visible if cancellation fails.

**Checkpoint**: User Story 3 is independently usable: confirmed deletion is complete and recoverable errors never produce a partial or misleading UI state.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Validate the complete feature and keep related Spec-Kit documentation aligned.

- [x] T025 [P] Update `specs/006-chat-history-management/checklists/requirements.md` with the final duration-format, mutation-contract, accessibility, and edge-case validation results.
- [x] T026 [P] Confirm German/English key parity for `src/i18n/locales/de.json` and `src/i18n/locales/en.json`, including duration units, actions, confirmation, errors, and accessible labels.
- [x] T027 [P] Update `specs/006-chat-history-management/quickstart.md` with any implementation-specific setup needed to create the `0min`, `1min`, `2h`, and `5d` duration cases.
- [x] T028 Run `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features` and record the result in `specs/006-chat-history-management/checklists/requirements.md`.
- [x] T029 Run the repository-toolchain equivalents of `pnpm typecheck`, `pnpm lint`, and `node scripts/check-chat-state.mjs`; record the result in `specs/006-chat-history-management/checklists/requirements.md`.
- [ ] T030 Execute the complete manual flow from `specs/006-chat-history-management/quickstart.md` on desktop and a narrow viewport, including verification that actions appear left of the duration without permanently consuming title space, then record deviations in `specs/006-chat-history-management/checklists/requirements.md`.

---

## Dependencies & Execution Order

### Phase Dependencies

- Setup (Phase 1) has no dependencies and prepares locales and the existing state harness.
- Foundational (Phase 2) depends on Setup and blocks all user stories.
- User Stories 1, 2, and 3 depend on Foundational, but are behaviorally independent from one another.
- Polish (Phase 6) depends on all selected user stories being implemented.

### User Story Dependencies

- **US1**: Can start after Phase 2; it uses existing `Thread.createdAt` and does not depend on rename/delete behavior.
- **US2**: Can start after Phase 2; it shares the history row with US1 but has an independent persistence contract.
- **US3**: Can start after Phase 2; it shares the history row with US1/US2 and must be sequenced when editing `src/pages/chat/[instance].vue`.

### Parallel Opportunities

- T001, T002, and T003 can run in parallel.
- T013 and T019 can run in parallel after the foundational persistence contract is agreed because they use separate test files.
- T025, T026, and T027 can run in parallel after feature implementation.
- US1, US2, and US3 can be assigned independently after Phase 2, but page integrations should merge sequentially to avoid conflicts in `src/pages/chat/[instance].vue`.

## Parallel Example: Setup

```text
Task: T001 Add German history-management keys in src/i18n/locales/de.json
Task: T002 Add English history-management keys in src/i18n/locales/en.json
Task: T003 Extend the deterministic chat-state fixtures in scripts/check-chat-state.mjs
```

## Parallel Example: Backend story tests

```text
Task: T013 Add rename contract tests in src-tauri/tests/chat_thread_rename.rs
Task: T019 Add delete contract tests in src-tauri/tests/chat_thread_delete.rs
```

## Implementation Strategy

### MVP First

1. Complete Phase 1 and Phase 2.
2. Complete US1 so the history immediately communicates thread age.
3. Validate US1 using its independent test and the duration section of `quickstart.md`.
4. Continue with US2 and US3 for the complete requested history management.

### Incremental Delivery

1. Deliver duration display as the smallest visible slice.
2. Add title editing without changing conversation selection or message state.
3. Add confirmed full-thread deletion and active-thread reset behavior.
4. Run the cross-cutting validation and update the checklist.

### Notes

- `[P]` is used only for tasks targeting separate files or independent setup work.
- Backend tests are written before their corresponding command implementation; frontend acceptance follows the existing state harness and manual quickstart convention.
- No task changes spaex, Spec-Kit, Constitution, or agent instruction files.
