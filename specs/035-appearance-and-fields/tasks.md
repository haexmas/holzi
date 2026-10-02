---
description: 'Task list for spec 035-appearance-and-fields'
---

# Tasks: Darstellung und Eingabefelder

**Input**: Design documents from `/specs/035-appearance-and-fields/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/appearance-actions.md](./contracts/appearance-actions.md), [contracts/appearance-file.md](./contracts/appearance-file.md), [contracts/token-map.md](./contracts/token-map.md), [quickstart.md](./quickstart.md)

**Tests**: Included. The constitution requires an executable check for non-trivial logic: the new `pnpm check:appearance` (`scripts/check-appearance.ts`, pure, in CI), `pnpm check:fields` (`scripts/check-fields.ts`, in CI), the extended `check:settings` and `check:agent-actions`, and the end-to-end scenarios `fields-basic`, `appearance-basic`, `appearance-sync-two-devices` and the extended `settings-color-scheme`. Write each test task before its implementation and see it fail first.

**Organization**: Grouped by user story, in priority order (P1: US1, US2, US3; P2: US4; P3: US5). US2 is delivered in two stages (research plan "Phasen"): the core (accent, window and container background) in Phase 4 and the COSMIC rest (text tint, component tint, window hint) in Phase 8. The branch for the code starts from `main` after the spec PR (#225). Modules under `src/lib/**` import siblings relatively with a `.ts` suffix so the Node check scripts can load them. Every file stays ≤ 500 lines. Commits follow Conventional Commits and carry no agent attribution. Before authoring any new named artifact, consult graphify (T002). Frontend commands run as `nix develop --command sh -c '…'` (plain `pnpm` is not on the path outside the shell). Test data uses invented values only. UI text is German and English in lockstep, says "Sitzung" and "Window Manager (wm)", and has no save or apply buttons.

**Shipping note**: One PR per stage is fine (Stage 1 = Phase 3; Stage 2 = Phases 4–7; Stage 3 = Phase 8); the stages are independent of each other except that Stage 2 and 3 need Phase 2. Phase 8 may be split off without delaying the others (spec assumption).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: User story from spec.md (US1–US5)

---

## Phase 1: Setup

- [ ] T001 Prepare a worktree for the code on a new topic branch from `main` (for example `.worktrees/035-appearance-impl`): real `pnpm install --frozen-lockfile` inside `nix develop` (no symlinked `node_modules`), reflink the Rust build cache only if a build is needed (`cp -a --reflink=always` from a sibling worktree's `src-tauri/target`); record baseline counts of `check:settings`, `check:agent-actions`, `check:templates`, `check:wm-navigation` in this task's note. Confirm `nuxt.config.ts` extends `github:haex-space/haextension/packages/haex-ui#2dcb8bc`
- [ ] T002 [P] Before the first new name, run `graphify query` for: "color scheme apply dark class system", "preferences vault get set", "settings action handlers register", "settings group row layout", "contrast text background e2e helper", "file dialog open save", "window active border"; prefer extending an existing candidate and note which candidates were evaluated and why they did not match. Known candidates: `useColorScheme`, `usePreferences`, `settingsActions.ts`/`settingsActionHandlers.ts`, `settings/Group.vue`/`Row.vue`, `scripts/e2e/lib/settings.ts` (`textContrast`, `backgroundContrast`, `choose`), `passwords/ImportWizard.vue` (dialog use)
- [ ] T003 [P] Upstream A1 in `haex-space/haextension` (own worktree there, PR through the `haexmas` fork, no agent attribution): `UiSelect` gets `groups`, `data-value` on its items, an empty value, and forwards `data-testid` and other attributes to the trigger (research R6). Not blocking: while A1 is not merged, `components/settings/Select.vue` stays in the allowlist of T005. If A1 merges during the work, bump the pin in `nuxt.config.ts` and remove that allowlist entry (T019)

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The two check scripts and their CI wiring, so every later task has something to run against.

- [ ] T004 Add `"check:appearance": "node --test scripts/check-appearance*.ts"` and `"check:fields": "node --test scripts/check-fields.ts"` to `package.json` and two steps "Check appearance" and "Check fields" running `corepack pnpm check:appearance` and `corepack pnpm check:fields` after "Check settings" in `.github/workflows/ci.yml`; both scripts may start with one placeholder test that is replaced by T005 and T020

**Checkpoint**: `pnpm check:appearance` and `pnpm check:fields` run (placeholders), CI wiring present.

---

## Phase 3: User Story 1 - Eingabefelder sehen überall gleich aus (P1) 🎯 MVP

**Goal**: Every text field, password field, multi-line field and select in `src/` is the haex-ui field with floating label, primary ring, error text and clear/copy where it fits; behaviour and `data-testid`s unchanged.

**Independent Test**: [quickstart.md](./quickstart.md) Stage 1: in every area click, type, clear, trigger an error, open a select; `check:fields` finds no old field kind outside the allowlist; existing e2e scenarios pass unchanged.

### Tests for User Story 1 (write first, see them fail)

- [ ] T005 [US1] Write `scripts/check-fields.ts` (`node:test`, relative `.ts` imports): scans `src/**/*.vue` for `ShadcnInput`, `ShadcnTextarea`, `ShadcnSelect`, `<input`, `<textarea`, `<select` and reports every hit not in the allowlist; `<input` with `type="radio|checkbox|range|file|color"` is not a text field and is skipped. Allowlist inside the file, each entry with a reason (FR-001, research R6): `src/components/chat/Composer.vue` (grows with its content, own key handling) and `src/components/settings/Select.vue` (grouped options, empty value, `data-value` for e2e; remove when T003 is merged). Add a second test that proves the scanner itself on in-memory samples (old kind found, allowlisted path skipped, `type="checkbox"` skipped). Expected failing now: ~25 files
- [ ] T006 [P] [US1] Add `scripts/e2e/scenarios/fields-basic.test.ts` and helpers in `scripts/e2e/lib/fields.ts` (pattern `settings.ts`): in the alias step of the onboarding and in the passwords editor a click on a field floats its label (`[data-slot="floating-label"]` has the floated classes), the focused field shows the primary ring colour (read the computed `border-color` equal to `--primary`), an empty required field shows the error text and `aria-invalid`, clear (X) empties and copy puts the value in the clipboard through the app's clipboard path; at 360 px the label is truncated and the field stays usable (FR-002 to FR-005, edge cases)

### Implementation for User Story 1

- [ ] T007 [US1] Add the labels of the helper buttons to `src/i18n/locales/de.json` and `en.json` under `fields.*` (`clear`, `copy`, `copied`, `show`, `hide`) in lockstep (FR-009) and a composable `src/composables/useFieldLabels.ts` returning the `labels` objects for `UiInput` (`{copy, copied, clear}`) and `UiInputPassword`/`UiTextarea`; check graphify (T002) and reuse the labels already passed to `UiInputPassword` in `passwords/EntryEditor.vue` and `passwords/ImportWizard.vue` instead of duplicating them
- [ ] T008 [US1] Migrate `src/components/passwords/EntryEditor.vue` (7 `ShadcnInput`, 1 `ShadcnTextarea`, 1 `UiInputPassword`): `UiInput` with `label`, `clearable` where sensible, `copyable` on username, URL and (locked) password; keep every `data-testid` and `id`, keep `autofocus`, Enter/Escape behaviour and the unsaved-changes guard; the password field keeps hold-to-show; use `label-bg` per surface (page: default, dialogs: `var(--popover)`)
- [ ] T009 [P] [US1] Migrate `src/components/passwords/GeneratorPanel.vue` (3 `ShadcnInput`, native `<select>`, range stays native), `FolderDialog.vue` (2), `KeyValues.vue` (2), `Passkeys.vue`, `TagManager.vue`, `Attachments.vue` (1 each); the preset `<select>` becomes `UiSelect` with `aria-label`; keep `data-testid`s and the generator's copy button behaviour
- [ ] T010 [P] [US1] Migrate `src/components/passwords/SelectionToolbar.vue` (two native `<select>` to `UiSelect`, one `ShadcnInput`), `passwords/Toolbar.vue` (search field: `UiInput` without label, `clearable`, placeholder kept) and the password field of the import wizard in `passwords/ImportWizard.vue` (`UiInputPassword` with `label`)
- [ ] T011 [P] [US1] Migrate `src/components/settings/AliasSetting.vue`, `ConnectDelegateProvider.vue`, `ServerList.vue` to `UiInput` (fields inside `SettingsGroup` boxes set `label-bg="var(--muted)"`, research R6) and `settings/Toolbar.vue` (search, no label, `clearable`); `settings/Select.vue` and the native `OptionRow.vue` radio stay (allowlist, T005); keep save-on-blur and Enter-to-save behaviour
- [ ] T012 [P] [US1] Migrate the chat selects `src/components/chat/ComposerControl.vue` and `ComposerSettingsPopover.vue` (8 hits: `ShadcnSelect` to `UiSelect` without label, radio `<input>` stays) and the thread rename in `ThreadSidebar.vue` (`UiInput` without label); `Composer.vue` stays (allowlist); keep the sentinel value for "no override" in the popover
- [ ] T013 [P] [US1] Migrate `src/components/models/HuggingFaceSearch.vue` and `HuggingFaceFilePicker.vue` (`UiInput`, search with `clearable`)
- [ ] T014 [P] [US1] Migrate the setup screens `src/components/onboarding/AliasStep.vue`, `CreateSheet.vue` and `LinkSheet.vue` (3 fields, password fields as `UiInputPassword`) with labels instead of separate `<label>` elements where one exists; keep `autofocus`, `id`s and `data-testid`s
- [ ] T015 [US1] Run `check:fields`, `check:templates` (`scripts/check-vue-templates.ts`), `typecheck`, `lint`, `format:check` in the shell and fix findings; then run the existing e2e scenarios touching fields against a freshly built debug app (`passwords-basic`, `passwords-narrow-window`, `settings-save-on-selection`, `settings-search`, `settings-narrow-window`, the onboarding/chat scenarios in `scripts/e2e/scenarios`) and `fields-basic`; every failure is a lost behaviour (FR-007, SC-002) to fix in the component, not in the test. Mark T005 green and open one PR for Stage 1

**Checkpoint**: `pnpm check:fields` green with only the two allowlisted files; all existing e2e scenarios and `fields-basic` pass.

---

## Phase 4: User Story 2 - Akzentfarbe und Hintergrund wählen (P1), Stufe 2

**Goal**: Settings → Darstellung offers accent swatches plus custom colour, window and container background, saved on selection, applied at once in every window, synced across devices.

**Independent Test**: [quickstart.md](./quickstart.md) Stage 2 items 1–3, 8, 9.

### Tests for User Story 2 (write first, see them fail)

- [ ] T016 [US2] Write `scripts/check-appearance.ts` (`node:test`, relative `.ts` imports, no Nuxt): colour maths with known values (oklch ↔ sRGB round trip, `#ffffff`, `#000000`, `#00ffff`), WCAG contrast of known pairs (21:1 for black on white, 1:1 for equal colours), `parseAppearance` (lenient: bad JSON, unknown `v` → default; one bad field → that field default; unknown fields ignored), `serialize` round trip, preset ids of every row exist and have a language key in `de.json` and `en.json` (flatten and compare), `derive(default, 'light'|'dark')` equals the values in `src/assets/css/tailwind.css` (parse the two blocks `:root` and `.dark` out of the file; research R4, data-model "Standardwerte"), and the contrast matrix of [contracts/token-map.md](./contracts/token-map.md): every accent preset and extreme custom accent (chroma 0.4, hue 0°–345° step 15°, greys, `#000000`, `#ffffff`) × both schemes × every pair, and every window and container tint (presets and extreme custom) × both schemes × every pair, all at the thresholds 4.5 and 3 (FR-016, FR-017, FR-022, SC-004). Keep file size ≤ 500 lines by splitting into `check-appearance-derive.ts` and `check-appearance-file.ts` when needed

### Implementation for User Story 2

- [ ] T017 [P] [US2] Create `src/lib/appearance/oklch.ts` (OKLCH ↔ linear sRGB per Ottosson, gamut clamp by lowering chroma at fixed lightness, `parseHex`, `toHex`, `toOklchString`) and `src/lib/appearance/contrast.ts` (relative luminance, `contrast(a, b)`, `solveLightness(...)` returning the smallest lightness change meeting the thresholds, research R3, R4). Add `ponytail:` where the lightness search is a bisection with a fixed step (ceiling: 1e-3 resolution)
- [ ] T018 [P] [US2] Create `src/lib/appearance/presets.ts`: the accent row (`teal` 180 default, `blue` 255, `violet` 300, `pink` 350, `red` 25, `orange` 55, `yellow` 95, `green` 150, `neutral` chroma 0) and the tint row (`neutral`, `warm` 60/0.02, `cool` 240/0.02, `green` 150/0.02, `violet` 300/0.02, `rose` 10/0.02) of [data-model.md](./data-model.md), each with `id`, hue, chroma and a language key `settings.appearance.preset.<id>`
- [ ] T019 [US2] Create `src/lib/appearance/schema.ts`: the `Appearance` type (`v`, `accent`, `window`, `container`, `text`, `component`, `windowHint`), `DEFAULT_APPEARANCE`, `parseAppearance(raw: unknown)` (lenient, FR-020), `parseAppearanceFile(text)` (strict, all or nothing, error keys of [contracts/appearance-file.md](./contracts/appearance-file.md), size limit 16 KiB), `serializeAppearance`, `exportFile(appearance, scheme)`. Keys: `APPEARANCE_KEY = 'appearance.theme'`. Reuses `parseColorScheme` from `src/lib/settings/colorScheme.ts`
- [ ] T020 [US2] Create `src/lib/appearance/tokens.ts` (token names per control, [contracts/token-map.md](./contracts/token-map.md) table, default lightness per scheme and per token as constants) and `src/lib/appearance/derive.ts`: `derive(appearance, scheme) → { tokens, adjustments }` implementing the order of the contract (accent lightness solved against `--background`, `--card`, `--popover`, `--sidebar`, `--muted`; foreground colour chosen between `0.985` and `0.145`; tints applied with default lightness and capped chroma; every pair measured, chroma lowered first, then foreground lightness; adjustments recorded with control, kind and reason). Make `check-appearance` of T016 pass for accent and the window and container tints
- [ ] T021 [US2] Update the default values in `src/assets/css/tailwind.css` to what `derive(DEFAULT_APPEARANCE, scheme)` yields (research R4): light accent (`--primary`, `--primary-foreground`) darker or with dark text, `--muted-foreground` light `L ≈ 0.53`, and `--ring`, `--sidebar-primary`, `--sidebar-primary-foreground`, `--sidebar-ring` following the accent in both schemes; record the old and new values and the measured contrasts of the defaults in the note of this task for the PR text. Make the "derive equals tailwind.css" test of T016 pass
- [ ] T022 [US2] Create `src/composables/useAppearance.ts` after the pattern of `useColorScheme.ts` (one state per process): `appearance`, `tokens`, `adjustments`, `loadAsync()` (reset to defaults, read `APPEARANCE_KEY`, apply), `refreshAsync()` (no intermediate state), `setAsync(partial)`, `resetAsync()`, `importAsync(text)`, `exportText()`, `clear()` (remove all inline properties); `apply()` sets the tokens with `documentElement.style.setProperty` and re-runs when the scheme changes (listen to the same observer `useColorScheme` already runs; extend `useColorScheme` only by exposing a change hook, no second observer). Writes through `usePreferences().setPrefAsync({ kind: 'vault' }, APPEARANCE_KEY, json)`; a write error keeps the old state and rethrows
- [ ] T023 [US2] Wire the composable in: `src/plugins/colorScheme.client.ts` calls `useAppearance().start()`; `src/pages/workspace/[instance].vue` loads it where `colorScheme.loadAsync` already runs (before the workspace paints) and registers `onVaultTablesChanged(['preferences'], appearance.refreshAsync)` next to the colour scheme; the lock and vault change call `clear()` so no earlier vault's colours show (spec edge cases, research R5)
- [ ] T024 [US2] Add the four actions of [contracts/appearance-actions.md](./contracts/appearance-actions.md) to `src/lib/actions/settingsActions.ts` (`settings.appearance.set`, `.reset`, `.export`, `.import`; English descriptions; input and result schemas; `scope: 'settings.device'`; `effect` write/read as in the contract) and the handlers in `src/stores/settingsActionHandlers.ts` using `useAppearance`; extend `settings.get` with `appearance`. Add the error keys `settings.appearance.invalid`, `.failed` and `.import.*` to `src/composables/useErrorString.ts` mapping. Extend `scripts/check-agent-actions.ts` expectations (new actions have descriptions and schemas) and `scripts/check-settings.ts` (`settings.get` includes `appearance`)
- [ ] T025 [P] [US2] Add `src/components/settings/ColorSwatches.vue`: a row of swatch buttons (radio semantics, roving tabindex, `aria-label` from the preset key, checked mark), the user's own colour as an extra swatch when set, and a "+" tile that opens a popover with `<input type="color">` and a hex `UiInput` (validated, last valid colour stays while typing, edge case "halbe Eingabe"); emits `select({preset}|{custom})`; `data-testid`s `appearance-swatch-<id>`, `appearance-swatch-custom`, `appearance-swatch-add`
- [ ] T026 [US2] Add `src/components/settings/AccentSetting.vue` and `TintSetting.vue` (one row each: title, `ColorSwatches`, "angepasst" note with the reason from `adjustments`, FR-017) using `SettingsRow`, saved on selection through the actions (`useActionOrThrow('settings.appearance.set')`), errors in a `role="alert"` line like `ColorSchemeSetting.vue`; `TintSetting` takes the control (`window`/`container`; Phase 8 adds `text`/`component`)
- [ ] T027 [US2] Create `src/components/settings/AppearanceView.vue` (the category "Darstellung": the colour-scheme row from `ColorSchemeSetting.vue` first, then accent, window, container; `ColorSchemeSetting.vue` is reduced to the row or folded in) and point `src/components/wm/appRoutes.ts` (line 39) to it; register the search keys of the new rows in `src/lib/settings/registry.ts` (`categoryLocation('appearance', [...])`) and add `settings.appearance.*` keys (titles, preset names, "angepasst" reasons, errors) to `src/i18n/locales/de.json` and `en.json` in lockstep; run `check:settings` and `check:templates`
- [ ] T028 [US2] Add `scripts/e2e/scenarios/appearance-basic.test.ts` (part 1): choose accent blue, `--primary` on `<html>` changes at once and the focused field's border shows it (FR-003, FR-014); a custom colour through `appearance-swatch-add`; restart the app (existing relaunch helper) and the value persists (SC-003); the vault preference `appearance.theme` holds the JSON; another vault does not show it (use the second-instance helper if present, else document and skip with a note in the task)
- [ ] T029 [P] [US2] Add `scripts/e2e/scenarios/appearance-sync-two-devices.test.ts` on the multi-device rig of spec 033 (pattern `passwords-sync-two-devices.test.ts`): a change of the accent on device A is on device B after the next sync without a restart; two devices change different controls at once and both end with the same, younger appearance (spec edge case, research R1) (SC-005)

**Checkpoint**: `pnpm check:appearance` (accent, window, container) green; `appearance-basic` part 1 and the sync scenario pass; `settings-color-scheme` unchanged green.

---

## Phase 5: User Story 3 - Hell, Dunkel oder automatisch (P1)

**Goal**: The scheme choice keeps working as in 023 and every derived value follows it, also live when the system changes.

**Independent Test**: [quickstart.md](./quickstart.md) Stage 2 item 5.

- [ ] T030 [US3] Extend `scripts/e2e/scenarios/appearance-basic.test.ts` and `settings-color-scheme.test.ts`: with a chosen accent switch scheme to light, dark and system; `--primary` changes lightness (derived for the scheme) while the hue stays; with system dark → light (the existing scenario's OS-scheme switch) `--primary` follows without restart (FR-018, US3 scenarios 1–3)
- [ ] T031 [US3] Make sure `useAppearance` re-derives on every scheme change including the system's: add a check in `scripts/check-appearance.ts` that `derive` is called for both schemes with the same appearance and yields different lightness for the accent but the same hue; fix `useColorScheme`'s hook (T022) if the e2e of T030 shows a missed change

**Checkpoint**: e2e of T030 green; `check:appearance` green.

---

## Phase 6: User Story 4 - Lesbar bleiben und zurücksetzen (P2)

**Goal**: A colour that would break readability is adjusted to the nearest allowed value with a visible note; "Auf Standard zurücksetzen" restores all values after a confirmation.

**Independent Test**: [quickstart.md](./quickstart.md) Stage 2 items 4 and 6.

- [ ] T032 [US4] Add the reset button to `src/components/settings/AppearanceView.vue` ("Auf Standard zurücksetzen") with a confirmation dialog (existing dialog component of the app, `UiButton`), calling `settings.appearance.reset`; the colour scheme stays; `data-testid`s `appearance-reset` and `appearance-reset-confirm`; German and English texts in lockstep. Extend `check-appearance.ts` with the reset result equal to `DEFAULT_APPEARANCE`
- [ ] T033 [US4] Show the adjustment note in `AccentSetting.vue`/`TintSetting.vue`: an own accent colour that needs a different lightness or chroma in the active scheme, and a tint whose chroma was lowered, show "angepasst" with the reason key; a preset accent shows none (data-model "Abgeleiteter Zustand"); the stored choice stays unchanged and yields the same adjusted value on every open (US4 scenario 2). Add cases to `check-appearance.ts` (preset accent → no adjustments; `#ffffaa` → adjustment present in light, stored value unchanged)
- [ ] T034 [US4] Add the contrast measurements to `settings-color-scheme.test.ts` and `appearance-basic.test.ts` against the running app (FR-022): text of the title and select values ≥ 4.5 (existing), primary button label on `--primary` ≥ 4.5, the focused field border and the off/on switch track ≥ 3 against their box (use `textContrast`/`backgroundContrast` from `scripts/e2e/lib/settings.ts`), for the defaults and for one custom accent (`#ffffaa`); the full matrix stays in `check:appearance`; add the reset flow (change, reset, confirm) and assert the defaults return and the scheme stays (US4 scenario 3)

**Checkpoint**: e2e green; `check:appearance` green including adjustments and reset.

---

## Phase 7: User Story 5 - Darstellung exportieren und importieren (P3)

**Goal**: Export to a file and import from a file, all or nothing, with a readable error.

**Independent Test**: [quickstart.md](./quickstart.md) Stage 2 item 7.

- [ ] T035 [US5] Extend `scripts/check-appearance-file.ts` (or the check file of T016): a valid file round-trips (export → import equals the state, SC-006); every error row of [contracts/appearance-file.md](./contracts/appearance-file.md) yields its key and writes nothing (not JSON, size above 16 KiB, wrong `format`, unknown `v`, bad `colorScheme`, preset not in its row, `custom` not six-digit hex, missing field, additional field, non-boolean `windowHint`); a file with one valid and one invalid value changes nothing (US5 scenario 3)
- [ ] T036 [US5] Add `src/components/settings/AppearanceFileButtons.vue` ("Importieren" and "Exportieren" in the header row of the category as in the COSMIC dialog): export through `settings.appearance.export` and `save()` of `@tauri-apps/plugin-dialog` (default name `holzi-appearance.json`, filter extension `json`), import through `open()` and `plugin-fs` `readTextFile` limited to 16 KiB, then `settings.appearance.import`; errors shown with the key of the contract and the field name, nothing changes on error; add `dialog:allow-save` only if not present and fs read scope as the passwords import already uses (check `src-tauri/capabilities/default.json`); mobile uses the same picker (research R7); `data-testid`s `appearance-export`, `appearance-import`
- [ ] T037 [US5] Extend `appearance-basic.test.ts` with export and import: drive `settings.appearance.export` and `.import` through the e2e `invoke` path (the OS file dialog is not drivable), change, reset, import the exported text and compare the whole state; import a broken text and assert the error key and an unchanged state (SC-006)

**Checkpoint**: file checks and e2e green.

---

## Phase 8: COSMIC-Rest (User Story 2, Stufe 3)

**Goal**: Text tint, component tint and the window hint (FR-013 rest, FR-024). May be split off into its own PR without delaying Phases 3–7.

**Independent Test**: [quickstart.md](./quickstart.md) Stage 3.

- [ ] T038 [US2] Extend `scripts/check-appearance.ts` first (fails): the contrast matrix for every `text` and `component` tint (presets and extreme custom: chroma at the cap, hue step 15°, greys, `#000000`, `#ffffff`) × both schemes × all pairs of [contracts/token-map.md](./contracts/token-map.md), including combinations (all four tints at extreme at once for three hues); lowering order chroma first, then foreground lightness; the window hint produces no token change but is part of `Appearance` and the file
- [ ] T039 [US2] Make T038 pass in `src/lib/appearance/derive.ts`/`tokens.ts`: text tint on the text tokens, component tint on `--secondary`, `--muted`, `--accent`, `--input`, `--border`, `--sidebar-accent`, `--sidebar-border`; chroma caps per control (`window` 0.03, `container` 0.03, `component` 0.04, `text` 0.04)
- [ ] T040 [US2] Add the rows "Texttönung" and "Komponententönung" to `src/components/settings/AppearanceView.vue` (`TintSetting` with `text` and `component`), the keys in `de.json` and `en.json`, search keys in `registry.ts`; and the switch "Akzentfarbe als Hinweis für das aktive Fenster" (`SettingsRow` plus the switch used in `SessionRestoreSetting.vue`, `data-testid="appearance-window-hint"`) saved on selection through `settings.appearance.set`
- [ ] T041 [US2] Window hint in `src/components/wm/Window.vue` (line ~153): `:class="active ? (hint ? 'border-primary' : 'border-foreground/40') : 'border-border'"` with `hint` from `useAppearance`; only the active window can carry it (`wm.focusWindow`); the border measures ≥ 3:1 through the accent solve (T020)
- [ ] T042 [US2] Extend `appearance-basic.test.ts`: change the text and component tint and check `--foreground` and `--muted` change and the app stays readable (the existing contrast helpers); switch the hint on, open two windows, only the active one has `border-color` equal to `--primary`, focus the other and the border moves (FR-024, US2 scenarios 6 and 7)

**Checkpoint**: `check:appearance` green with all four tints and combinations; e2e green.

---

## Phase 9: Polish & Cross-Cutting

- [ ] T043 [P] Add glossary entries to `CONTEXT.md`: "Darstellung" (all appearance settings of a vault), "Tönung" (hue and capped chroma applied to a scheme's default surfaces), "Farbfeld" (a preset or a custom colour), "Fensterhinweis"; keep "Sitzung" and "Window Manager (wm)" wording
- [ ] T044 [P] Add a row to `plans/README.md` for the follow-ups: more Darstellung options (font size, density, style/corner radius as in COSMIC "Stil"), per-extension theming for extensions with their own UI, A1 for `UiSelect`
- [ ] T045 Keep the locale files in lockstep: add a check to `scripts/check-appearance.ts` (or the passwords lockstep check) that `settings.appearance.*` and `fields.*` exist with the same keys in `de.json` and `en.json`
- [ ] T046 Run the CI-equivalent commands of [quickstart.md](./quickstart.md) in `nix develop --command sh -c '…'`: `check:appearance`, `check:fields`, `check:settings`, `check:agent-actions`, `check:wm-navigation`, `check:wm-state`, `check:chat-state`, `check:vault-data`, `check:passwords`, `check:templates`, `typecheck`, `typecheck:scripts`, `lint`, `format:check`; build the debug app and run `fields-basic`, `appearance-basic`, `appearance-sync-two-devices`, `settings-color-scheme` and the existing scenarios that touch fields; write down what was run and what was not
- [ ] T047 Complete [quickstart.md](./quickstart.md) Stages 1–3 by hand on the real app (also a 360 px window and, if possible, one phone for the colour picker and file dialog); check SC-001 and SC-007 (a person without prior knowledge finds the Darstellung and sets a colour in under 30 seconds); update `specs/035-appearance-and-fields/checklists/requirements.md` notes with the result
- [ ] T048 Prepare the PR texts, one per stage: call out the points beyond the spec wording — the light default accent and `--muted-foreground` change with old and new values and measured contrasts (T021), `--ring` and `--sidebar-primary` now following the accent, own colour maths instead of a library, one sync cell for the whole appearance (concurrent edits keep only the younger), the reset button with confirmation, `check:fields` allowlist entries with reasons, A1 status

## Dependencies & Execution Order

- **Phase 1** → **Phase 2** (blocking). T003 is independent upstream work and only matters for the allowlist entry.
- **US1 (Phase 3)** needs Phase 2 only and does not depend on the appearance library; it may ship first as Stage 1.
- **US2 core (Phase 4)** needs Phase 2. T017, T018 in parallel; T019 after T017/T018; T020 after T017–T019; T021 after T020; T022 after T019/T020; T023 after T022; T024 after T022; the UI tasks (T025–T027) after T024; e2e T028, T029 after T027.
- **US3 (Phase 5)** needs Phase 4. **US4 (Phase 6)** needs Phase 4 (T033 also needs the UI rows). **US5 (Phase 7)** needs T019 and T024. **Phase 8** needs Phase 4 and may follow US4/US5 or run in parallel to them after T027.
- Tasks that edit `src/i18n/locales/*.json`, `src/components/settings/AppearanceView.vue`, `src/lib/actions/settingsActions.ts` or `scripts/check-appearance.ts` run one after the other across stories.
- Within a story: tests first (they fail), then implementation.

## Parallel Opportunities

- Phase 1: T002 and T003 in parallel after T001.
- US1: T006 parallel to T007; T009 to T014 touch different files and run in parallel after T007 and T008 (T008 sets the pattern).
- US2: T017 and T018 in parallel; T025 parallel to T022–T024; T029 parallel to T028.
- US4 and US5 touch different files and run in parallel after Phase 4.
- Phase 9: T043 and T044 in parallel.

## Implementation Strategy

1. **MVP = Phases 1–3 (US1)**: all fields consistent, `check:fields` guards them. Stop and run the quickstart Stage 1; open the Stage 1 PR.
2. **Phases 4–7 (US2 core, US3, US4, US5)**: the library, the composable, the actions, the Darstellung group, reset, export and import. Stage 2 PR after T046 for these phases.
3. **Phase 8 (COSMIC rest)**: text tint, component tint, window hint; split off if it grows.
4. Finish with Phase 9. Do not merge a stage before its checks are green and its PR text names the deviations (T048).
