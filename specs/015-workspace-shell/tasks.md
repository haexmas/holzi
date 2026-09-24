# Tasks: Workspace-Shell

**Input**: Design documents from `/specs/015-workspace-shell/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

## Phase 1: Setup and roadmap gate

- [x] T001 Add the approved Shell phase and its dependency on feature 013 to `plans/README.md`
  - Done 2026-09-24: roadmap row added, dependency on Spec 013 named. Commit `3301fdd`.
- [x] T002 [P] Add Shell/App/Window/Tab/Launcher terminology to `CONTEXT.md`
  - Done 2026-09-24: added to "Sprachkonventionen im UI" alongside the existing Workspace/Arbeitsbereich entry. Commit `3301fdd`.
- [x] T003 [P] Confirm migration numbering and trigger version against the current `main` and feature-013/014 branches in `specs/015-workspace-shell/research.md`
  - Done 2026-09-24: main is at `0018`/version 10 (013 already merged into it); no `014-portable-mode` branch exists yet. `0019`/11 confirmed free. Commit `3301fdd`.
- [x] T004 [P] Add the `check:shell-state` script entry and CI placeholder in `package.json` and `.github/workflows/ci.yml`
  - Done 2026-09-24: placeholder harness (`scripts/check-shell-state.ts`) wired into both; real checks land in T053. Commit `3301fdd`.

## Phase 2: Foundational chat split

**Purpose**: Move behavior without changing the chat contract before embedding it in a Shell window.

- [x] T005 Capture the current replay-test count and baseline output of `pnpm check:chat-state` in `specs/015-workspace-shell/quickstart.md`
  - Done 2026-09-24: 45/45 green, recorded in quickstart.md §0. Commit `5c65449`.
- [x] T006 Extract composer send/cancel/new-conversation logic into `src/composables/useComposer.ts` without changing behavior
  - Done 2026-09-24: `send`/`abort`/`newChat`/`onVoiceTranscript` moved; shared refs (`input`/`busy`/`pendingSend`/`turnSetupPending`) stay page-created, passed in as dependencies (same convention as `useChatTranscript`/`useThreadSidebar`). Commit `1a8a14a`.
- [x] T007 Run `pnpm check:chat-state` and preserve the baseline replay-test count after T006
  - Done 2026-09-24: 45/45 green. Commit `695881e`.
- [x] T008 Extract attachment handling into `src/composables/useComposerAttachments.ts` without changing behavior
  - Done 2026-09-24: owns `attachments` plus add/remove/refresh-on-model-switch. Commit `695881e`.
- [x] T009 Run `pnpm check:chat-state` and preserve the baseline replay-test count after T008
  - Done 2026-09-24: 45/45 green. Commit `695881e`.
- [x] T010 [P] Extract the thread sidebar into `src/components/chat/ThreadSidebar.vue` with props and emits only
  - Done 2026-09-24: includes the delete-confirmation `UiDrawerModal` (same thread-management concern). The edited-row input's focus/select moved from `useThreadSidebar.ts` into a local watcher here, since the DOM node is now local to this component. Commit `9ac6d20`.
- [x] T011 [P] Extract transcript, reasoning, tool rows, and audit-marker translation into `src/components/chat/MessageList.vue`
  - Done 2026-09-24: `renderMarkdown`/`delegateAnsweredByLabel`/`DENY_AUDIT_MARKERS`/`toolResultContentLabel`/`reasoningFor` moved in as local pure functions; emits `toggleReasoning` since that state is also cleared from outside (new chat, thread delete). Commit `9ac6d20`.
- [x] T012 [P] Extract the composer view into `src/components/chat/Composer.vue` using existing composer controls
  - Done 2026-09-24: also reads `useModelsStore()` directly for model/effort display instead of prop-drilling it, and owns `useAutoResizeTextarea` (needs its own `<textarea>` DOM node — exposes `reset()` via `defineExpose` for the page's `useComposer`/`useThreadSidebar` dependency). Commit `9ac6d20`.
- [x] T013 Integrate the extracted chat components into `src/pages/chat/[instance].vue` and keep the orchestrator under 500 lines
  - Done 2026-09-24: page went 1109 → 492 lines. Two further extractions beyond T010-T012 were needed to get under 500: `useChatPermissionMode.ts` (permission-mode/autonomy-mode persistence) and `useChatSubscriptions.ts` (`chat.on*` listener registration). `useComposer.ts` also absorbed the sub-agent-activity indicator state. Two new small presentational components, `ChatHeader.vue` and `StatusBanners.vue`, cover the header and the three status banners. Commit `9ac6d20`.
- [x] T014 Run `pnpm check:chat-state`, `pnpm typecheck`, and `pnpm lint` after the split
  - Done 2026-09-24: 45/45 green, typecheck clean, lint clean, `pnpm check:templates` also run (33/33 Vue templates compile, including all 5 new components). Documented in quickstart.md §0.

## Phase 3: User Story 1 — Apps as windows (Priority: P1)

**Independent test**: Open a completed vault, launch Chat and Settings from the Launcher, use both, and reach them through the legacy routes.

- [x] T015 [P] [US1] Define Shell entities and app identifiers in `src/lib/shell/types.ts` and `src/lib/shell/apps.ts`
  - Done 2026-09-24: `Workspace`/`ShellTab`/`ShellWindow`/`ShellState`/`CloseGuard(Result)`/`TabRuntime`/`PersistedLayout` types; the three shipped apps in `apps.ts`. Verified loading standalone under Node's type stripping. Commit `c97bf63`.
- [x] T016 [P] [US1] Implement pure app/window/workspace hydration and opening reducers in `src/lib/shell/layoutState.ts`
  - Done 2026-09-24: `openApp` (singleton search across all windows, FR-016), `focusWindow`, `closeWindow`, `hydrate` (drops unresolvable tabs/empty windows, repairs dangling `activeTabId`/`workspaceId`, clamps geometry, re-derives `stack`, falls back to a default workspace). `getAppDefinition` (apps.ts) now takes the app list as a parameter (needed for T052 later). Verified by hand against every hydrate repair path. Commit `b158245`.
- [x] T017 [P] [US1] Implement app component mapping with async components in `src/components/shell/appComponents.ts`
  - Done 2026-09-24: built once at module load (not per call), so Vue's async-component cache stays keyed by app id. Commit `0b666ea` (done after T021/T022 since it needs `ChatApp.vue`/`SettingsApp.vue`/`FederationApp.vue` to exist — file-level dependency, not a spec change).
- [x] T018 [US1] Implement the Pinia Shell store and public actions in `src/stores/shell.ts`
  - Done 2026-09-24: `openApp`/`focusWindow`/`closeWindow`/`flushAsync` (placeholder until T047) plus `tabRuntime` bookkeeping and the three `useShellTab()` setters. Commits `479ab61`, extended in `d4f95d4` (T023) with the setters once `ShellWindow.vue` needed them.
- [x] T019 [P] [US1] Add the Shell tab contract (`useShellTab`) in `src/composables/useShellTab.ts`
  - Done 2026-09-24: provide/inject per contracts/shell-app-contract.md, inert default outside a Shell instance. Commit `2c16c94`.
- [x] T020 [P] [US1] Move preload listeners and status state into `src/composables/useModelPreloadStatus.ts`
  - Done 2026-09-24: moved unchanged from the old workspace stub; wired into `ShellStatusBar.vue` in T025. Commit `2c16c94`.
- [x] T021 [US1] Move the split chat page to `src/components/apps/ChatApp.vue`, preserving route-independent behavior
  - Done 2026-09-24: `git mv`; instance name from `useInstancesStore()`, settings links → `shell.openApp()`, `lock()` flushes first, `h-screen` → `h-full min-h-0`. Harness path and fake `useShellStore` updated to match. All 45 tests/typecheck/lint/vault-lifecycle/38-template-compile stay green. Commit `daab7df`.
- [x] T022 [P] [US1] Move settings and federation pages to `src/components/apps/SettingsApp.vue` and `src/components/apps/FederationApp.vue`
  - Done 2026-09-24: same route-coupling treatment; Settings drops its now-meaningless "back to workspace" link. Commit `310ff47`.
- [x] T023 [P] [US1] Add Shell host and Launcher components in `src/components/shell/ShellDesktop.vue`, `src/components/shell/ShellLauncher.vue`, and `src/pages/workspace/[instance].vue`
  - Done 2026-09-24: also created `ShellWindow.vue` ahead of T029 — deliberately minimal (icon/title/close, click-to-focus, no drag/resize/minimize/maximize/tabs yet), since US1's own test needs a real window to click and close; T029 extends this same file rather than replacing it. Removed the now-dead `components/workspace/ChatFab.vue`. Verified with typecheck/lint/38-template-compile and a full `pnpm generate` production build; not yet checked in a running Tauri window (needs a real vault session — deferred to the manual quickstart pass). Commit `d4f95d4`.
- [x] T024 [US1] Add legacy route redirects in `src/pages/chat/[instance].vue`, `src/pages/settings/[instance].vue`, and `src/pages/federation/[instance].vue`
  - Done 2026-09-24: `definePageMeta({ redirect })` to `/workspace/:instance?open=system.<app>` — resolves before any navigation guard, so `onboarded` still runs against the final URL. Also added the `?open=` consumption on the workspace host page itself (belongs with T023, was the one piece still missing there). Commit `e268dd4`.
- [x] T025 [US1] Add Shell status bar and preserve onboarding/instance close behavior in `src/components/shell/ShellStatusBar.vue` and `src/pages/workspace/[instance].vue`
  - Done 2026-09-24: markup moved unchanged from the old stub, wired to `useModelPreloadStatus`. Commit `f6d6421`.
- [x] T026 [US1] Add initial Shell translations and remove obsolete workspace Stub keys only after `rg` confirms no remaining usage in `i18n/locales/de.json` and `i18n/locales/en.json`
  - Done 2026-09-24: `shell.apps.*`/`shell.launcher.*`/`shell.window.close` added; `workspace.heading`/`chatFab.*`/`settings.iconTitle` removed (confirmed unused via `rg`), `workspace.modelPreload.*` kept. `jq` key-diff confirms exact de/en parity. **Phase 3 (User Story 1) complete.** Commit `81461aa`.

## Phase 4: User Story 2 — Window management (Priority: P1)

**Independent test**: Move, resize, minimize, restore, maximize, and close two windows while a Chat draft and response remain alive.

- [x] T027 [P] [US2] Implement geometry clamping, cascade placement, minimum sizes, compact correction, and maximize/restore invariants in `src/lib/shell/geometry.ts`
  - Done 2026-09-24: `clampGeometry` (moved from `layoutState.ts`), `clampDragPosition` (64x32-visible-strip rule), `clampResizeSize`, `cascadePosition` (moved), `windowDisplayRect` (resolves maximize/compact display rect without ever touching stored geometry, research R7). Commit `b90756c`.
- [x] T028 [P] [US2] Implement pointer-capture move and eight-way resize gestures in `src/composables/useWindowPointerGesture.ts`
  - Done 2026-09-24: pointer capture, requestAnimationFrame-batched updates, delegates all math to `geometry.ts`. Commit `06a88a6`.
- [x] T029 [US2] Add Shell window frame, title bar, focus, minimize, maximize, restore, and close controls in `src/components/shell/ShellWindow.vue` and `src/components/shell/ShellWindowControls.vue`
  - Done 2026-09-24: `layoutState.ts` gained `minimizeWindow`/`toggleMaximizeWindow`/`updateWindowGeometry`; `ShellWindow.vue` wires the pointer gesture to the title bar and 8 new resize handles, disabled while compact/maximized. Commit `f6a4e6c`.
- [x] T030 [US2] Add window overview with minimized-window restore, close, active-tab title, and attention badges in `src/components/shell/ShellWindowOverview.vue`
  - Done 2026-09-24: store gained `windowDisplayInfo` (icon/title/tabCount/attention, i18n-free — callers translate), shared with `ShellWindow.vue`'s own title. Overview-trigger button added next to the Launcher FAB. Commit `9b9b1ae`.
- [x] T031 [US2] Keep visited app tabs mounted with `v-show`, wire close guards and attention state into `src/components/shell/ShellWindow.vue` and `src/composables/useShellTab.ts`
  - Done 2026-09-24: fixed a real bug — `ShellWindow.vue` had no `v-show` gate at all, so minimized windows stayed fully visible. Tab content sits behind a lazy-mounted `role="tabpanel"`. `useShellTab.ts` gained `requestCloseWindow` (gathers guard results via the store's new `guardResultsFor`, a `window.confirm` placeholder until T038, then closes). Commit `e7f045b`.
- [x] T032 [US2] Add Chat close guard and permission attention lifecycle in `src/components/apps/ChatApp.vue`
  - Done 2026-09-24: registers a close guard (running reply/pending permission → confirm → abort()); `onToolPermissionRequest` requests attention, cleared on answer (once no approvals remain) or turn-complete. Caught a real regression before committing: `chat-state-harness.ts` had no fake `useShellTab`, so all 45 tests failed with "useShellTab is not defined" — fixed and reverified. Trimmed comments to keep the page at 499 lines. **Phase 4 (User Story 2) complete.** Commit `86efd69`.

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
