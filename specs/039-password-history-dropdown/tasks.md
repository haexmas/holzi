# Tasks: Kompakter Passwortverlauf

## Phase 1: Setup

- [x] T001 Confirm the feature artifacts and active worktree in `specs/039-password-history-dropdown/`

## Phase 2: User Story 1 — Frühere Eintragsstände übersichtlich auswählen

- [x] T002 [US1] Replace the two-column history wrapper with a single vertical flow in `src/components/passwords/HistoryTab.vue`
- [x] T003 [US1] Render the existing history state selector as an accessible dropdown while preserving state order and selection events in `src/components/passwords/HistoryTimeline.vue`

## Phase 3: User Story 2 — Zeitinformation zurückhaltend wahrnehmen

- [x] T004 [US2] Reduce the visual emphasis of the saved-at metadata without changing timestamp fallback behavior in `src/components/passwords/HistorySnapshot.vue`

## Phase 4: Validation

- [x] T005 Run the relevant template, type, and lint checks from `specs/039-password-history-dropdown/quickstart.md`
