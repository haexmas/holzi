---
description: 'Task list for making the active window-manager tab easier to identify'
---

# Tasks: Active Tab Emphasis

**Input**: Design documents from `specs/040-active-tab-emphasis/`

**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `quickstart.md`

**Tests**: No new automated test task is needed for this presentation-only class change; existing template, navigation, lint, and type checks validate the affected surface.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm the existing component and theme tokens are the implementation surface.

- [x] T001 Confirm the existing active-tab binding and theme utility classes in `src/components/wm/TabBar.vue` against `specs/040-active-tab-emphasis/research.md`

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: No additional foundation is required because active-tab state, ARIA semantics, and theme tokens already exist.

- [x] T002 Confirm no new state, dependency, data model, or contract is needed for `src/components/wm/TabBar.vue`

## Phase 3: User Story 1 - Aktiven Tab sofort erkennen (Priority: P1) 🎯 MVP

**Goal**: Make the selected tab immediately distinguishable in the visible multi-tab strip while preserving all existing tab behavior.

**Independent Test**: In a window with at least two tabs, verify that exactly the selected tab has the stronger surface, text weight, and lower accent in both light and dark themes; switch tabs and verify the treatment follows the selection.

### Implementation for User Story 1

- [x] T003 [US1] Strengthen the active multi-tab class branch in `src/components/wm/TabBar.vue` with a contrasting theme surface, a semibold title, and a visible bottom accent while leaving the inactive and compact branches unchanged

**Checkpoint**: User Story 1 is complete when the active tab is visually obvious, selection moves the treatment correctly, and existing tab controls remain usable.

## Phase 4: Polish & Cross-Cutting Concerns

**Purpose**: Validate the implementation against the existing frontend checks and the manual visual scenarios.

- [x] T004 [P] Run `pnpm check:templates` and `pnpm check:wm-navigation` for template validity and unchanged tab behavior
- [x] T005 [P] Run `pnpm lint` and `pnpm typecheck` for frontend correctness
- [ ] T006 [P] Perform the light/dark visual validation from `specs/040-active-tab-emphasis/quickstart.md`
- [x] T007 [US1] Give the multi-tab `role="tab"` element in `src/components/wm/TabBar.vue` an inset focus-visible ring, since the scrolling tablist clips the default outline (FR-007)

## Dependencies & Execution Order

### Phase Dependencies

- Phase 1 and Phase 2 are complete because the existing implementation was inspected during planning.
- Phase 3 depends on the confirmed existing active-tab binding.
- Phase 4 depends on T003 and T007.

### User Story Dependencies

- User Story 1 has no dependency on another user story and is the complete MVP.

### Parallel Opportunities

- T004, T005, and T006 can run independently after T003 and T007; T006 is the manual visual check.

## Implementation Strategy

1. Apply the smallest presentation-only change in `TabBar.vue`.
2. Run the existing automated checks.
3. Verify both themes and the preserved single-tab/compact layouts manually.
