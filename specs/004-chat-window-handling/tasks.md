---

description: "Actionable, dependency-ordered task list for chat window and session handling"

---

# Tasks: Chatfenster und Session-Handling

**Input**: Design documents from `specs/004-chat-window-handling/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-commands.md, quickstart.md
**Tests**: Backend regression tests are required for load lifecycle and race conditions; frontend verification follows the repository's existing manual quickstart convention.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel when files and dependencies do not overlap
- **[Story]**: User story label, required for story-phase tasks
- Every task names the concrete file or files it changes.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish the shared frontend contract and feature-local UI surface.

- [ ] T001 [P] Add `ModelLoadStatusPayload`, `ModelLoadErrorEvent`, and related load-phase types to `src/composables/useChat.ts` according to `specs/004-chat-window-handling/contracts/tauri-commands.md`.
- [ ] T002 [P] Add the new composer, model-load, and reasoning i18n keys to `src/i18n/locales/de.json` and `src/i18n/locales/en.json` with identical key trees.
- [ ] T003 [P] Create the feature-local component files `src/components/chat/ReasoningAccordion.vue` and `src/components/chat/ComposerControl.vue` with typed props/events and no hardcoded user-facing strings.

**Checkpoint**: Shared types, locale keys, and component boundaries exist; no runtime behavior has changed.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Make model preloading observable, cancellable, and safe across Vault/model races.

**⚠️ CRITICAL**: No user story implementation may begin until this phase is complete.

- [ ] T004 Add ephemeral `ModelLoadStatus`, monotonically increasing `load_id`, and a cancellable preload handle to `src-tauri/src/chat/session.rs`; keep all new state outside SQLite.
- [ ] T005 Refactor `src-tauri/src/chat/commands.rs` so manual `load_model` and the internal preload share one model-load implementation, publish `model-load-progress` with `loadId`, and publish structured `model-load-error` on failure.
- [ ] T006 Add the read-only `model_load_status` Tauri command to `src-tauri/src/chat/commands.rs` and register it in `src-tauri/src/lib.rs`.
- [ ] T007 Add `modelLoadStatusAsync`, `onModelLoadProgress`, and `onModelLoadError` wrappers to `src/composables/useChat.ts`; ignore stale events by `loadId`.
- [ ] T008 Implement `start_default_model_preload` and `cancel_preload_and_wait` in `src-tauri/src/chat/commands.rs` or the shared chat runtime module, using the existing resolver from Spec 002 and ensuring a preload does not hold `ChatState.operation` long enough to block Vault switching.
- [ ] T009 Wire the preload start after active-instance publication in `src-tauri/src/instances/create.rs` and `src-tauri/src/instances/open.rs`; reset/invalidate the previous model session before publishing the new Vault.
- [ ] T010 Cancel and await any running preload before close or manual model replacement in `src-tauri/src/instances/close.rs` and `src-tauri/src/chat/commands.rs`.
- [ ] T011 [P] Add unit coverage for `load_id` monotonicity, stale-result rejection, and status transitions in `src-tauri/src/chat/session_tests.rs`; register the test module in `src-tauri/src/chat/mod.rs`.
- [ ] T012 Add lifecycle integration coverage in `src-tauri/tests/chat_model_preload.rs` for preload after Vault publication, no duplicate load when the Chat mounts, and cancellation during a Vault switch.

**Checkpoint**: A background preload can run, be observed by a later-mounted page, and be cancelled without publishing stale state or blocking Vault lifecycle operations.

---

## Phase 3: User Story 1 - Jeder Chat-Einstieg beginnt mit einer neuen Unterhaltung (Priority: P1) 🎯 MVP

**Goal**: Every entry into the Chat starts with a clean transient conversation while preserving explicit access to history.

**Independent Test**: Send a message, leave and reopen Chat, verify that no prior thread is selected, send again, and verify the second message belongs to a new thread while the first remains in history.

### Implementation for User Story 1

- [ ] T013 [US1] Change `refreshThreads` in `src/pages/chat/[instance].vue` so loading the thread list never automatically selects the first persisted thread.
- [ ] T014 [US1] Initialize a transient new-chat draft in `src/pages/chat/[instance].vue` on every Chat entry and reset `activeThreadId`, input, pending stream state, and `expandedReasoning` for the draft.
- [ ] T015 [US1] Preserve explicit history selection in `src/pages/chat/[instance].vue`, including message loading, pending approval routing, and returning to the transient draft through the existing New Chat action.
- [ ] T016 [US1] Ensure the send path in `src/pages/chat/[instance].vue` sends `threadId: null` for the draft and refreshes history only after the backend returns the newly created thread.
- [ ] T017 [US1] Add a backend regression test in `src-tauri/tests/chat_message_idempotency.rs` or a new `src-tauri/tests/chat_new_thread.rs` proving that `threadId: null` creates a new thread and never appends to the most recent thread.
- [ ] T018 [US1] Document and execute the new-entry and abandoned-draft scenarios in `specs/004-chat-window-handling/quickstart.md` without leaving empty persisted threads.

**Checkpoint**: Opening Chat always presents a new conversation; explicit history navigation and existing message persistence still work.

---

## Phase 4: User Story 2 - Das Modell wird bereits beim Vault-Open vorbereitet (Priority: P1)

**Goal**: The selected local default model is already loading or ready when the user opens Chat; provider models are not proactively connected.

**Independent Test**: Open a Vault with a local model, stay in Workspace, then open Chat and verify that the same preload is reused without a second model load.

### Implementation for User Story 2

- [ ] T019 [US2] Update `src/pages/workspace/[instance].vue` to subscribe to and display a subtle localized preload status without making Workspace navigation dependent on model readiness.
- [ ] T020 [US2] Replace Chat-page-only model resolution in `src/pages/chat/[instance].vue` with listener registration plus `modelLoadStatusAsync`; reuse the global ready session and show loading/error states until the current status is settled.
- [ ] T021 [US2] Guard manual model selection in `src/pages/chat/[instance].vue` so selecting another model invalidates the current preload and cannot be overwritten by a stale completion.
- [ ] T022 [US2] Add structured preload error localization and retry/model-selection affordances in `src/pages/chat/[instance].vue` and `src/i18n/locales/de.json`.
- [ ] T023 [US2] Add test coverage in `src-tauri/tests/chat_model_preload.rs` for no candidate, local model ready, preload failure, and a second Vault replacing the first Vault's load.
- [ ] T024 [US2] Execute the Vault-open and model-preload scenarios in `specs/004-chat-window-handling/quickstart.md` on the normal build and a local-model build.

**Checkpoint**: Model startup begins at Vault-open, is visible to Workspace/Chat, and remains correct across failures and Vault switches.

---

## Phase 5: User Story 3 - Kompakte Chat-Konfiguration direkt im Composer (Priority: P1)

**Goal**: Model, Effort, and Permission controls are compact, accessible, and integrated into one Composer; Reasoning is automatic when supported by the selected model.

**Independent Test**: Inspect the Composer on desktop and narrow widths; open each control, change a value, and verify keyboard and screenreader access.

### Implementation for User Story 3

- [ ] T025 [P] [US3] Implement the compact trigger/popover or select behavior in `src/components/chat/ComposerControl.vue`, including current-value display, focus handling, and accessible naming.
- [ ] T026 [US3] Move the model picker from the separate settings row into the unified Composer container in `src/pages/chat/[instance].vue` without changing model-selection semantics.
- [ ] T027 [US3] Remove the Reasoning mode control/state from `src/pages/chat/[instance].vue` and keep Reasoning automatic for models that support it; preserve the existing Effort control values and disabled states.
- [ ] T028 [US3] Integrate `PermissionPrompt` into the compact Composer control area in `src/pages/chat/[instance].vue` without changing Spec 003 approval behavior.
- [ ] T029 [US3] Add responsive wrapping/popover behavior and keyboard focus styles in `src/components/chat/ComposerControl.vue` and `src/pages/chat/[instance].vue` so controls remain usable in narrow viewports.
- [ ] T030 [US3] Verify all new Composer labels and option text in `src/i18n/locales/de.json` and `src/i18n/locales/en.json`, excluding a Reasoning-mode label and including accessible labels and current-value announcements.
- [ ] T031 [US3] Execute the Composer desktop, narrow-viewport, keyboard, and unchanged-value scenarios in `specs/004-chat-window-handling/quickstart.md`.

**Checkpoint**: Composer configuration is compact and integrated; model, Effort, and Permission semantics remain unchanged, and supported models use Reasoning automatically.

---

## Phase 6: User Story 4 - Das Eingabefeld wächst mit mehrzeiligem Text (Priority: P1)

**Goal**: The prompt textarea grows with its content up to a bounded maximum and then scrolls internally.

**Independent Test**: Enter one line, several lines, and a long prompt; verify growth, bounded height, internal scrolling, send behavior, and reset.

### Implementation for User Story 4

- [ ] T032 [P] [US4] Create `src/composables/useAutoResizeTextarea.ts` with a typed resize handler that resets height, reads `scrollHeight`, clamps to min/max height, and exposes the overflow state.
- [ ] T033 [US4] Replace fixed `rows="3"` sizing and `resize-none` behavior in `src/pages/chat/[instance].vue` with `useAutoResizeTextarea`, a one-line minimum, and the planned bounded maximum height.
- [ ] T034 [US4] Preserve `Enter` to send and `Shift+Enter` for newline in `src/pages/chat/[instance].vue`, including disabled/loading/streaming states and focus retention after resize.
- [ ] T035 [US4] Reset the textarea height after successful send and after starting a new Chat draft in `src/pages/chat/[instance].vue`.
- [ ] T036 [US4] Execute the multiline, maximum-height, internal-scroll, send-reset, and narrow-viewport scenarios in `specs/004-chat-window-handling/quickstart.md`.

**Checkpoint**: Long prompts remain readable and fully editable without making the Composer or viewport unusable.

---

## Phase 7: User Story 5 - Reasoning ist nachvollziehbar, aber standardmäßig verborgen (Priority: P1)

**Goal**: Non-empty Reasoning is visible per Assistant response through an independently controllable, collapsed accordion.

**Independent Test**: Generate two responses with Reasoning, verify both accordions start closed, open only one, and verify live updates and unchanged normal answer text.

### Implementation for User Story 5

- [ ] T037 [US5] Implement native disclosure semantics in `src/components/chat/ReasoningAccordion.vue` with `aria-expanded`, keyboard operation, localized label, and a bounded scrollable content region.
- [ ] T038 [US5] Integrate `ReasoningAccordion` into `src/pages/chat/[instance].vue` for non-empty reasoning only; keep the normal Assistant answer fully visible when collapsed.
- [ ] T039 [US5] Remove the old `auto`/`on`/`off` display branching in `src/pages/chat/[instance].vue`; render the accordion solely when non-empty Reasoning deltas exist and never auto-expand it.
- [ ] T040 [US5] Preserve live `TokenEvent.reasoning` updates while an accordion is open and clear per-message expansion state on new Chat entry/reload in `src/pages/chat/[instance].vue`.
- [ ] T041 [US5] Confirm that `src/composables/useChat.ts` and `src/pages/chat/[instance].vue` do not add Reasoning persistence to `chat_messages` or alter Spec 003 stream/retry behavior.
- [ ] T042 [US5] Execute the Reasoning accordion, independent expansion, streaming update, keyboard, and reload scenarios in `specs/004-chat-window-handling/quickstart.md`.

**Checkpoint**: Every available Reasoning block is discoverable but collapsed by default, independently expandable, and non-invasive to the main response.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Validate the complete feature and reconcile the owning documentation.

- [ ] T043 [P] Update `specs/002-onboarding-model-prefs/spec.md` with an explicit cross-reference that Spec 004 supersedes its chat-entry, Composer, and Reasoning-visibility assumptions while preserving its model fallback contract.
- [ ] T044 [P] Run an automated `de`/`en` locale-key parity check against all new keys referenced by `src/pages/chat/[instance].vue`, `src/components/chat/ComposerControl.vue`, `src/components/chat/ReasoningAccordion.vue`, and `src/pages/workspace/[instance].vue`.
- [ ] T045 Run `cargo test --lib` and the focused preload/new-thread integration suites from `src-tauri/tests/`.
- [ ] T046 Run `pnpm typecheck` and fix any binding/type regressions in `src/composables/useChat.ts` and the Chat pages.
- [ ] T047 Execute the complete `specs/004-chat-window-handling/quickstart.md` on desktop and a narrow viewport; record any deviations in the checklist at `specs/004-chat-window-handling/checklists/requirements.md`.
- [ ] T048 Update `specs/004-chat-window-handling/checklists/requirements.md` with the completed validation state and any explicitly deferred findings.

---

## Dependencies & Execution Order

### Phase Dependencies

- Setup (Phase 1) has no implementation dependency and can run immediately.
- Foundational (Phase 2) depends on Setup and blocks all user stories.
- US1 depends on the shared frontend types and can begin after the foundation.
- US2 depends on the foundation and is independent of the UI-only portions of US1, but both touch `src/pages/chat/[instance].vue` and should be merged sequentially there.
- US3, US4, and US5 depend on the shared Composer/page structure; execute sequentially when editing the same page, or split the page refactor into a coordinated change.
- Polish (Phase 8) depends on all selected stories.

### User Story Dependencies

- **US1**: Foundation only; no dependency on another user story.
- **US2**: Foundation only for backend preload; integrates with the Chat page from US1.
- **US3**: Foundation plus the existing Chat page; independent behaviorally, but shares the Composer file with US1/US2.
- **US4**: Composer structure from US3.
- **US5**: Message rendering from US1 and Composer/page state from US3; Reasoning event transport already exists in the foundation.

### Parallel Opportunities

- T001, T002, and T003 can run in parallel.
- T011 can run in parallel with T005-T010 after the runtime types are agreed.
- T025, T032, and T037 can be prepared in parallel because they use separate component/composable files; their page integrations must be sequenced.
- T043 and T044 can run in parallel with each other after implementation.

## Parallel Example: Shared Foundation

```text
Task: T001 Add load-status types in src/composables/useChat.ts
Task: T002 Add de/en i18n keys in src/i18n/locales/de.json and src/i18n/locales/en.json
Task: T003 Create chat component boundaries in src/components/chat/
```

## Parallel Example: UI Components

```text
Task: T025 Implement ComposerControl.vue
Task: T032 Create useAutoResizeTextarea.ts
Task: T037 Implement ReasoningAccordion.vue
```

## Implementation Strategy

### MVP First

1. Complete Setup and Foundation.
2. Complete US1 so every Chat entry starts cleanly and history remains usable.
3. Complete US2 so local model startup begins with Vault-open.
4. Validate US1 + US2 independently before the visual Composer work.

### Incremental Delivery

1. Add US3 for the integrated Composer controls.
2. Add US4 for textarea auto-growth.
3. Add US5 for collapsed Reasoning visibility.
4. Run the full Polish phase and update the requirements checklist.

### Notes

- `[P]` is used only where tasks target distinct files or independent setup work.
- The existing repository has no frontend test runner; frontend acceptance is therefore documented manual verification, while backend lifecycle/race behavior receives Rust tests.
- No task changes the spaex or Spec Kit instruction layers.
