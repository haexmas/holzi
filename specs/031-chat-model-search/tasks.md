---
description: 'Task list for 031-chat-model-search'
---

# Tasks: Modellsuche im Chat

**Input**: Design documents from `/specs/031-chat-model-search/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [quickstart.md](./quickstart.md)

**Tests**: Included — plan.md and quickstart.md call for a `node --test` check
of the pure filter function and a new Tauri e2e scenario.

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [ ] T001 Add `fuse.js` to `dependencies` in `package.json` and run
      `pnpm install` (research.md R1).
- [ ] T002 [P] Add the new i18n keys
      `chat.composer.settingsPopover.modelSearch.placeholder` and
      `chat.composer.settingsPopover.modelSearch.noResults` to
      `src/i18n/locales/de.json` and `src/i18n/locales/en.json` in lockstep
      (spec FR-009).

## Phase 2: Foundational (blocking)

**⚠️ CRITICAL**: Both user stories build directly on this.

- [ ] T003 Create `filterModelGroups(groups: ModelGroup[], query: string):
ModelGroup[]` in new `src/lib/chat/modelSearch.ts`, using `fuse.js`
      over `providerName` and `models[].name` (data-model.md); empty query
      returns `groups` unchanged; a group with no matching models is
      dropped entirely; surviving models keep their original relative
      order (no cross-provider re-ranking, spec FR-004). Export the
      `ModelGroup` type from here (or re-export from
      `ComposerSettingsPopover.vue`) so the check script can import it
      without Vue.
- [ ] T004 [P] Create `scripts/check-chat-model-search.ts` (pattern:
      `scripts/check-settings.ts`), with `node:test` cases for: a
      non-contiguous match ("gpt4o" finds "GPT-4o"), a single-substitution
      typo (e.g. "cluade" finds "claude"), a query matching only the
      provider name, no match (empty result), empty query (returns input
      unchanged, groups and order untouched), and a group with zero
      matches being dropped while a sibling group with a match is kept.
- [ ] T005 Add a `"check:chat-model-search": "node --test
scripts/check-chat-model-search.ts"` script to `package.json`
      alongside the existing `check:*` scripts, and run it to confirm T003
      passes T004's cases.

**Checkpoint**: `filterModelGroups` is correct and independently verified
before touching the UI.

---

## Phase 3: User Story 1 - Ein Modell im Chat per Suchbegriff finden (Priority: P1) 🎯 MVP

**Goal**: A search field in the chat composer's model picker filters the
list in real time, fuzzily, grouped by provider, with a no-results hint.

**Independent Test**: Open the model picker with ≥10 models across ≥2
providers, type part of a model's name: only matching models (and their
provider groups) stay visible; the model is selectable without scrolling
the full list (spec.md Independent Test, User Story 1).

### Implementation for User Story 1

- [ ] T006 [US1] In `src/components/chat/ComposerSettingsPopover.vue`, add
      a `query` ref (component-local, starts empty) and a `computed`
      `filteredModelGroups` calling `filterModelGroups(props.modelGroups,
query.value)` (T003).
- [ ] T007 [US1] Add a text `<input>` as the first child inside
      `ShadcnSelectContent` (before the `ShadcnSelectGroup` loop), bound to
      `query`, with `:placeholder="t('chat.composer.settingsPopover
.modelSearch.placeholder')"` (T002) and an accessible label; render
      the existing `ShadcnSelectGroup`/`ShadcnSelectItem` loop over
      `filteredModelGroups` instead of `props.modelGroups`.
- [ ] T008 [US1] When `filteredModelGroups` is empty, render
      `t('chat.composer.settingsPopover.modelSearch.noResults')` in place
      of the group list (spec FR-005, research.md R3) instead of an empty
      area.
- [ ] T009 [US1] Reset `query` to `''` whenever the popover closes and
      reopens (spec FR-006) — hook into the existing open/close state in
      `ComposerSettingsPopover.vue` (`isOpen`/`onMounted`/watch, whichever
      the component already uses for popover lifecycle).
- [ ] T010 [US1] Verify the already-selected model (`props.modelId`) stays
      the active selection even when it is filtered out of the visible
      list and the user closes without picking another (spec FR-008) — no
      code change expected since `ShadcnSelect`'s `model-value` is
      independent of which items are currently rendered, but confirm this
      manually per quickstart.md step 9 before moving on.

**Checkpoint**: User Story 1 is fully functional — search, filter, group,
empty state.

---

## Phase 4: User Story 2 - Weiterhin vollständig per Tastatur bedienbar (Priority: P2)

**Goal**: Typing filters; arrow keys move within visible matches; Enter
selects; Escape closes without changing the active model — no regression
versus the pre-search behavior.

**Independent Test**: Open the picker by keyboard, type a search term,
arrow to a match, Enter to select — the expected model becomes active
without touching the mouse (spec.md Independent Test, User Story 2).

### Implementation for User Story 2

- [ ] T011 [US2] On the search input from T007, handle `@keydown`: stop
      propagation for character-producing keys and Backspace/Delete (so
      Reka `Select`'s own type-ahead-jump does not also fire while typing
      in the search field, research.md R2), while letting ArrowDown,
      ArrowUp, Enter and Escape bubble to the existing `ShadcnSelect`
      listbox unchanged.
- [ ] T012 [US2] Confirm (manually, quickstart.md steps 7–9) that: arrow
      keys move only across currently-visible (filtered) items, Enter
      selects the highlighted item and closes the popover, and Escape
      closes without changing `modelId` — all matching the pre-search
      behavior of `ShadcnSelect`. No further code expected if T011 is
      correct; fix here if a regression turns up.

**Checkpoint**: Both user stories work together — search plus full
keyboard control, no regression.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [ ] T013 [P] Add `scripts/e2e/scenarios/chat-model-search.test.ts`
      (pattern: `scripts/e2e/scenarios/*.test.ts`), covering: opening the
      composer's model popover with the stand-in provider's models
      (`scripts/e2e/lib/provider.ts`), typing a non-contiguous filter and
      asserting only matching items remain visible, typing a query with no
      matches and asserting the no-results hint appears, clearing the
      query and asserting the full list returns, and keyboard-only
      select-and-close (ArrowDown, Enter) landing on the expected model.
- [ ] T014 Run `pnpm lint`, `pnpm typecheck`, `npx tsc --project
tsconfig.scripts.json --noEmit`, `pnpm format:check`, `pnpm
check:chat-model-search` (T005), and the new e2e scenario (T013) via
      `pnpm test:e2e -- --grep chat-model-search`; fix anything that fails.
- [ ] T015 Walk through quickstart.md manually once, end to end, and note
      the result in the PR description.

---

## Dependencies & Execution Order

- **Setup (T001–T002)**: no dependencies, can start immediately; T001 and
  T002 are independent of each other.
- **Foundational (T003–T005)**: T003 needs T001 (fuse.js installed); T004
  can be written in parallel with T003 (it only needs the function
  signature) but needs T003 to actually pass; T005 needs both.
- **User Story 1 (T006–T010)**: needs Foundational complete (T003) and
  T002 (i18n keys) for T007/T008; T006 before T007/T008/T009; T010 last.
- **User Story 2 (T011–T012)**: needs User Story 1's T007 (the input must
  exist first); otherwise independent of the rest of US1.
- **Polish (T013–T015)**: needs both user stories complete.

### Parallel Example: Foundational

```text
Task: "Create filterModelGroups in src/lib/chat/modelSearch.ts (T003)"
Task: "Create scripts/check-chat-model-search.ts test cases (T004)"
```

(T004 can be drafted in parallel with T003, but T005's run needs both
finished.)

## Implementation Strategy

**MVP first**: Setup → Foundational → User Story 1 (T001–T010) is a
complete, demoable improvement on its own (search works with the mouse).
User Story 2 (T011–T012) then confirms/completes keyboard parity before
Polish.
