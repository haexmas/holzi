# Tasks: Dark-Mode-Input-Kontrast

**Input**: Design documents from `/specs/039-dark-mode-input-contrast/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, quickstart.md

**Tests**: No new test task is required for this one-token visual change; the
existing formatter, build, and appearance checks provide the validation seam.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm the existing frontend/theme structure.

- [x] T001 Verify the shared stylesheet and existing validation commands in `src/assets/css/tailwind.css` and `package.json`

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: No additional infrastructure is required for this CSS-only change.

- [x] T002 Confirm the existing Dark Mode `--input` and `--border` tokens are the shared seam in `src/assets/css/tailwind.css`

## Phase 3: User Story 1 - Nicht fokussierte Eingabefelder erkennen (Priority: P1) 🎯 MVP

**Goal**: Make non-focused input boundaries distinguishable in Dark Mode while preserving Light Mode and focus styling.

**Independent Test**: Inspect empty and filled inputs in Dark Mode with focus removed, then compare focused inputs and Light Mode inputs against the acceptance scenarios in `spec.md`.

### Implementation for User Story 1

- [x] T003 [US1] Increase the Dark Mode `--input` token contrast in `src/assets/css/tailwind.css` and keep the matching default in `src/lib/appearance/tokens.ts` without changing the Light Mode token, focus rules, or surrounding `--border` token

**Checkpoint**: User Story 1 is complete when the changed stylesheet passes formatting and the quickstart validation confirms the Dark Mode boundary is visible.

## Phase 4: Polish & Cross-Cutting Concerns

**Purpose**: Validate the completed visual change.

- [x] T004 Run the formatter check, relevant appearance check, and production build from `quickstart.md`
- [x] T005 Review the final diff for scope, unchanged Light Mode values, and absence of debug instrumentation

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001 is complete and has no dependencies.
- **Foundational (Phase 2)**: T002 follows T001 and confirms the implementation seam.
- **User Story 1 (Phase 3)**: T003 follows T002.
- **Polish (Phase 4)**: T004 and T005 follow T003; T004 must pass before handoff.

### User Story Dependencies

- **User Story 1 (P1)**: Can start after the foundational token check; no other story dependencies.

### Parallel Opportunities

- No implementation tasks are parallelized because the validation and styling tasks intentionally touch the same stylesheet and final diff.

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Verify the token seam.
2. Apply the single shared Dark Mode token adjustment.
3. Run the independent visual and automated checks.

### Incremental Delivery

This feature has one user story and is delivered as one small stylesheet change.

## Notes

- The task list uses the required checkbox/ID/story/path format.
- No new data model, contract, component, dependency, or test framework is introduced.
