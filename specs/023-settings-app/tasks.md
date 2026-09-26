---
description: 'Task list for spec 023-settings-app'
---

# Tasks: Einstellungs-App mit Kategorien

**Input**: Design documents from `/specs/023-settings-app/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/settings-app.md](./contracts/settings-app.md), [quickstart.md](./quickstart.md)

**Tests**: Included. The constitution requires an executable check for non-trivial logic: the new `scripts/check-settings.ts` (`pnpm check:settings`, in CI) for the registry, header back, color scheme and app alias; a palette-color denylist in `scripts/check-vue-templates.ts`; Rust tests in `src-tauri/src/storage/known_devices_tests.rs` and `src-tauri/src/device/commands_tests.rs`. Write each test task before its implementation and see it fail first.

**Organization**: Grouped by user story. The branch `023-settings-app-plan` starts from `main` (specs 015, 020, 022 and the haex-ui pin `db48f9a948522c18a00331aac232718825cc9317` merged). Modules under `src/lib/**` import siblings relatively with a `.ts` suffix so the Node check scripts can load them. Every file stays ≤ 500 lines. Commits follow Conventional Commits and carry no agent attribution. Rust commands run as `nix develop --command scripts/with-nix-host-bridge.sh cargo …`; frontend commands as `nix develop --command bash -c '…'`.

**Shipping note**: Everything lands in one PR. US1 registers all five categories; the views of "Darstellung" (US4) and "Föderation" (US5) are added in their phases, so those two locations render an empty content area until then. Do not merge before all phases are done.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: User story from spec.md (US1–US6)

---

## Phase 1: Setup

- [ ] T001 Prepare `.worktrees/023-settings-app`: rerun the real `pnpm install` (main moved to the haex-ui pin `db48f9a`); reflink the Rust build cache from the primary checkout if its `src-tauri/Cargo.lock` matches (`cp -a --reflink=always ../../src-tauri/target src-tauri/target`); record baseline counts of `pnpm check:wm-state`, `check:wm-navigation`, `check:chat-state`, `check:vault-lifecycle`, `check:templates` and `cargo test` in this task's note
- [ ] T002 [P] Add the 023 row to `plans/README.md` after the 022 row: priority P1, effort M, status "Spezifiziert 2026-09-26, Plan 2026-09-26", gate "setzt Spec 015, 020, 022 voraus"
- [ ] T003 [P] Add `"check:settings": "node --test scripts/check-settings.ts"` to `package.json` and a step "Check settings app" running `corepack pnpm check:settings` after "Check window manager navigation" in `.github/workflows/ci.yml`; create `scripts/check-settings.ts` with the file header only

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The pure settings registry and its routes. Every user story builds on it.

**⚠️ CRITICAL**: No user-story work starts before this phase is complete

### Tests first

- [ ] T004 Write registry tests in `scripts/check-settings.ts` (fail until T006): exactly five categories in the order `general`, `appearance`, `models`, `agents`, `federation` with paths `/`, `/appearance`, `/models`, `/agents`, `/federation`; every location id and path unique; every location has a category, `titleKey`, `descriptionKey`; every `parent` exists and belongs to the same category; `overviewRows('models')` = `models.default`, `models.installed`, `models.download`, `models.speech` and `overviewRows('agents')` = `agents.providers`, `agents.autonomy`, `agents.denyRules`; `general`, `appearance`, `federation` have no overview rows; `matchRoute(settingsRoutePatterns(), path)` resolves every location from contracts §1, including `/models/download/repo/Qwen/Qwen2.5-GGUF` with `params = { owner: 'Qwen', name: 'Qwen2.5-GGUF' }`; `locationFor('/nope')` is `undefined`; every registry i18n key exists in both `src/i18n/locales/de.json` and `en.json` (FR-020)
- [ ] T005 Write `headerBack` tests in `scripts/check-settings.ts` (fail until T007), using `createHistory`/`push` from `src/lib/wm/navigation.ts`: previous entry is the parent path → `{ kind: 'back' }`, also when the previous entry carries a query (`/models/download/search?q=qwen` as parent of a repo); previous entry is some other location → `{ kind: 'push', path: parent }`; a history with only the current entry (deep link) → `{ kind: 'push', path: parent }`

### Implementation

- [ ] T006 Implement `src/lib/settings/registry.ts` per data-model.md and contracts §1: `SETTINGS_CATEGORIES`, `SETTINGS_LOCATIONS`, `settingsRoutePatterns()`, `locationFor(path)`, `categoryOf(path)`, `overviewRows(categoryId)`; pure, relative imports only (`../wm/routeMatch.ts`); make T004 pass except the i18n part
- [ ] T007 Implement `headerBack(history, parentPath)` in `src/lib/settings/registry.ts` (research R3); make T005 pass
- [ ] T008 [P] Add `settings.categories.<id>.{title,description}` for the five categories, `settings.locations.<id>.{title,description}` for every location with a parent, and `settings.back` ("Zurück zu {title}" / "Back to {title}") to `src/i18n/locales/de.json` and `en.json`; the repo location title interpolates `{owner}/{name}`; make the i18n part of T004 pass
- [ ] T009 Replace the `system.settings` entry in `src/components/wm/appRoutes.ts` with a root `/` record (component `SettingsApp`) whose children come from `settingsRoutePatterns()`, each with its async component from contracts §1 keyed by location id and its `titleKey` for tab titles and the history list (spec 020 R9)

**Checkpoint**: `pnpm check:settings` green; the settings app still renders its old page at `/`.

---

## Phase 3: User Story 1 - Einstellungen nach Kategorien finden (P1) 🎯 MVP

**Goal**: Sidebar, header and content; every existing setting lives in exactly one category and saves without a button (FR-021).

**Independent Test**: quickstart S1–S3, S17.

- [ ] T010 [US1] Rebuild `src/components/apps/SettingsApp.vue` as the frame (contracts §2): sidebar left, header with title and one-line description from `locationFor` (params interpolated through `t`), content `<WmRouterView />` at depth 1 as the only scrolling area; load `currentDeviceInfoAsync()` once and `provide` it with a reload function under `SETTINGS_DEVICE_KEY` (exported from a small `src/components/settings/deviceContext.ts`)
- [ ] T011 [P] [US1] Create `src/components/settings/Sidebar.vue`: one button per `SETTINGS_CATEGORIES` entry with icon and name, `aria-current="page"` on `categoryOf(route.path)`, click → `router.push(category.path)` even when that category is already active (FR-010)
- [ ] T012 [P] [US1] Create `src/components/settings/OverviewView.vue`: rows from `overviewRows(categoryOf(route.path))` with icon (`text-primary`), title, one-line description and chevron; click → `router.push(row.path)` (FR-003)
- [ ] T013 [P] [US1] Create `src/components/settings/GeneralView.vue` with `SettingsAliasSetting` (device from `SETTINGS_DEVICE_KEY`, reload on save) and `SettingsSessionRestoreSetting`
- [ ] T014 [P] [US1] Convert `src/components/settings/AliasSetting.vue` (research R5): save on blur and on Enter through `settings.device.setAlias`; an empty name is not saved and the field explains why; drop the save button and its i18n text
- [ ] T015 [P] [US1] Convert `src/components/settings/DefaultModelSetting.vue`: two selects "Dieses Gerät" (Wie alle Geräte / models) and "Alle Geräte" (Keins / models), saved on selection via `settings.models.setDefault` / `.clearDefault`; device uuid from `SETTINGS_DEVICE_KEY`; drop the scope radios and both buttons
- [ ] T016 [P] [US1] Convert `src/components/settings/SttModelSetting.vue`: the select saves on selection via `settings.models.setStt`; a model that is not installed is fetched with visible progress; drop the switch button
- [ ] T017 [P] [US1] Convert `src/components/settings/AutonomyModeSetting.vue`: the options save on selection via `settings.autonomy.setMode`; drop the save button
- [ ] T018 [P] [US1] Convert `src/components/settings/DelegateDenyRulesSetting.vue`: save on blur via `settings.delegate.setDenyRules`; invalid rules are not saved and the field explains why; drop the save button
- [ ] T019 [US1] Add `downloads: Record<string, DownloadProgressEvent>` and an idempotent `watchDownloads()` to `src/stores/models.ts` (research R6), with a `ponytail:` comment that the subscription lives until the process ends (one vault per process, spec 013); call it in `src/pages/workspace/[instance].vue` after the session restore
- [ ] T020 [US1] Create `src/components/settings/InstalledModels.vue` from the installed-models tab of `src/components/models/HuggingFaceModelManagement.vue`: list, load, delete, check and install updates, integrity dialog; reads `useModelsStore()`
- [ ] T021 [P] [US1] Create `src/components/settings/DownloadModels.vue` from the catalog tab: recommended models with progress from `models.downloads`, and a row "Auf HuggingFace suchen" → `router.push('/models/download/search')`
- [ ] T022 [P] [US1] Rework `src/components/models/HuggingFaceSearch.vue` as the location `/models/download/search`: search term in `route.query.q` via `router.setQuery({ q })` (replace), a result → `router.push('/models/download/repo/<owner>/<name>')` instead of emitting `select`
- [ ] T023 [P] [US1] Rework `src/components/models/HuggingFaceFilePicker.vue` as the location `/models/download/repo/:owner/:name`: repo from `route.params`, progress from `models.downloads`; remove its own download listeners
- [ ] T024 [US1] Delete `src/components/models/HuggingFaceModelManagement.vue` (its size exception goes with it) and the unused i18n keys (`settings.header.forDevice`, the save texts of T014–T018, the old tab labels); `pnpm typecheck`, `lint`, `check:templates` green
- [ ] T025 [US1] Operator: run quickstart S1–S3 and S17

**Checkpoint**: every existing setting is reachable by category and saves without a button.

---

## Phase 4: User Story 2 - Unteransichten mit Übersicht und Zurück (P1)

**Goal**: The header back arrow and robust sub-views.

**Independent Test**: quickstart S4–S7, S9.

- [ ] T026 [US2] Add the back arrow to the header in `src/components/apps/SettingsApp.vue` for locations with a `parent`: label `settings.back` with the parent's title; click runs `headerBack(wm.historyOf(tabId), parentPath)` → `router.back()` or `router.push(path)` (FR-009)
- [ ] T027 [P] [US2] Show a hint with the way back to the overview instead of an error when a sub-view's data is gone: unknown repo in `HuggingFaceFilePicker.vue`, no provider connected in `ConnectDelegateProvider.vue`, deleted model in `InstalledModels.vue` (edge case)
- [ ] T028 [US2] Operator: run quickstart S4–S7 and S9

---

## Phase 5: User Story 3 - Direkt an eine Stelle springen (P2)

**Goal**: Callers open the settings where the user needs them.

**Independent Test**: quickstart S8, S10.

- [ ] T029 [US3] Audit the callers of `openApp({ appId: 'system.settings' })` and `wm.app.open` for the settings (`src/components/apps/ChatApp.vue`, chat banners and dialogs): contextual hints (for example "no model installed") pass `at` with the matching location (`/models/download`, `/models/default`, `/agents/providers`); generic settings buttons stay at `/`
- [ ] T030 [US3] Operator: run quickstart S8 and S10

---

## Phase 6: User Story 4 - Farbschema wählen (P2)

**Goal**: Light, dark or system, applied at once and readable everywhere.

**Independent Test**: quickstart S11, S12.

### Tests first

- [ ] T031 [P] [US4] Write color-scheme tests in `scripts/check-settings.ts` (fail until T033): `parseColorScheme` accepts `light`/`dark`/`system` and maps anything else to `null`; `effectiveColorScheme` device over vault, both unset → `system`; `isDark('system', true)` true, `isDark('system', false)` false, `isDark('light', true)` false
- [ ] T032 [P] [US4] Add a palette-color denylist to `scripts/check-vue-templates.ts` (research R9): `(text|bg|border|ring|divide|fill|stroke|outline|from|to|via)-(white|black|neutral|gray|slate|zinc|stone|red|green|amber|yellow|blue|emerald|orange|sky|indigo|rose)` with optional shade and opacity in any `.vue` under `src/`; it fails now with about 170 hits

### Implementation

- [ ] T033 [US4] Implement `src/lib/settings/colorScheme.ts` (`parseColorScheme`, `effectiveColorScheme`, `isDark`); make T031 pass
- [ ] T034 [US4] Implement `src/composables/useColorScheme.ts` per contracts §4: module-level state, `loadAsync(deviceUuid)` reading `appearance.color_scheme` for both scopes via `usePreferences`, `setAsync(scope, scheme | null)`, applying the `dark` class on `document.documentElement` and following `matchMedia('(prefers-color-scheme: dark)')` while `system` applies
- [ ] T035 [P] [US4] Create `src/plugins/colorScheme.client.ts` that applies `system` at start (before unlock, FR-014); call `useColorScheme().loadAsync(...)` in `src/pages/workspace/[instance].vue` after opening, leaving `system` on a read error
- [ ] T036 [US4] Add `settings.appearance.setColorScheme` and `settings.appearance.clearColorScheme` to `src/lib/actions/settingsActions.ts` (scope `settings.device`, agent-callable, result without `null` per contracts §3), handlers in `src/stores/settingsActionHandlers.ts` calling `useColorScheme().setAsync`, and `colorScheme` in `settings.get`; i18n `actions.settings.appearance.*`
- [ ] T037 [US4] Create `src/components/settings/ColorSchemeSetting.vue`: selects "Dieses Gerät" (Wie alle Geräte / Hell / Dunkel / System) and "Alle Geräte" (System / Hell / Dunkel), saved on selection through the two actions; i18n `settings.colorScheme.*`
- [ ] T038 [US4] In its own commit (`refactor(ui): use theme colors instead of palette colors`), replace the palette colors in the ~28 Vue files per the mapping in research R9; make T032 pass; `check:templates`, `typecheck`, `lint`, `format:check` green
- [ ] T039 [US4] Operator: run quickstart S11 and S12, looking at chat, settings, onboarding and dialogs in dark mode

---

## Phase 7: User Story 5 - Föderation in den Einstellungen (P2)

**Goal**: The federation app is gone; the category lists the vault's devices.

**Independent Test**: quickstart S13, S13a, S14.

### Tests first

- [ ] T040 [P] [US5] Create `src-tauri/src/storage/known_devices_tests.rs` (declared in `src-tauri/src/storage/mod.rs` like `maintenance_tests`; real vault like `maintenance_tests.rs`): `list_devices` returns this installation's row, skips the `VAULT_SCOPE_UUID` row, returns a second inserted device with `alias = NULL` as `None`
- [ ] T041 [P] [US5] Create `src-tauri/src/device/commands_tests.rs` (declared via `#[cfg(test)] #[path]` in `src-tauri/src/device/mod.rs`): the pure ordering helper puts the current device first, then the others by name case-insensitively, unnamed devices last, and marks exactly one `is_current`
- [ ] T042 [P] [US5] Add `resolveAppAlias` tests to `scripts/check-settings.ts`: `system.federation` → `{ appId: 'system.settings', at: '/federation' }`; `system.chat` → `{ appId: 'system.chat', at: null }`; `system.federation` is not in `WM_APPS`

### Implementation

- [ ] T043 [US5] Implement `list_devices(conn)` in `src-tauri/src/storage/known_devices.rs` (research R12); make T040 pass
- [ ] T044 [US5] Implement the command `list_vault_devices` and `VaultDevicePayload { vault_device_uuid, alias, is_current }` (serde camelCase) with the ordering helper in `src-tauri/src/device/commands.rs`, register it in `src-tauri/src/lib.rs`; make T041 pass; `cargo fmt --check`, `lint:rust`, `cargo test` green
- [ ] T045 [US5] Add `VaultDevice` and `listVaultDevicesAsync()` to `src/composables/useDevice.ts`; add the read action `settings.devices.list` (scope `settings.read`, effect `read`) to `settingsActions.ts` with a handler that omits a `null` alias; i18n `actions.settings.devices.list`
- [ ] T046 [US5] Create `src/components/settings/FederationView.vue`: one row per device with name or "Unbenanntes Gerät" and the mark "Dieses Gerät", loaded when the category opens; i18n `settings.federation.*`
- [ ] T047 [US5] Add `LEGACY_APP_ALIASES` and `resolveAppAlias` to `src/lib/wm/apps.ts` and remove `system.federation` from `WM_APPS`; resolve the alias before `knownApp` in `wm.app.open` and `wm.tab.new` in `src/stores/wmActionHandlers.ts` (an input `at` wins over the alias `at`); make T042 pass
- [ ] T048 [US5] Remove the federation entry from `src/components/wm/appRoutes.ts`, delete `src/components/apps/FederationApp.vue`, its two tests in `scripts/check-vault-lifecycle.ts` and the i18n key `wm.apps.federation`; point `src/pages/federation/[instance].vue` at `?open=system.settings&at=/federation`
- [ ] T049 [P] [US5] Add a note to `specs/015-workspace-shell/spec.md` next to FR-003/FR-004: the federation app is replaced by the settings category "Föderation" (spec 023)
- [ ] T050 [US5] Operator: run quickstart S13, S13a and S14

---

## Phase 8: User Story 6 - Schmale Fenster (P2)

**Goal**: The sidebar shrinks to icons in narrow windows.

**Independent Test**: quickstart S15.

- [ ] T051 [US6] Make the settings frame a container (`@container` on the root of `SettingsApp.vue`); in `Sidebar.vue` show icon and name from `@2xl` (42rem) up and only icons with the name as `UiButton` tooltip below, widths 16rem and 3.5rem (research R4); check that no view scrolls horizontally at 360 px
- [ ] T052 [US6] Operator: run quickstart S15

---

## Phase 9: Polish & Cross-Cutting

- [ ] T053 [P] Add the terms "Einstellungskategorie", "Farbschema" and "Geräte der Vault" to `CONTEXT.md`
- [ ] T054 Run the automated part of `quickstart.md` in full: `check:settings`, `check:templates`, `check:wm-state`, `check:wm-navigation`, `check:chat-state`, `check:vault-lifecycle`, `typecheck`, `typecheck:scripts`, `lint`, `format:check`, `cargo fmt --check`, `lint:rust` (both feature sets), `cargo test`, `test:e2e`; record the counts here
- [ ] T055 Operator: run quickstart S16 and S18
- [ ] T056 After merge: refresh the graphify graph on `main` and rerun the queries from research.md (flagged there because the worktree snapshot predates the window manager work)

---

## Dependencies & Execution Order

- Setup (T001–T003) → Foundational (T004–T009) → user stories.
- US1 (T010–T025) is the MVP and comes first; US2–US6 build on its frame (`SettingsApp.vue`, `Sidebar.vue`) and can then run in any order.
- Within US1: T019 before T021 and T023 (they read `models.downloads`); T020–T023 before T024.
- US4: T031 and T032 first; T033 → T034 → T035/T036 → T037; T038 last (it touches files of other stories, so run it after they are done or rebase it).
- US5: tests T040–T042 first; T043 → T044 → T045 → T046; T047 before T048.
- Polish after all stories.

## Parallel Opportunities

- T002, T003 in Setup.
- T008 next to T006/T007.
- In US1: T011–T018 touch different files; T021–T023 after T019.
- US4 T031/T032, US5 T040–T042 are independent test files.
- US2, US3, US5 and US6 can proceed in parallel once US1 is done; US4's T038 should come last.

## Implementation Strategy

1. Setup and Foundational: registry and tests green.
2. US1: the frame and every existing setting in its category, saved without buttons. Stop and let the operator run S1–S3, S17.
3. US2 and US3: header back, sub-view hints, context links.
4. US4: color scheme, then the theme-color migration as its own commit.
5. US5: device list and federation removal.
6. US6: narrow windows.
7. Polish: full checks, e2e, remaining manual scenarios.
