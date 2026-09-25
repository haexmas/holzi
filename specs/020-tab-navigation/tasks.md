---
description: 'Task list for spec 020-tab-navigation'
---

# Tasks: Navigation im Tab (Vor/Zurück je Tab)

**Input**: Design documents from `/specs/020-tab-navigation/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Included. The constitution requires an executable check for non-trivial logic; the plan names `scripts/check-shell-navigation.ts` (`pnpm check:shell-navigation`, Node type-stripping, same style as `scripts/check-shell-state.ts`). Write each test task before its implementation task and see it fail first.

**Organization**: Tasks are grouped by user story. File paths assume spec 015 is merged (they come from branch `015-workspace-shell`). Modules under `src/lib/**` import siblings relatively with a `.ts` suffix and never use `~/` or Nuxt auto-imports (015 research R6). Every file stays ≤ 500 lines. Commits follow Conventional Commits and carry no agent attribution.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: User story from spec.md (US1–US8)

---

## Phase 1: Setup

**Purpose**: Gate, environment, script wiring

- [x] T001 Confirm the phase gate (research R15): spec 015 (PR #141) is merged into `main`; rebase `020-tab-navigation` onto `main` and resolve conflicts in `specs/` only
  - Done 2026-09-25: 015 merged as ccf932a (PR #141); rebased 020-tab-navigation onto main, the only conflict was .specify/feature.json (kept specs/020-tab-navigation); force-pushed with lease (no PR yet).
- [x] T002 Run a real `pnpm install` in `.worktrees/020-tab-navigation` (never symlink `node_modules`), then confirm `pnpm check:shell-state` and `pnpm check:chat-state` are green as the baseline; record both test counts in the T002 note
  - Done 2026-09-25: real pnpm install in the worktree; baseline check:shell-state 44/44 (one test added by the 015 review fixes), check:chat-state 45/45.
- [x] T003 [P] Add the 020 roadmap row to `plans/README.md` (after the 015 row): priority P1, effort M, status "Spezifiziert 2026-09-25, Umsetzung nach Merge von 015", gate "setzt Spec 015 voraus"
  - Done 2026-09-25: row added after 015 in plans/README.md.
- [x] T004 [P] Create `scripts/check-shell-navigation.ts` as an empty `node:test` harness with a header comment in the style of `scripts/check-shell-state.ts`; add `"check:shell-navigation": "node scripts/check-shell-navigation.ts"` to `package.json` and a matching step next to `check:shell-state` in `.github/workflows/ci.yml`
  - Done 2026-09-25: harness + package script + CI step. Deviation: check:shell-navigation runs several files via node --test (check-shell-navigation.ts, check-shell-actions.ts, check-shell-nav-store.ts as they appear) so each stays below 500 lines; later tasks name the file they extend.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Pure navigation and action modules, the store extension and the Vue building blocks every story uses

**⚠️ CRITICAL**: No user-story work starts before this phase is complete

### Tests first

- [x] T005 [P] Add history-reducer tests to `scripts/check-shell-navigation.ts`: `createHistory` defaults to `/`; `push` appends, drops forward entries and stores the leaving title; `push` of an equal location is a no-op (equality = normalized path and equal query regardless of key order); `replace` changes only the current entry; `go` within bounds moves `index`, outside bounds changes nothing; max 50 entries (`MAX_HISTORY_ENTRIES`), oldest dropped and `index` adjusted; `removeEntry` before, at and after `index` (at `index`: neighbor in travel direction); locations survive `JSON.parse(JSON.stringify(x))`
  - Done 2026-09-25: 21 tests in scripts/check-shell-navigation.ts (history + matcher sections), failing first on the missing modules.
- [x] T006 [P] Add route-matcher tests to `scripts/check-shell-navigation.ts`: literal and `:param` segments, `''` index child, nested chains outer → inner with merged params, URL-decoding of params, trailing-slash normalization, no match → `null`, an app without routes matches only `/`
  - Done 2026-09-25: matcher tests in the same file (index child, nested params with URL-decoding, trailing slash, unknown path, implicit '/' route).
- [x] T007 [P] Add action-core tests to `scripts/check-shell-navigation.ts`: schema validator accepts only the subset (`object`, `properties`, `required`, `string`, `number`, `integer`, `boolean`, `array`, `enum`, `description`) and reports the failing `field`; runner order per data-model.md "Ablauf runAction" with every error code (`unknown_action`, `invalid_input`, `target_required`, `target_not_found`, `forbidden_for_agents`, `app_unavailable`, `failed`); a `guardrails` action is refused for `builtinAgent` and `externalAgent` before its handler runs and executed for `user`; agents without explicit target get `target_required`, `user` falls back to focus
  - Done 2026-09-25: in scripts/check-shell-actions.ts (added to check:shell-navigation): schema subset incl. strict objects and array items, every runner error code, guardrail lock for both agent kinds, target rules.
- [x] T008 [P] Add catalog-invariant tests to `scripts/check-shell-navigation.ts` (data-model.md invariants 6–7): unique ids, every schema inside the subset, every `scope` exists in `scopes.ts`, `scope = 'guardrails'` ⇒ `agentCallable = false`, `binding = 'tab'` ⇒ `appId` set
  - Done 2026-09-25: catalog invariants iterate src/lib/actions/catalog.ts (ALL_ACTIONS aggregator, empty until the per-area catalogs land in US1/US4/US7), so they tighten automatically.

### Implementation

- [x] T009 [P] Implement `src/lib/shell/navigation.ts`: `TabLocation = { path: string; query: Record<string, string> }` (path app-relative, starts with `/`, no trailing `/` except `/`, segments URL-encoded), `HistoryEntry = { location; title: string | null }`, `TabHistory = { entries; index }` with `1 ≤ entries.length ≤ 50`, `parseLocation(string)`, `locationsEqual`, `createHistory`, `push`, `replace`, `go`, `removeEntry`, `canGoBack`, `canGoForward`, `backList`/`forwardList` (max 15 each, nearest first), per data-model.md transitions
  - Done 2026-09-25: immutable reducers (no-op returns the same object); also formatLocation/normalizePath helpers.
- [x] T010 [P] Implement `src/lib/shell/routeMatch.ts`: pure pattern type `RoutePattern = { path; titleKey?; children? }` and `matchRoute(patterns, path) → { chain, params } | null` per data-model.md "RouteRecord"
  - Done 2026-09-25: matchRoute(patterns, path) → { chain, params } | null.
- [x] T011 [P] Implement `src/lib/actions/types.ts` (`ShellActionDefinition`, `ActionScope`, `ActionCaller`, `ActionOutcome`, error codes) and `src/lib/actions/scopes.ts` (`shell.layout`, `shell.navigation`, `shell.read`, `chat.read`, `chat.write`, `settings.read`, `settings.device`, `settings.models`, `guardrails`, each with `titleKey` `actions.scopes.<name>`) exactly as in data-model.md
  - Done 2026-09-25: types.ts + scopes.ts; JsonSchema subset adds 'items' for arrays (needed by array inputs).
- [x] T012 [P] Implement `src/lib/actions/schema.ts`: validator for the JSON-schema subset, returning `{ ok: true } | { ok: false; field; message }` (no new dependency, research R8)
  - Done 2026-09-25: schema.ts validate + isSchemaInSubset, no dependency.
- [x] T013 Implement `src/lib/actions/runner.ts`: `createActionRunner({ catalog, resolveFocus, handlers })` with `runAction(id, input, caller)` following data-model.md steps 1–6; tab-bound handlers resolved through an injected `awaitTabHandler(appId, id, timeoutMs = 5000)`
  - Done 2026-09-25: runner.ts createActionRunner(deps) with globalHandler/resolveFocus/targetExists/awaitTabHandler injection; TAB_HANDLER_TIMEOUT_MS = 5000. 38/38 tests green.
- [ ] T014 Extend `src/lib/shell/types.ts` with `TabRuntime.history: TabHistory`; make `layoutState.ts` `hydrate`, `openApp` and `tabs.ts` `addTab` accept an optional start location and create each new tab's history (`hydrate` always `[{ path: '/' }]`, FR-011); keep all 015 tests green
- [ ] T015 Extend `src/stores/shell.ts`: per-tab `navigate(tabId, to, { replace })`, `go(tabId, delta)`, `historyOf(tabId)`, runner instance wired to the store (focus resolution from `state.activeWindowId`), tab-handler registry (`registerTabActionHandler(tabId, actionId, handler)`, removed on tab close), and `runAction(id, input?, caller = { kind: 'user' })`; keep the file ≤ 500 lines (extract a `src/stores/shellNavigation.ts` helper if needed)
- [ ] T016 Add store-integration tests to `scripts/check-shell-navigation.ts` for T014/T015 with the mocked `invokeFn` pattern from `scripts/check-shell-persistence.ts`: new tab has one `/` entry; `openApp(appId, at)` starts at `at`; `hydrate` gives every tab exactly `[/]`; closing a tab drops its history
- [ ] T017 [P] Implement `src/composables/useAction.ts` (`useAction(id)` → `(input?) => shell.runAction(id, input, { kind: 'user' })`)
- [ ] T018 [P] Implement `src/composables/useTabRouter.ts` per contracts/tab-navigation-contract.md §2 (`route` with `path`, `query`, `params`, `matched`; `canGoBack`, `canGoForward`, `push`, `replace`, `setQuery` defaulting to replace, `back`, `forward`); inert router at `/` outside a Shell, like `useShellTab()`
- [ ] T019 [P] Create `src/components/shell/ShellRouterView.vue` (renders the matched record at its own depth via provide/inject) and `src/components/shell/ShellLink.vue` (`<a href="#">`, `to`, `replace`, `prefix`, `aria-current="page"` when active) per contract §3
- [ ] T020 Replace `src/components/shell/appComponents.ts` with `src/components/shell/appRoutes.ts` (per-app route tables with components; apps without routes map `/` to their root component) and make `src/components/shell/ShellTabPanel.vue` render the root `ShellRouterView` for its tab; all three apps behave exactly as before
- [ ] T021 Extend `src/composables/useShellTab.ts` with `openApp(appId, at?)` and `registerActionHandler(actionId, handler)` (contract §4), wired in `ShellTabPanel.vue`

**Checkpoint**: `pnpm check:shell-navigation`, `check:shell-state`, `check:chat-state`, `typecheck` green; the app looks and behaves like 015

---

## Phase 3: User Story 1 - Zurück und Vor innerhalb eines Tabs (Priority: P1) 🎯 MVP

**Goal**: Back/forward buttons in every window's title bar navigate the active tab's history

**Independent Test**: quickstart M1–M3: open Chat, open three conversations in turn, back twice, forward once; buttons enable/disable correctly

- [ ] T022 [P] [US1] Add `shell.tab.back`, `shell.tab.forward`, `shell.tab.go` (`steps`) and `shell.tab.navigate` (`to`, `replace?`) with global handlers to `src/lib/actions/shellActions.ts` (target `tab`, scope `shell.navigation`, effect `write`, `agentCallable: true`; back/forward `defaultKeys` `Alt+ArrowLeft`/`Alt+ArrowRight`, mac additionally `Meta+BracketLeft`/`Meta+BracketRight`, `yieldToTextInput: { mac: true }`) and register them in a new `src/plugins/actions.client.ts`
- [ ] T023 [US1] Create `src/components/shell/ShellNavButtons.vue`: two icon buttons (`@lucide/vue` `arrow-left`/`arrow-right`), `aria-label` `shell.nav.back`/`shell.nav.forward`, `disabled` without entries in that direction, tooltip with the shortcut, `pointerdown.stop` so no window drag starts, clicks via `useAction`
- [ ] T024 [US1] Place `ShellNavButtons` left of `ShellTabBar` in `src/components/shell/ShellWindow.vue`, bound to the window's active tab; keep the 015 title-bar order otherwise (FR-015)
- [ ] T025 [US1] Give the chat its routes in `src/components/shell/appRoutes.ts` (`/` start = new conversation, `thread/:id` with `titleKey` `shell.chat.thread`) and create `src/composables/useChatNavigation.ts`: opening a conversation from the sidebar calls `push('/thread/<id>')`; a watcher on the route calls the existing `selectThread(id)` (or the new-conversation path for `/`); wire it into `src/components/apps/ChatApp.vue` with a single call (file stays ≤ 500 lines)
- [ ] T026 [US1] Verify replace semantics end to end with a test in `scripts/check-shell-navigation.ts`: a `setQuery` on the current location keeps the history length and back returns to the previous view (US1 AS5); nested chain `/models/hf/x` highlights `/models` via prefix match (US1 AS7)

**Checkpoint**: MVP — back/forward works for the chat in any window

---

## Phase 4: User Story 2 - Jeder Tab hat seine eigene Historie (Priority: P1)

**Goal**: Navigation in one tab never touches another; history survives window operations

**Independent Test**: quickstart M9–M12

- [ ] T027 [P] [US2] Add isolation tests to `scripts/check-shell-navigation.ts` (data-model.md invariants 1–3): navigation in tab A leaves tab B unchanged; `switchTab`, `minimizeWindow`, `toggleMaximizeWindow`, `moveWindowToWorkspace`, `switchWorkspace` never change any history; back without entries changes nothing and closes nothing (SC-005)
- [ ] T028 [US2] Make `ShellNavButtons` in `src/components/shell/ShellWindow.vue` react to the active-tab switch (US2 AS2) and fix any store gap the T027 tests expose in `src/stores/shell.ts`

**Checkpoint**: Two windows with independent histories (M9–M12)

---

## Phase 5: User Story 3 - Vor/Zurück per Maus, Tastatur und Geste (Priority: P1)

**Goal**: Keyboard, mouse side buttons, history list and system back reach the intended tab

**Independent Test**: quickstart M5–M8, M13; `shell.system.back` cases in `check:shell-navigation`

- [ ] T029 [P] [US3] Add keybinding tests to `scripts/check-shell-navigation.ts`: chord normalization `Ctrl+Alt+Shift+Meta+<code>`; platform detection; `Alt+ArrowLeft` resolves to `shell.tab.back` on all platforms and `Meta+BracketLeft` on mac; on mac an editable target with `Alt+ArrowLeft` is not intercepted, on Linux/Windows it is
- [ ] T030 [P] [US3] Implement `src/lib/shell/keybindings.ts` (`chordFromEvent`, `isMac`, `resolveChord(catalog, chord, platform)`, `shouldYield(action, platform, target)`)
- [ ] T031 [US3] Implement `src/composables/useShellKeyboard.ts` (one capture-phase `keydown` listener in the top document, `preventDefault` + `runAction` on a hit) and call it from `src/pages/workspace/[instance].vue`
- [ ] T032 [US3] Spike (quickstart §3): on Linux (WebKitGTK), and where available macOS and Windows, record whether mouse side buttons reach the DOM as `mouseup` with `button` 3/4 and whether the webview navigates its own history on them; write the results into a `Spike T032` note in this task and into research.md R6
- [ ] T033 [US3] Handle mouse side buttons in `src/components/shell/ShellWindow.vue`: `data-shell-window-id` on the root; `mouseup` `button === 3/4` → `shell.tab.back`/`forward` with the window's active `tabId`, no `focusWindow`; `preventDefault` on `mousedown`/`auxclick` for those buttons (contracts/shell-actions.md §4)
- [ ] T034 [US3] Create `src/components/shell/ShellHistoryMenu.vue` (haex-ui `ShadcnDropdownMenu`, max 15 entries nearest first, entry titles, keyboard operable) and open it from `ShellNavButtons` on long press (≥ 500 ms, pointer stays on the button) or `contextmenu`; selection runs `shell.tab.go` with `steps` (FR-016)
- [ ] T035 [US3] Add `shell.system.back` (scope `shell.navigation`, `agentCallable: false`) to `src/lib/actions/shellActions.ts` with the three-step handler from contracts/shell-actions.md §5 (close open shell overlay → back in top visible window's active tab → compact mode without entries opens the window overview); expose the overlay open-state it needs from the shell components/store; add its three cases to `scripts/check-shell-navigation.ts`
- [ ] T036 [US3] Add the Android back hook behind a platform check in the Tauri setup (research R7): verify the Tauri 2 back-button API first, call `shell.system.back`; mark with a `ponytail:` comment that it is untested until an Android target exists

**Checkpoint**: M5–M8 pass; system-back logic covered by tests

---

## Phase 6: User Story 8 - Inhalte in Tabs verändern die Navigation nicht (Priority: P1)

**Goal**: Embedded documents (haextensions) cannot change holzi's navigation

**Independent Test**: quickstart M13, M21, M22; SC-009

- [ ] T037 [US8] Keep the top document's webview history flat: enter the workspace page with `router.replace` from `src/pages/index.vue` and the onboarding completion path in `src/pages/onboarding/[instance].vue`; add `onBeforeRouteLeave` to `src/pages/workspace/[instance].vue` that cancels every router navigation away and ignores it (never interpreted as back, research R7)
- [ ] T038 [US8] Disable webview-level history navigation from input devices where the platform allows it, in the Tauri window setup (`src-tauri/src/lib.rs` or the window builder): WebView2 browser accelerator keys off; mouse back/forward navigation off per the T032 findings; document platforms without such a setting as limitations in research.md R17
- [ ] T039 [US8] Run quickstart M13, M21 and M22 manually and record the results (including per-platform limitations) in a note on this task

**Checkpoint**: 20 history writes and backs inside an iframe change no tab history (SC-009)

---

## Phase 7: User Story 4 - Apps direkt an einem Ort öffnen (Priority: P2)

**Goal**: Open an app at a location; a running singleton navigates instead

**Independent Test**: quickstart M14–M15; singleton-push tests

- [ ] T040 [P] [US4] Add tests to `scripts/check-shell-navigation.ts`: `shell.app.open` with `at` on a closed app starts a single-entry history at `at`; on an open singleton it activates the tab (015 FR-016) and pushes `at`; the same location adds no entry (US4 AS3)
- [ ] T041 [US4] Add `shell.app.open` (`appId`, `at?`; target `none`, scope `shell.navigation`) and `shell.tab.new` (`appId`, `at?`; target `window`, scope `shell.layout`) to `src/lib/actions/shellActions.ts`, delegating to the store's `openApp`/`addTab`
- [ ] T042 [US4] Accept `&at=<path>` next to `?open=` in `src/pages/workspace/[instance].vue` (call `shell.app.open`, then strip both via `router.replace`) and keep the legacy redirects in `src/pages/{chat,settings,federation}/[instance].vue` working (FR-013)
- [ ] T043 [US4] Make the root `ShellRouterView` handle an unknown location: `replace('/')` and show the `shell.nav.unknownLocation` toast (haex-ui toast) (FR-014)
- [ ] T044 [US4] Route the chat header's settings button (`src/components/chat/ChatHeader.vue` / `ChatApp.vue`) through `shell.app.open` with `appId: 'system.settings'`

---

## Phase 8: User Story 5 - Tab-Titel folgt der Ansicht (Priority: P2)

**Goal**: Tab, tab list, window overview and history list show the view's title

**Independent Test**: quickstart M5 titles; title tests

- [ ] T045 [P] [US5] Add title tests to `scripts/check-shell-navigation.ts`: display title = `setTitle` override → deepest matched `titleKey` (with `{param}` interpolation) → app name; the override resets on navigation; a left entry keeps the title shown when it was left (research R9)
- [ ] T046 [US5] Implement the title chain in `src/stores/shell.ts` `tabDisplayInfo` and store the leaving title on `push`/`go`; show it in `ShellTabBar.vue`, `ShellTabListMenu.vue`, `ShellWindowOverview.vue` and `ShellHistoryMenu.vue`; let the chat set the conversation title via `setTitle` in `useChatNavigation.ts`

---

## Phase 9: User Story 7 - Jede Aktion ist beschrieben und aufrufbar (Priority: P2)

**Goal**: A complete, described action catalog for shell, chat and settings; guardrails locked for agents

**Independent Test**: quickstart M23; runner and catalog tests with agent callers (SC-007, SC-008)

- [ ] T047 [US7] Complete `src/lib/actions/shellActions.ts` with every shell action of contracts/shell-actions.md §1 (layout, workspace, overview, launcher, read actions `shell.state.get`, `shell.tab.history`, `shell.apps.list`, `shell.actions.list`), each with English `description`, input/result schema, target, scope, effect; destructive window/tab closes run the 015 guards and return `failed` "declined by user" on decline
- [ ] T048 [US7] Switch every state-changing control in `src/components/shell/*.vue` (tab bar, "+" menu, Chevron, window controls, overview, workspace overview, launcher, close confirm triggers) to `useAction`; no direct store mutation from templates remains
- [ ] T049 [US7] Inventory every state-changing control in `src/components/apps/ChatApp.vue`, `src/components/chat/**` and write the list into this task's note; create `src/lib/actions/chatActions.ts` with at least the chat rows of contracts/shell-actions.md §1 (`chat.approval.decide` and `chat.permissionMode.set` in scope `guardrails`, `agentCallable: false`) plus every inventoried control
- [ ] T050 [US7] Implement tab-bound chat handlers in `src/composables/useChatActions.ts` (registered through `useShellTab().registerActionHandler`, one call from `ChatApp.vue`) and switch the chat controls to `useAction`; `pnpm check:chat-state` keeps its T002 count
- [ ] T051 [US7] Inventory every state-changing control in `src/components/apps/SettingsApp.vue`, `src/components/settings/**`, `src/components/models/**` and write it into this task's note; create `src/lib/actions/settingsActions.ts` with at least the settings rows of contracts/shell-actions.md §1 (`settings.delegate.connectProvider`, `settings.autonomy.setMode`, `settings.delegate.setDenyRules` in scope `guardrails`, `agentCallable: false`) plus every inventoried control
- [ ] T052 [US7] Implement the global settings handlers in `src/plugins/actions.client.ts` (via `usePreferences`, `useDevice`, provider and model composables) and switch the settings/model controls to `useAction`; `settings.get` never returns provider credentials or secrets
- [ ] T053 [US7] Add the template rule to `scripts/check-vue-templates.ts` (research R20): in `src/components/shell/**` and `src/components/apps/**`, `@click`/`@select` handlers that change state call only `useAction`-derived functions; exemptions carry an `action-exempt:` comment; extend its tests
- [ ] T054 [US7] Extend `scripts/check-shell-navigation.ts` with catalog-wide agent tests: every `guardrails` action refused for both agent kinds and executed for `user` (SC-008); `shell.state.get` returns workspaces, windows, tabs with app, location, title, attention

---

## Phase 10: User Story 6 - Navigation im Chat (Priority: P3)

**Goal**: Conversation switches and new conversations are history entries, consistent with specs 003/004/006

**Independent Test**: quickstart M2–M4, M16

- [ ] T055 [US6] In `src/composables/useChatNavigation.ts`: "new conversation" pushes `/`; the first message creating a thread (`useComposer.ts` sets `activeThreadId`) replaces `/` with `/thread/<id>`; deleting the active thread (spec 006 FR-015) replaces with `/`
- [ ] T056 [US6] Skip entries to deleted conversations: on reaching `/thread/<id>` for a missing thread, `removeEntry` and continue in the same direction, else `replace('/')` (FR-027); navigation reuses `selectThread`, so running replies and pending approvals follow specs 003/006 unchanged
- [ ] T057 [US6] Add chat-navigation cases to the replay harness via `scripts/lib/chat-state-harness.ts` / `scripts/check-chat-state.ts` where feasible (open → back → forward, create-replace, delete-skip); existing replay count unchanged plus the new cases

---

## Phase 11: Polish & Cross-Cutting Concerns

- [ ] T058 [P] Add all new keys (`shell.nav.*`, `shell.chat.thread`, `actions.*`, `actions.scopes.*`) to `src/i18n/locales/de.json` and `en.json` in lockstep; `jq` key comparison shows no difference (FR-023)
- [ ] T059 [P] Add Ort, Tab-Historie, Aktion, Berechtigungsbereich and Aufrufer to `CONTEXT.md` (note: code says "action", never "command")
- [ ] T060 Run quickstart §1 (all automated checks) and §2 (M1–M23); record results and platform limitations for the PR
- [ ] T061 Run `/speckit-analyze` on spec/plan/tasks; resolve all CRITICAL/HIGH findings before opening the PR

---

## Dependencies & Execution Order

- **Setup (T001–T004)** → **Foundational (T005–T021)** → user stories.
- **US1 (T022–T026)** is the MVP and needs Foundational only.
- **US2 (T027–T028)** and **US3 (T029–T036)** need US1 (buttons, back/forward actions).
- **US8 (T037–T039)** needs US3's T032 spike for T038; T037 can start after Foundational.
- **US4 (T040–T044)** needs Foundational; **US5 (T045–T046)** needs US3's T034 for the history-list titles.
- **US7 (T047–T054)** needs Foundational; T048 touches the same shell components as US1/US3 — do it after them. T050 and T055–T057 both touch chat composables — run US6 after T050.
- **US6 (T055–T057)** needs T025.
- **Polish (T058–T061)** last; T061 is the PR gate.

## Parallel Examples

- Foundational tests: T005, T006, T007, T008 together; then T009, T010, T011, T012 together.
- After Foundational: T017, T018, T019 together.
- US3: T029 and T030 together while T032 (spike) runs.
- US7: T049 and T051 inventories in parallel; their handler tasks T050/T052 afterwards.
- Polish: T058 and T059 together.

## Implementation Strategy

1. **MVP**: Setup + Foundational + US1 → back/forward for the chat, all 015 checks green. Stop and validate M1–M3.
2. **P1 completion**: US2, US3, US8 → correct targets, inputs, isolation.
3. **P2**: US4, US5, US7 → deep links, titles, full agent-ready catalog (prerequisite for spec 021).
4. **P3**: US6 → chat history details.
5. Each story ends with its checkpoint; commit per task, push after each phase.
