# Tasks: Kompakter Passwortverlauf

## Phase 1: Setup

- [x] T001 Confirm the feature artifacts and active worktree in `specs/039-password-history-dropdown/`

## Phase 2: User Story 1 — Frühere Eintragsstände übersichtlich auswählen

- [x] T002 [US1] Replace the two-column history wrapper with a single vertical flow in `src/components/passwords/HistoryTab.vue`
- [x] T003 [US1] Render the existing history state selector as an accessible dropdown while preserving state order and selection events in `src/components/passwords/HistoryTimeline.vue`

## Phase 3: User Story 2 — Zeitinformation zurückhaltend wahrnehmen

- [x] T004 [US2] Reduce the visual emphasis of the saved-at metadata without changing timestamp fallback behavior in `src/components/passwords/HistorySnapshot.vue`

## Phase 4: User Story 3 — Eintragsaktionen verlässlich ausführen

- [x] T006 [US3] Render the trash confirmation for the entry header's delete button in `src/components/passwords/EntryView.vue`, checked by `scripts/check-passwords-entry-delete.ts`
- [x] T007 [US3] Keep a mouse-held reveal through a layout reflow and end it on release, cancel and lost pointer capture in `src/components/passwords/MaskedValue.vue`, checked by `scripts/check-passwords-reveal.ts`
- [x] T008 [US3] Add the app-scoped `active_instance_name` command (`src-tauri/src/instances/list.rs`, allow-list in `src-tauri/src/vault_gate/invoke.rs`) and return to the active workspace from `src/pages/index.vue`, checked by `scripts/check-vault-lifecycle.ts`

## Phase 5: Validation

- [x] T005 Run the relevant template, type, and lint checks from `specs/039-password-history-dropdown/quickstart.md`
- [x] T009 Drive the history dropdown (open, arrow key, End, Enter) in `scripts/e2e/scenarios/passwords-tabs.test.ts`
