# Tasks: Portable Mode with Protected Data

**Input**: Design documents from `/specs/014-portable-mode/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

## Phase 1: Setup and feasibility gate

- [ ] T001 Record the portable-mode implementation phase and feature-013 dependency in `plans/README.md`
- [ ] T002 [P] Create the portable path-owner inventory in `specs/014-portable-mode/research.md` and `specs/014-portable-mode/contracts/runtime-boundary.md`
- [ ] T003 [P] Build the protected-model feasibility spike against `src-tauri/src/models/paths.rs` and `src-tauri/src/chat/model_loading.rs` in `src-tauri/tests/portable_model_feasibility.rs`
- [ ] T004 Evaluate T003 for readable host-disk plaintext, wrong-key exposure, staging files, hashing, cancellation, and cleanup; record the pass/fail decision in `specs/014-portable-mode/research.md`
- [ ] T005 [P] Add deterministic portable path and mode fixtures in `src-tauri/src/portable/paths_tests.rs`

## Phase 2: Foundational portable runtime

- [ ] T006 Add the process-owned portable mode types and state boundary in `src-tauri/src/portable/mod.rs` and `src-tauri/src/portable/state.rs`
- [ ] T007 Add root validation, child-path construction, and no-fallback errors in `src-tauri/src/portable/paths.rs`
- [ ] T008 Route managed instance paths through the selected root in `src-tauri/src/instances/paths.rs` and cover normal-installation compatibility in `src-tauri/src/instances/paths_tests.rs`
- [ ] T009 Route model roots, imports, downloads, and staging paths through the selected root in `src-tauri/src/models/paths.rs` and `src-tauri/src/models/paths_tests.rs`
- [ ] T010 Route installation identity and startup-owned paths through the selected root in `src-tauri/src/device/commands.rs`, `src-tauri/src/instances/paths.rs`, `src-tauri/src/instances/startup.rs`, and `src-tauri/src/models/paths.rs`; document every additional preferences, webview, cache, diagnostics, or helper-process owner in `specs/014-portable-mode/contracts/runtime-boundary.md`
- [ ] T011 Integrate mode detection before instance discovery and directory creation in `src-tauri/src/instances/startup.rs`
- [ ] T012 Carry and restore only the non-secret relaunch descriptor through the feature-013 lifecycle, without creating a third app-owned file, in `src-tauri/src/state.rs` and the existing relaunch modules
- [ ] T013 Add failure-path tests for missing, read-only, full, removed, and inconsistent portable roots in `src-tauri/src/portable/runtime_tests.rs`

## Phase 3: User Story 1 — Removable storage isolation (Priority: P1)

**Independent test**: Run a complete removable-storage session and verify that every app-owned file is below the portable root and no host fallback exists.

- [ ] T014 [P] [US1] Add portable-mode startup and root-selection commands in `src-tauri/src/portable/commands.rs`
- [ ] T015 [P] [US1] Add the frontend mode state and structured error mapping in `src/composables/usePortableMode.ts`
- [ ] T016 [US1] Add the first-start mode selection and unavailable-root states in `src/components/portable/PortableModeSetup.vue` and `src/pages/index.vue`
- [ ] T017 [US1] Add filesystem isolation checks for instances, models, preferences, cache, diagnostics, and temporary files in `scripts/check-portable-filesystem.ts`
- [ ] T018 [US1] Add relaunch and storage-removal integration coverage in `src-tauri/tests/portable_runtime.rs`

## Phase 4: User Story 2 — Protection at rest (Priority: P2)

**Independent test**: With markers in every protected category, inspect storage without the passphrase and then unlock and use a protected local model.

- [ ] T019 [US2] If T004 passes, implement the audited protected-container adapter in `src-tauri/src/portable/container.rs` with the invariants from `specs/014-portable-mode/contracts/protected-container.md`
- [ ] T020 [US2] Add session-key derivation, zeroization, and close/failure release handling in `src-tauri/src/portable/state.rs`
- [ ] T021 [US2] Route model download, import, hashing, and load operations through the protected boundary in `src-tauri/src/models/commands.rs`, `src-tauri/src/models/import.rs`, and `src-tauri/src/chat/model_loading.rs`
- [ ] T022 [US2] Add wrong-passphrase, corruption, interrupted-write, disk-full, and plaintext-staging tests in `src-tauri/src/portable/container_tests.rs`
- [ ] T023 [US2] Add raw marker and filename inspection coverage in `src-tauri/tests/portable_protection.rs`
- [ ] T024 [US2] If T004 fails, implement the explicitly weaker removable-storage warning and keep single-file protection unavailable in `src/components/portable/ProtectionNotice.vue`

## Phase 5: User Story 3 — Single-file mode (Priority: P2)

**Independent test**: On Windows without administrator rights, run the non-installer file, create/unlock/delete one protected container, and observe no system-wide changes.

- [ ] T025 [US3] Add Windows single-file launch descriptor and non-installer packaging configuration in `src-tauri/tauri.conf.json` and `src-tauri/src/portable/mod.rs`
- [ ] T026 [US3] Add container creation, location selection, opaque default naming, and free-space validation in `src-tauri/src/portable/commands.rs`
- [ ] T027 [US3] Add unlock, delete, end-of-session deletion offer, and protected-container error states in `src/components/portable/PortableContainerSetup.vue`
- [ ] T028 [US3] Add no-elevation/no-system-registration packaging checks in `scripts/check-portable-distribution.ts`
- [ ] T029 [US3] Add Windows black-box coverage for relaunch, copy, wrong passphrase, deletion, and no-admin operation in `src-tauri/tests/portable_single_file.rs`

## Phase 6: User Story 4 — Limits statement (Priority: P3)

**Independent test**: Before unlock, verify that the UI and documentation enumerate every covered and uncovered category from FR-019 and FR-020.

- [ ] T030 [P] [US4] Add the pre-unlock limits statement component in `src/components/portable/PortableLimitsStatement.vue`
- [ ] T031 [P] [US4] Add German and English portable-mode security strings in `i18n/locales/de.json` and `i18n/locales/en.json`
- [ ] T032 [US4] Document the threat model, OS traces, deliberate host writes, and deletion limitations in `docs/security/portable-mode.md`
- [ ] T033 [US4] Add a UI/content check that the statement is available before unlock in `scripts/check-portable-limits.ts`

## Phase 7: User Story 5 — Honest unsupported environments (Priority: P3)

**Independent test**: Start with missing components or an unusable root and verify a plain-language refusal with zero host writes.

- [ ] T034 [US5] Add structured missing-component and protection-unavailable errors in `src-tauri/src/error.rs` and `src-tauri/src/portable/commands.rs`
- [ ] T035 [US5] Add localized refusal states in `src/components/portable/PortableUnavailable.vue`
- [ ] T036 [US5] Add missing-component and blocked-launch scenarios to `scripts/check-portable-filesystem.ts`

## Phase 8: User Story 6 — Normal installation compatibility (Priority: P3)

**Independent test**: Run the existing normal-installation suite and confirm unchanged paths, no additional encryption, and no portable UI.

- [ ] T037 [US6] Add normal-installation regression assertions to `src-tauri/src/portable/normal_installation_tests.rs`
- [ ] T038 [US6] Add a frontend check that portable setup is absent for normal launches in `scripts/check-portable-limits.ts`

## Phase 9: Polish and handoff

- [ ] T039 [P] Update `CONTEXT.md` with portable mode, protected container, session key, and limits terminology
- [ ] T040 [P] Update `specs/014-portable-mode/quickstart.md` with the final supported platform and feasibility result
- [ ] T041 Run `cargo test`, `cargo clippy --all-targets -- -D warnings`, all portable checks, `pnpm typecheck`, `pnpm lint`, and `pnpm format:check`; record commands and results in the PR
- [ ] T042 Re-run cross-artifact analysis across `spec.md`, `plan.md`, and `tasks.md` and resolve all CRITICAL/HIGH findings before opening the PR

## Dependencies and execution order

- T001–T005 are the gate. T019 and T025 are forbidden until T004 passes for the single-file protection path.
- T006–T013 establish the runtime boundary before any user story.
- US1 can proceed after Phase 2; US2 and US3 depend on the T004 result and US1's root plumbing.
- US4 and US5 can proceed in parallel with US2/US3 once the error and mode contracts exist.
- US6 is a regression gate for every phase, not an optional cleanup task.
- T039–T042 run only after the supported scope and tests are stable.

## MVP scope

The first demonstrable slice is Phase 2 plus US1: removable-storage root
isolation, relaunch retention, refusal without fallback, and filesystem evidence.
The single-file distribution is not MVP until T004 proves the protected-model
boundary.
