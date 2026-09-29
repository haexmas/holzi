---
description: 'Task list for 030-app-multi-instance'
---

# Tasks: Mehrfachinstanzen für Apps

**Input**: Design documents from `/specs/030-app-multi-instance/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [quickstart.md](./quickstart.md)

**Tests**: Included — the underlying mechanism (`openApp`/`addTab`/`hydrate`)
is already covered generically (spec 015 T051/T052); this feature adds
regression coverage with the real `system.chat` app plus one e2e scenario
for the visible user path.

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

None — no new dependency, no new file structure beyond tests
(research.md R1).

## Phase 2: Foundational (blocking)

- [ ] T001 In `src/lib/wm/apps.ts`, change the `system.chat` entry's
      `multiInstance` from `false` to `true`. Leave every other field of
      that entry, and the entire `system.settings` entry, unchanged.

**Checkpoint**: The registry change both user stories depend on is in
place.

---

## Phase 3: User Story 1 - Mehrere Chat-Unterhaltungen gleichzeitig offen (Priority: P1) 🎯 MVP

**Goal**: Opening Chat again (Launcher or "+") always creates a new,
independent instance instead of activating an existing one.

**Independent Test**: Open Chat, send a message. Open Chat again via "+" in
the same window: a second tab with a new, empty conversation appears; the
first tab and its conversation are untouched (spec.md Independent Test).

### Tests for User Story 1

- [ ] T002 [P] [US1] In `scripts/check-wm-state.ts`, add cases using the
      real `WM_APPS` registry (not the synthetic `MULTI_APP`) mirroring the
      existing T051/T052 cases: `openApp` with `system.chat` twice opens
      two windows; `addTab` with `system.chat` appends a second tab in the
      same window instead of activating a singleton elsewhere; `hydrate`
      restores two windows that each hold a `system.chat` tab without
      collapsing them. Also add one case confirming `system.settings`
      still activates its existing tab instead of opening a second one
      (no regression).
- [ ] T003 [P] [US1] Add
      `scripts/e2e/scenarios/chat-multi-instance.test.ts` (pattern:
      `scripts/e2e/scenarios/tab-content-isolation.test.ts`): open Chat,
      send a message via the stand-in provider; open Chat again through
      the window's "+" menu and confirm a second tab exists with an empty,
      independent conversation and the first tab's conversation is
      unchanged; open Chat via the Launcher and confirm a _new window_
      appears (not a focused existing tab); confirm the "+" menu never
      shows "bereits geöffnet"/"already open" for Chat.

### Implementation for User Story 1

- [ ] T004 [US1] Run `pnpm check:wm-state` and confirm T002's new cases
      pass without any change beyond T001 (per research.md R1, the
      mechanism needs no code change — this step is verification, not
      implementation).

**Checkpoint**: User Story 1 fully functional and independently verified.

---

## Phase 4: User Story 2 - Einstellungen bleiben Einzelinstanz (Priority: P1)

**Goal**: No regression — Settings keeps activating its existing tab.

**Independent Test**: Open Settings, open it again via the Launcher: still
one tab, activated and focused (spec.md Independent Test).

### Tests for User Story 2

- [ ] T005 [US2] Extend `scripts/e2e/scenarios/chat-multi-instance.test.ts`
      (T003) with a Settings check: open Settings, open it again via the
      Launcher and via "+", and confirm both times the existing tab is
      activated/focused rather than a second one appearing.

**Checkpoint**: Both user stories verified together — Chat multi-instance,
Settings unchanged.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [ ] T006 Run `pnpm lint`, `pnpm typecheck`, `npx tsc --project
tsconfig.scripts.json --noEmit`, `pnpm format:check`, `pnpm
check:wm-state` (T002/T004), and the new e2e scenario (T003/T005)
      via `pnpm test:e2e -- --grep chat-multi-instance`; fix anything that
      fails.
- [ ] T007 Add the amendment blockquote to `specs/015-workspace-shell/spec.md`
      FR-017 (same style as Spec 022's note on FR-023) recording that Chat
      is a multi-instance app as of this spec; leave the original FR-017
      wording in place as the historical record (research.md R2).
- [ ] T008 [P] Add the corresponding amendment to
      `specs/020-tab-navigation/spec.md`'s Assumptions entry ("Mittelklick
      oder Strg+Klick ... weil alle ausgelieferten Apps Einzelinstanz-Apps
      sind"), noting that this is no longer true for Chat as of this spec.
- [ ] T009 Walk through quickstart.md manually once, end to end, and note
      the result in the PR description.

---

## Dependencies & Execution Order

- **Foundational (T001)**: no dependencies, must land before every other
  task.
- **User Story 1 (T002–T004)**: needs T001. T002 and T003 can be written in
  parallel (different files); T004 needs both to actually run them.
- **User Story 2 (T005)**: needs T003 (extends the same scenario file);
  otherwise independent of T002/T004.
- **Polish (T006–T009)**: T006 needs everything above; T007/T008 are
  documentation-only and independent of the code tasks and of each other.

## Implementation Strategy

**MVP first**: T001 alone already delivers User Story 1 in the running
app; T002–T004 exist purely to make that regression-proof. User Story 2
(T005) is a same-file addition confirming no regression. Polish
(T006–T009) closes out verification and the spec amendments.
