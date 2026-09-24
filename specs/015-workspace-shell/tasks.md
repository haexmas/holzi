# Tasks: Workspace-Shell

**Input**: Design documents from `/specs/015-workspace-shell/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

## Phase 1: Setup and roadmap gate

- [ ] T001 Add the approved Shell phase and its dependency on feature 013 to `plans/README.md`
- [ ] T002 [P] Add Shell/App/Window/Tab/Launcher terminology to `CONTEXT.md`
- [ ] T003 [P] Confirm migration numbering and trigger version against the current `main` and feature-013/014 branches in `specs/015-workspace-shell/research.md`
- [ ] T004 [P] Add the `check:shell-state` script entry and CI placeholder in `package.json` and `.github/workflows/ci.yml`

## Phase 2: Foundational chat split

**Purpose**: Move behavior without changing the chat contract before embedding it in a Shell window.

- [ ] T005 Capture the current replay-test count and baseline output of `pnpm check:chat-state` in `specs/015-workspace-shell/quickstart.md`
- [ ] T006 Extract composer send/cancel/new-conversation logic into `src/composables/useComposer.ts` without changing behavior
- [ ] T007 Run `pnpm check:chat-state` and preserve the baseline replay-test count after T006
- [ ] T008 Extract attachment handling into `src/composables/useComposerAttachments.ts` without changing behavior
- [ ] T009 Run `pnpm check:chat-state` and preserve the baseline replay-test count after T008
- [ ] T010 [P] Extract the thread sidebar into `src/components/chat/ThreadSidebar.vue` with props and emits only
- [ ] T011 [P] Extract transcript, reasoning, tool rows, and audit-marker translation into `src/components/chat/MessageList.vue`
- [ ] T012 [P] Extract the composer view into `src/components/chat/Composer.vue` using existing composer controls
- [ ] T013 Integrate the extracted chat components into `src/pages/chat/[instance].vue` and keep the orchestrator under 500 lines
- [ ] T014 Run `pnpm check:chat-state`, `pnpm typecheck`, and `pnpm lint` after the split

## Phase 3: User Story 1 — Apps as windows (Priority: P1)

**Independent test**: Open a completed vault, launch Chat and Settings from the Launcher, use both, and reach them through the legacy routes.

- [ ] T015 [P] [US1] Define Shell entities and app identifiers in `src/lib/shell/types.ts` and `src/lib/shell/apps.ts`
- [ ] T016 [P] [US1] Implement pure app/window/workspace hydration and opening reducers in `src/lib/shell/layoutState.ts`
- [ ] T017 [P] [US1] Implement app component mapping with async components in `src/components/shell/appComponents.ts`
- [ ] T018 [US1] Implement the Pinia Shell store and public actions in `src/stores/shell.ts`
- [ ] T019 [P] [US1] Add the Shell tab contract (`useShellTab`) in `src/composables/useShellTab.ts`
- [ ] T020 [P] [US1] Move preload listeners and status state into `src/composables/useModelPreloadStatus.ts`
- [ ] T021 [US1] Move the split chat page to `src/components/apps/ChatApp.vue`, preserving route-independent behavior
- [ ] T022 [P] [US1] Move settings and federation pages to `src/components/apps/SettingsApp.vue` and `src/components/apps/FederationApp.vue`
- [ ] T023 [P] [US1] Add Shell host and Launcher components in `src/components/shell/ShellDesktop.vue`, `src/components/shell/ShellLauncher.vue`, and `src/pages/workspace/[instance].vue`
- [ ] T024 [US1] Add legacy route redirects in `src/pages/chat/[instance].vue`, `src/pages/settings/[instance].vue`, and `src/pages/federation/[instance].vue`
- [ ] T025 [US1] Add Shell status bar and preserve onboarding/instance close behavior in `src/components/shell/ShellStatusBar.vue` and `src/pages/workspace/[instance].vue`
- [ ] T026 [US1] Add initial Shell translations and remove obsolete workspace Stub keys only after `rg` confirms no remaining usage in `i18n/locales/de.json` and `i18n/locales/en.json`

## Phase 4: User Story 2 — Window management (Priority: P1)

**Independent test**: Move, resize, minimize, restore, maximize, and close two windows while a Chat draft and response remain alive.

- [ ] T027 [P] [US2] Implement geometry clamping, cascade placement, minimum sizes, compact correction, and maximize/restore invariants in `src/lib/shell/geometry.ts`
- [ ] T028 [P] [US2] Implement pointer-capture move and eight-way resize gestures in `src/composables/useWindowPointerGesture.ts`
- [ ] T029 [US2] Add Shell window frame, title bar, focus, minimize, maximize, restore, and close controls in `src/components/shell/ShellWindow.vue` and `src/components/shell/ShellWindowControls.vue`
- [ ] T030 [US2] Add window overview with minimized-window restore, close, active-tab title, and attention badges in `src/components/shell/ShellWindowOverview.vue`
- [ ] T031 [US2] Keep visited app tabs mounted with `v-show`, wire close guards and attention state into `src/components/shell/ShellWindow.vue` and `src/composables/useShellTab.ts`
- [ ] T032 [US2] Add Chat close guard and permission attention lifecycle in `src/components/apps/ChatApp.vue`

## Phase 5: User Story 3 — Tabs in windows (Priority: P2)

**Independent test**: Add Settings to a Chat window, switch through the Chevron, close the Settings tab, and preserve the Chat draft.

- [ ] T033 [P] [US3] Implement tab insertion, singleton resolution, activation, neighbor close, and last-tab window close in `src/lib/shell/tabs.ts`
- [ ] T034 [P] [US3] Add the ARIA tab-list and Firefox-style tab bar in `src/components/shell/ShellTabBar.vue`
- [ ] T035 [P] [US3] Add the `+` app menu and singleton-open state in `src/components/shell/ShellNewTabMenu.vue`
- [ ] T036 [P] [US3] Add the Chevron tab-list menu with active and attention states in `src/components/shell/ShellTabListMenu.vue`
- [ ] T037 [US3] Add compact-mode title/Chevron behavior and keyboard navigation in `src/components/shell/ShellWindow.vue` and `src/components/shell/ShellTabBar.vue`
- [ ] T038 [US3] Add tab close confirmation aggregation for all guards in `src/components/shell/ShellCloseConfirm.vue`

## Phase 6: User Story 4 — Multiple workspaces (Priority: P2)

**Independent test**: Create, switch, move a window between, and delete a workspace; verify dense numbering and last-workspace protection.

- [ ] T039 [P] [US4] Add workspace creation, switching, deletion, dense-position handling, and attention derivation to `src/lib/shell/layoutState.ts` and `src/stores/shell.ts`
- [ ] T040 [P] [US4] Add workspace overview, move-window menu, and delete confirmation in `src/components/shell/ShellWorkspaceOverview.vue` and `src/components/shell/ShellCloseConfirm.vue`
- [ ] T041 [US4] Add workspace attention indicators to `src/components/shell/ShellLauncher.vue`, `src/components/shell/ShellWorkspaceOverview.vue`, `src/components/shell/ShellWindowOverview.vue`, and `src/components/shell/ShellTabListMenu.vue`

## Phase 7: User Story 5 — Persisted layout (Priority: P2)

**Independent test**: Persist two workspaces, windows, tabs, active tab, geometry, and maximize state; close and reopen the vault on the same and another device.

- [ ] T042 [P] [US5] Add migration `0019_shell_layout`, trigger version update, and device-scoped tables in `src-tauri/src/identity/migrations.rs`
- [ ] T043 [P] [US5] Implement workspace storage, ordering, deletion, and device isolation in `src-tauri/src/storage/shell_workspaces.rs` and `src-tauri/src/storage/shell_workspaces_tests.rs`
- [ ] T044 [P] [US5] Implement window/tab batch replacement and close operations in `src-tauri/src/storage/shell_windows.rs` and `src-tauri/src/storage/shell_windows_tests.rs`
- [ ] T045 [US5] Implement DTOs, validation, six Tauri commands, and active-device lookup in `src-tauri/src/storage/shell_commands.rs` and `src-tauri/src/storage/shell_commands_tests.rs`
- [ ] T046 [US5] Register Shell commands and storage modules in `src-tauri/src/lib.rs` and `src-tauri/src/storage/mod.rs`
- [ ] T047 [US5] Implement the serialized invoke queue, 400 ms geometry debounce, dirty retry, and `flushAsync` ordering in `src/composables/useShellLayout.ts`
- [ ] T048 [US5] Integrate hydration, persistence, unknown-app filtering, geometry correction, and flush-before-close in `src/stores/shell.ts` and `src/pages/workspace/[instance].vue`

## Phase 8: User Story 6 — Compact screens (Priority: P2)

**Independent test**: Shrink the app below 768 px, verify full-area windows and reachable Launcher/overview/workspace controls, then restore the previous geometry.

- [ ] T049 [US6] Implement compact-mode detection and layout transitions with `useWindowSize` in `src/components/shell/ShellDesktop.vue`
- [ ] T050 [US6] Complete compact Shell controls, touch sizing, and no-horizontal-scroll behavior in `src/components/shell/ShellLauncher.vue`, `ShellWindowOverview.vue`, and `ShellWorkspaceOverview.vue`

## Phase 9: User Story 7 — Multi-instance app model (Priority: P3)

**Independent test**: Use a test-only app definition with `multiInstance: true` to open two tabs and two windows and restore both from persistence.

- [ ] T051 [US7] Add multi-instance reducer and persistence cases to `src/lib/shell/tabs.ts`, `src/lib/shell/layoutState.ts`, and `src-tauri/src/storage/shell_commands_tests.rs`
- [ ] T052 [US7] Add the multi-instance scenario to `scripts/check-shell-state.ts`

## Phase 10: Verification and handoff

- [ ] T053 [P] Add pure reducer, geometry, tabs, hydration, queue, and unknown-app checks to `scripts/check-shell-state.ts`
- [ ] T054 [P] Update `scripts/check-chat-state.ts` to load `src/components/apps/ChatApp.vue` and preserve the existing replay count
- [ ] T055 [P] Add Shell script and CI commands to `package.json` and `.github/workflows/ci.yml`
- [ ] T056 Run the full validation commands documented in `specs/015-workspace-shell/quickstart.md`, including targeted Rust tests, generated binding drift, Shell/chat checks, templates, typechecks, lint, and formatting
- [ ] T057 Run the manual scenarios from `specs/015-workspace-shell/quickstart.md` and record platform limitations in the PR
- [ ] T058 Re-run cross-artifact analysis across `spec.md`, `plan.md`, and `tasks.md`; resolve all CRITICAL/HIGH findings before opening the PR

## Dependencies and execution order

- T001–T004 are setup and must precede implementation.
- T005–T014 are the blocking chat-split foundation; each extraction step keeps the replay harness green.
- US1 depends on the foundation; US2 depends on the Shell store and host; US3 depends on the window frame; US4 and US5 depend on the reducers; US6 depends on the window frame; US7 depends on tabs and persistence.
- Rust storage tasks T042–T046 can run in parallel with frontend pure modules after the migration decision is fixed, but tasks touching the same file remain sequential.
- T056–T058 are the PR gate.

## MVP scope

The smallest demonstrable Shell is US1 plus the window-management subset of US2:
Chat and Settings as usable windows, Launcher, focus, close, minimize, and
content preservation. Tabs, multiple workspaces, persistence, and compact mode
follow as independently testable increments.
