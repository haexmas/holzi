---
description: "Task list for the frontend-onboarding feature (spec 001)"
---

# Tasks: Frontend Onboarding — Landing, Create, Open, Connect

**Input**: Design documents from `specs/001-frontend-onboarding/`
**Prerequisites**: [`spec.md`](./spec.md), [`plan.md`](./plan.md), [`contracts/tauri-commands.md`](./contracts/tauri-commands.md), [`contracts/events.md`](./contracts/events.md), [`contracts/types.md`](./contracts/types.md)

**Blockers external to this spec**:

- `haex-crdt` extraction from `haex-vault` (per [`docs/plans/2026-09-04-haex-crdt-extraction-plan.md`](../../docs/plans/2026-09-04-haex-crdt-extraction-plan.md) and [`docs/plans/2026-09-04-v1-scope-design.md §10`](../../docs/plans/2026-09-04-v1-scope-design.md)). No implementation task below that requires `haex-crdt` at runtime may start until the crate is importable.
- Frontend-only scaffold tasks (T001..T010) do not depend on `haex-crdt` and may begin immediately.

**Tests**: E2E tests are IN scope (SC-006 and SC-007 depend on them). Vitest for stores, composables, and the `html5-qrcode` scanner lifecycle in `ConnectSheet` is IN scope. Rust `cargo test` for command handlers is IN scope. Other pure UI component tests are OUT of scope for this slice because their behavior is covered by E2E.

**Organization**: Tasks are grouped by user story (US1..US5 from `spec.md`). Setup + Foundational phases are shared; user stories are as independent as they can practically be, given they share the Landing page.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies).
- **[Story]**: `US1..US5` per spec; `-` for shared work.

## Path Conventions

Paths follow [`plan.md → Project Structure`](./plan.md).

---

## Phase 1: Setup (Shared Infrastructure)

- [ ] **T001** [-] Initialize Nuxt 4 project in repo root: `pnpm create nuxt@latest . --package-manager pnpm --git-init false`; set `ssr: false` in `nuxt.config.ts`.
- [ ] **T002** [-] Add Tauri to the project: `cargo tauri init` under `src-tauri/`; wire up `pnpm tauri` script in `package.json`.
- [ ] **T003** [P] [-] Install Tailwind v4 via `@tailwindcss/vite` (NOT `@nuxtjs/tailwindcss` — that module tracks v3). Create `src/assets/css/tailwind.css` with `@import 'tailwindcss'` and shadcn-vue CSS variables. Register the Vite plugin in `nuxt.config.ts` and add `css: ['~/assets/css/tailwind.css']`.
- [ ] **T004** [P] [-] Install shadcn-vue: `pnpm dlx shadcn-vue@latest init` — writes `components.json`, creates `src/lib/utils.ts` with `cn()` helper. Verify `components.json` points aliases at `~/components`, `~/lib/utils`.
- [ ] **T005** [P] [-] Add shadcn-vue components: `pnpm dlx shadcn-vue@latest add button card sheet dialog input label radio-group sonner`. Verify auto-import pathPrefix names: `<UiButton>`, `<UiSheet>`, etc.
- [ ] **T006** [P] [-] Install and configure `@nuxt/icon` with local Lucide bundle: install `@iconify-json/lucide` as dependency; configure `nuxt.config.ts` with `icon: { mode: 'svg', serverBundle: false, clientBundle: { scan: true, includeCustomCollections: true } }`. Add a Playwright network-assertion smoke test that fails if the built app requests `api.iconify.design` (satisfies SC-006).
- [ ] **T007** [P] [-] Install and configure `@nuxtjs/i18n` with `de` (default) and `en` locales; create `src/i18n/locales/{de,en}.json` with an initial `landing`, `errors`, `onboarding` namespace tree; enable per-component `<i18n lang="yaml">` blocks (haex-vault pattern).
- [ ] **T008** [P] [-] Install `@pinia/nuxt` and `@vueuse/nuxt`; add both to `modules`. Create empty `src/stores/instances.ts` (Pinia setup store) as a placeholder — filled in T014.
- [ ] **T009** [-] Configure Nuxt component auto-import with `pathPrefix: true`: `components: [{ path: '~/components', pathPrefix: true }]` in `nuxt.config.ts`. Verify by adding a placeholder `src/components/ui/button/Button.vue` and using `<UiButton />` in `app.vue` without an import.
- [ ] **T010** [P] [-] Set up ts-rs binding pipeline: add `ts-rs` to `src-tauri/Cargo.toml`; add `pnpm generate:ts-types` script matching haex-vault's; add `@bindings/*` path alias to `tsconfig.json` resolving to `src/types/bindings/*`; ensure the generated dir is git-tracked and CI checks it against a fresh regeneration.

**Checkpoint**: Phase 1 complete → Nuxt app boots to a blank shell with Tailwind, shadcn-vue, i18n, Pinia, `@nuxt/icon`, and pathPrefix auto-import all working; Tauri window opens; ts-rs pipeline in place. No `haex-crdt` needed yet.

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: No user story work can begin until this phase is complete. All tasks in this phase depend on `haex-crdt` being importable.

- [ ] **T011** [-] Add `haex-crdt` as a Rust dependency in `src-tauri/Cargo.toml` (workspace path or crates.io per its release status). Verify a hello-world `HaexCrdt::init` compiles.
- [ ] **T012** [-] Implement `src-tauri/src/instances/paths.rs` mirroring [`haex-vault paths.rs`](../../../../haex-vault/src-tauri/src/database/paths.rs): `get_instance_path(name)`, `get_instances_directory()` — resolves against `BaseDirectory::AppLocalData`, creates directory if missing.
- [ ] **T013** [-] Implement `src-tauri/src/error.rs` with the `HolziError` enum per [`contracts/tauri-commands.md`](./contracts/tauri-commands.md). Add `#[derive(TS)]` and export.
- [ ] **T014** [-] Implement `src-tauri/src/state.rs`: `AppState { active_instance: Mutex<Option<ActiveInstanceHandle>> }`. `ActiveInstanceHandle` holds the `haex-crdt` handle and shutdown senders for the Nostr relay + iroh peer. Register in `main.rs::manage`.
- [ ] **T015** [-] Implement `src-tauri/src/instances/events.rs::emit_instance_list_changed(app, reason, affected)` and wire the AppHandle plumbing.
- [ ] **T016** [-] Implement `src-tauri/src/instances/list.rs::list_instances` per contract. Treat the database mtime as the persisted `lastAccess` value, sort by it desc, and filter out `.trash/`, hidden files, and pending Genesis files. `#[tauri::command]` registered in `main.rs`. Add coverage that a successful `open_instance` mtime refresh changes list ordering.
- [ ] **T017** [-] Implement `src-tauri/src/instances/crud.rs::close_instance` per contract, including `AppHandle` event emission. Idempotent. `#[tauri::command]` registered. `open_instance` reuses the same shutdown path while holding the state lock for an atomic switch.
- [ ] **T018** [-] Wire up frontend `src/composables/useInstance.ts` skeleton: exposes `openAsync`, `createAsync`, `confirmCreateAsync`, `abortCreateAsync`, `closeAsync`, `importAsync`, `trashAsync`. Each wraps a single `invoke(...)` and re-throws typed errors.
- [ ] **T019** [-] Fill `src/stores/instances.ts` (Pinia): `instances` ref, `activeInstance` ref, `syncAsync()` calling `list_instances`, event listener for `instance-list-changed` calling `syncAsync()`. Test with Vitest that a fake emitted event triggers a re-sync.
- [ ] **T020** [-] Implement `src/app.vue`: root with `<UiSonner />` (toast provider) and `<NuxtPage />`. Add `useHead({ title: 'Holzi' })`. Register the i18n locale switcher plumbing (UI element deferred to a later spec).

**Checkpoint**: Phase 2 complete → backend can list, close instances; frontend store loads and reacts to backend events; app.vue renders. Ready for user-story work.

---

## Phase 3: User Story 4 — Zuletzt verwendet + Unlock (Priority: P1) 🎯 MVP

**Rationale for taking US4 before US1**: US4 depends only on Foundational + a placeholder `create_instance` returning a fixture. Starting US4 first exposes the Pinia+Sheet+i18n integration on the simplest possible surface, so US1's more complex flows land on a stable base. Alternative order (US1 first) works too if a team prefers.

**Goal**: Landing page renders the instances list; clicking an item opens the Unlock sheet; correct passphrase navigates to `/federation/<name>`.

**Independent Test**: Seed `<AppLocalData>/instances/` with one throwaway `.db` (created directly via `haex-crdt` test helper), launch the app, unlock. Verify list appears sorted, unlock succeeds, wrong passphrase shows generic error.

### Tests for US4

- [ ] **T021** [P] [US4] `tests/stores/instances.spec.ts`: Vitest — asserts `syncAsync` populates from a mocked `invoke('list_instances')` in `lastAccess` desc order and renders the non-secret `alias` field.
- [ ] **T022** [P] [US4] `e2e/onboarding.spec.ts::"landing shows seeded instances"`: Playwright — with a fixture instance file present, page renders the list.
- [ ] **T023** [P] [US4] `e2e/onboarding.spec.ts::"unlock happy path"`: enter correct passphrase → navigates to `/federation/<name>`.
- [ ] **T024** [P] [US4] `e2e/onboarding.spec.ts::"unlock wrong passphrase shows generic error, does not leak file existence"`: enter wrong passphrase for existing and non-existing name — error text is identical.

### Implementation for US4

- [ ] **T025** [US4] Implement `src-tauri/src/instances/crud.rs::open_instance` per contract. Under the state lock, validate the requested instance and SQLCipher credentials before closing any current active runtime; on validation failure, preserve the existing runtime and `AppState.active_instance` unchanged. After validation, close the current runtime, start the requested SQLCipher/relay/iroh runtime, refresh its mtime as `lastAccess`, and publish only the fully-active result. On any later startup failure, stop started services, drop the handle, clear state, and retain an import-pending copy and marker for retry after failed unlock attempts. Permit deletion only after an explicit discard action or conclusive validation that the file is not a holzi instance. Enforce `NotFound` and `WrongPassphrase` return the same discriminator-different-but-message-same guarantee. Add tests for active-instance preservation on validation failure and retry after a forced partial failure.
- [ ] **T026** [US4] Implement `src/pages/index.vue` landing shell with `<UiLogo />`, welcome text, version footer via `useAppVersion` composable, and slots for CTAs + list.
- [ ] **T027** [P] [US4] Implement `src/components/onboarding/InstancesList.vue`: iterates `useInstancesStore().instances`, renders each with alias + `formatRelativeTime(lastAccess)`, click emits `select(name)`. Empty state hides (FR-004).
- [ ] **T028** [P] [US4] Implement `src/components/onboarding/UnlockSheet.vue`: `<UiSheet>` with masked passphrase field, submit button, inline error area. On submit calls `useInstance.openAsync(name, passphrase)`.
- [ ] **T029** [US4] Wire `InstancesList → UnlockSheet` in `pages/index.vue`. On successful unlock, `navigateTo('/federation/' + name)`; do not call `close_instance` first because `open_instance` owns the atomic switch. Placeholder `pages/federation/[instance].vue` shows "Federation view — not this spec" so the navigation target exists.
- [ ] **T030** [P] [US4] i18n keys: `landing.welcome`, `landing.lastUsed`, `onboarding.unlock.title`, `onboarding.unlock.passphrase`, `onboarding.unlock.submit`, `errors.openFailed` (single generic message for `NotFound` + `WrongPassphrase` — see FR-021).

**Checkpoint**: US4 fully functional. Operator can Unlock an existing instance seeded manually.

---

## Phase 4: User Story 1 — Anlegen (Genesis) (Priority: P1) 🎯 MVP

**Goal**: Anlegen CTA opens the Create sheet; Genesis mode creates a fresh instance and displays the paper-seed. Recover sub-mode is deferred to US5.

**Independent Test**: On an empty install, click Anlegen, submit valid inputs, confirm paper-seed, land in the federation view. New file exists in `instances/`.

### Tests for US1

- [ ] **T031** [P] [US1] `e2e/onboarding.spec.ts::"anlegen genesis happy path"`: click Anlegen → fill name+passphrase+passphrase → paper-seed displayed → confirm (which invokes `confirm_create`) → arrives at federation view; verify the DB and no `.pending` marker remain.
- [ ] **T032** [P] [US1] `e2e/onboarding.spec.ts::"anlegen name collision blocked"`: seed an instance, try to Anlegen with same name → inline error, no file created.
- [ ] **T033** [P] [US1] `e2e/onboarding.spec.ts::"anlegen cancelled after file created discards orphan"`: intercept close-before-confirm, verify `.pending` marker + file are gone on next `list_instances`.
- [ ] **T034** [P] [US1] Rust test: `create_instance` with `CreateMode::Genesis` writes `.pending` before `.db` and activates the runtime; `confirm_create` removes the marker and emits `instance-list-changed { reason: 'confirmed' }`; `abort_create` deletes both marker and DB on cancellation or error.

### Implementation for US1

- [ ] **T035** [US1] Implement `src-tauri/src/instances/crud.rs::create_instance` per contract — Genesis branch only in this task (Join/Recover in later phases). Enforces `NameConflict`, `InvalidName`, `WeakPassphrase`, `InstanceAlreadyActive`. Writes `.pending` marker; hands passphrase + Genesis mode to `haex-crdt::init`; creates and stores the `ActiveInstanceHandle`, starts the Genesis Nostr relay and iroh peer, and completes runtime activation before returning the paper-seed.
- [ ] **T036** [US1] Implement startup orphan-cleanup in `src-tauri/src/main.rs::setup`: on boot, scan `instances/` for Genesis `.pending` markers and delete both the marker and the sibling `.db`; do not remove import-pending files, which await unlock validation. Emit `instance-list-changed { reason: 'startup-cleanup' }`.
- [ ] **T037** [P] [US1] Implement `src/components/onboarding/CreateSheet.vue`: `<UiSheet>` with `<UiRadioGroup>` (Genesis / Recover — Recover disabled with tooltip "Post-MVP" until US5), name field, two passphrase fields with match check, submit button. On submit calls `useInstance.createAsync(...)`.
- [ ] **T038** [P] [US1] Implement `src/components/onboarding/PaperSeedDisplay.vue`: renders the seed in a monospaced block with a "Ich habe die Wiederherstellungs-Seed notiert" confirm checkbox and Continue button. Copy-to-clipboard is intentionally NOT in v1 (operator must physically record it — same as haex-vault Genesis-parallel flows).
- [ ] **T039** [US1] Wire CreateSheet → PaperSeedDisplay in a two-step flow. On PaperSeedDisplay Continue, call `confirm_create` (never `close_instance`); on cancellation call `abort_create`. After confirmation the newly-created instance stays active and the app navigates to `/federation/<name>`.
- [ ] **T040** [P] [US1] Add Anlegen CTA to `pages/index.vue` as the first primary button; wire to open CreateSheet.
- [ ] **T041** [P] [US1] i18n: `onboarding.create.title`, `onboarding.create.name`, `onboarding.create.passphrase`, `onboarding.create.passphraseConfirm`, `onboarding.create.submit`, `onboarding.create.paperSeed.title`, `onboarding.create.paperSeed.recorded`, `errors.nameConflict`, `errors.invalidName`, `errors.weakPassphrase`.

**Checkpoint**: US1 + US4 shipped → holzi has a working walking-skeleton for a single device.

---

## Phase 5: User Story 2 — Verbinden (Join federation via pairing) (Priority: P1) 🎯 MVP

**Goal**: Verbinden CTA opens the Connect sheet; scanning/pasting a valid pairing token from a parent device creates a new instance, co-signs the pairing transcript, syncs `haex-crdt`, navigates to federation view.

**Independent Test**: With a second real holzi instance acting as parent (or a mocked pairing server for CI), Verbinden a fresh install and observe the attestation ring contains both devices on both sides.

### Tests for US2

- [ ] **T042** [US2] `e2e/two-device-pairing.spec.ts`: launch two isolated Tauri app instances, have the parent issue a token, paste it into the joiner, verify relay delivery and `haex-crdt` synchronization of both attestations, then send the walking-skeleton ping and assert the pong. This is the MVP acceptance test.
- [ ] **T043** [P] [US2] Rust test: expired token → `PairingTokenInvalid { reason: "expired" }`, no instance file created.
- [ ] **T044** [P] [US2] Rust test: already-consumed token → `PairingTokenInvalid { reason: "already-consumed" }`.
- [ ] **T045** [P] [US2] `e2e/onboarding.spec.ts::"verbinden happy path (mocked backend)"`: two desktop variants — (a) paste the token into the text fallback, submit, and assert navigation; (b) launch Chromium with a deterministic fake-camera video fixture containing a valid QR code so `getUserMedia()` returns a real `MediaStream`, let `html5-qrcode.start()` decode it, submit, and assert navigation. Add close/reopen and decode/close cases that assert every fake-camera `MediaStreamTrack` reaches `ended` before another scanner starts.
- [ ] **T045a** [P] [US2] `tests/components/onboarding/ConnectSheet.spec.ts`: mock `html5-qrcode` at its module boundary. Assert QR decode callbacks populate the same token field used by text input and submit it to `CreateMode::Join`; assert the idempotent cleanup path awaits `stop()` before `clear()` on successful decode, Sheet close, component unmount, and startup failure, and tolerates a scanner that never reached the running state. Cover granted, denied, prompt, and unavailable-camera states while keeping the text fallback usable.
- [ ] **T045b** [US2] Run Android-emulator and iOS-simulator smoke tests for UI, text fallback, and unavailable-camera behavior; simulators without camera input MUST NOT assert QR decoding. Run one real-device acceptance pass per platform to verify WebView camera-permission grant/denial, QR decode into the token field, cleanup on close, close/reopen, successful submit/navigation, and text fallback. Record that the Android merged manifest contains `android.permission.CAMERA` and the iOS bundle contains `NSCameraUsageDescription`.
- [ ] **T045c** [US2] Run packaged desktop acceptance tests for Linux, macOS, and Windows (one supported package per OS) with a fake camera/device harness: verify camera-permission grant and denial fallback, QR decode, `stop()`/`clear()` teardown, close/reopen, and the shared text-token fallback. Keep the package matrix separate from Chromium-only T045 so WebView packaging regressions are caught.

### Implementation for US2

- [ ] **T046** [US2] Implement `src-tauri/src/pairing/join.rs`: token parsing, transcript construction, contact-hint dial into parent's relay, co-signature exchange. Depends on the presence-event / attestation surface being present in `haex-crdt` (may require a shim task tracked separately if `haex-crdt` isn't fully there).
- [ ] **T047** [US2] Extend `create_instance` to route `CreateMode::Join` to `pairing::join::run`. Same orphan-file guarantee as Genesis.
- [ ] **T048** [P] [US2] Implement `src/components/onboarding/ConnectSheet.vue`: `<UiSheet>` with a live camera preview + QR decoder using `html5-qrcode` on every platform (works via `getUserMedia` in both desktop Tauri WebView and mobile WebView). Fallback text-input for the same token below the scanner. On successful decode, stop the scanner, populate the token field, and enable submit. Keep the text field available during permission denial or camera failure. A single idempotent cleanup path MUST await `stop()` after every `start()` attempt, tolerate the scanner not reaching its running state, and only then call `clear()`; invoke it on successful decode, Sheet close, component unmount, and startup failure.
- [ ] **T049** [P] [US2] Add Verbinden CTA to `pages/index.vue` (third primary button); wire to open ConnectSheet.
- [ ] **T050** [P] [US2] i18n: `onboarding.connect.title`, `onboarding.connect.scan`, `onboarding.connect.cameraPermission`, `onboarding.connect.cameraUnavailable`, `onboarding.connect.token`, `onboarding.connect.submit`, `errors.pairingTokenInvalid`.
- [ ] **T050a** [P] [US2] Add `html5-qrcode` (^2.3.8, matching haex-vault) to `package.json`. Tauri camera-permission configuration: on Android add `<uses-permission android:name="android.permission.CAMERA" />` to `src-tauri/gen/android/app/src/main/AndroidManifest.xml` and wire the WebView's `PermissionRequest` handler to auto-grant `RESOURCE_VIDEO_CAPTURE` after the OS runtime prompt is accepted; on iOS add `NSCameraUsageDescription` to `Info.plist`. Desktop (Linux/macOS/Windows) uses standard WebRTC — no extra config.

**Checkpoint**: US1 + US2 + US4 shipped → holzi v1 walking-skeleton fully reachable via UI. Two devices can be paired end-to-end from a fresh install.

---

## Phase 6: User Story 3 — Öffnen (Import external `.db` file) (Priority: P2)

**Goal**: Öffnen CTA opens the OS file picker; selected `.db` is copied into `<AppLocalData>/instances/`; appears in the list.

**Independent Test**: Point the file picker at a `.db` outside the managed directory; verify copy, source untouched, list refreshes.

### Tests for US3

- [ ] **T051** [P] [US3] Rust test: `import_instance_file` with a valid throwaway encrypted instance copies atomically to a pending target, leaves source untouched, emits `instance-list-changed { reason: 'imported' }`, and clears the marker after a successful unlock with the supplied passphrase.
- [ ] **T052** [P] [US3] Rust test: invalid source (not a regular `.db`, structurally unreadable, or wrong extension) rejected before copy; encrypted-page validation is covered at unlock time.
- [ ] **T053** [P] [US3] Rust test: name collision with `ConflictPolicy::Rename` produces `<name>-2.db`; with `Abort` returns `NameConflict`.
- [ ] **T054** [P] [US3] `e2e/onboarding.spec.ts::"öffnen happy path"`: simulate a `plugin-dialog` selection → sheet completes → assert the backend's `instance-list-changed { reason: 'imported' }` event refreshes the new item in the list.

### Implementation for US3

- [ ] **T055** [US3] Implement `src-tauri/src/instances/import.rs::import_instance_file` per contract, including regular-file/extension validation, atomic copy (temp + rename), import-pending marker, conflict handling, and active-target rejection under the state lock. Defer SQLCipher credential validation to `open_instance`.
- [ ] **T056** [P] [US3] Implement `src/components/onboarding/OpenSheet.vue`: `<UiSheet>` with "Datei wählen" button that calls `@tauri-apps/plugin-dialog::open({ filters: [{ name: 'Instance', extensions: ['db'] }] })`. Pass the picker-returned external `source_path` only to `import_instance_file`; show the basename only for privacy and provide an "Importieren" button. Managed instance paths never cross the frontend boundary.
- [ ] **T057** [P] [US3] Add Öffnen CTA to `pages/index.vue` (second primary button, between Anlegen and Verbinden).
- [ ] **T058** [P] [US3] Conflict resolution UI: if `import_instance_file` returns `NameConflict`, show a `<UiDialog>` with three options: Rename (default), Overwrite (requires confirming a checkbox), Cancel. Never silently overwrite; an overwrite targeting the active instance is rejected under the backend state lock until that instance is explicitly closed.
- [ ] **T059** [P] [US3] i18n: `onboarding.open.title`, `onboarding.open.chooseFile`, `onboarding.open.import`, `onboarding.open.conflict.title`, `onboarding.open.conflict.rename`, `onboarding.open.conflict.overwrite`, `onboarding.open.conflict.cancel`, `errors.notAValidInstance`.

**Checkpoint**: US1 + US2 + US3 + US4 shipped → landing has all three CTAs plus Unlock.

---

## Phase 7: User Story 5 — Aus Paper-Seed wiederherstellen (Recover) (Priority: P3)

**Goal**: The Recover sub-mode of Anlegen is unlocked and functional. Operator enters a paper-seed; fingerprint match is shown; on confirm, an instance is created whose federation-root derives from the seed.

**Independent Test**: With a paper-seed from a prior Genesis run recorded out-of-band, complete Recover on a fresh install; verify the derived fingerprint matches, the instance is created, and the attested-device registry is empty (only this device).

### Tests for US5

- [ ] **T060** [P] [US5] Rust test: valid seed + correct fingerprint → instance created, `root_fingerprint` returned matches.
- [ ] **T061** [P] [US5] Rust test: valid seed + wrong claimed fingerprint → `FingerprintMismatch`, no file created.
- [ ] **T062** [P] [US5] `e2e/onboarding.spec.ts::"recover happy path"`: enter recorded seed → fingerprint shown → confirm → navigation succeeds.

### Implementation for US5

- [ ] **T063** [US5] Extend `create_instance` to route `CreateMode::Recover` to a `pairing::recover` helper. Same orphan-file guarantee.
- [ ] **T064** [US5] Un-disable the Recover radio in `CreateSheet.vue`; conditionally render seed-input field and fingerprint-match display.
- [ ] **T065** [P] [US5] i18n: `onboarding.create.recover.seed`, `onboarding.create.recover.fingerprintExpected`, `onboarding.create.recover.fingerprintActual`, `onboarding.create.recover.mismatch`.

**Checkpoint**: US1..US5 all shipped. Full spec implemented.

---

## Phase 8: Polish & Cross-Cutting

- [ ] **T066** [P] [-] Playwright network-assertion: build the production bundle, load in headless Chromium, assert zero requests to `api.iconify.design`, `fonts.googleapis.com`, `fonts.gstatic.com`, or any host outside the app's own scheme (satisfies SC-006).
- [ ] **T067** [P] [-] Playwright visual snapshot: landing with 0, 1, 10 instances (satisfies SC-007).
- [ ] **T068** [P] [-] Trash context menu on `InstancesList` items per FR-023: `<UiDialog>` confirms trash, then assert `instance-list-changed { reason: 'trashed' }` removes the item after the backend move completes. Do not expose list-remove-with-file-retained in v1 because the list is a directory scan without exclusion metadata; document this decision in `research.md`.
- [ ] **T069** [P] [-] Passphrase strength meter component `<OnboardingPassphraseStrength>` for CreateSheet and ConnectSheet — pure UI, drives `WeakPassphrase` prevention client-side to match the backend policy.
- [ ] **T070** [-] Run [`quickstart.md`](./quickstart.md) end-to-end from a fresh clone; fix any drift; commit updates.
- [ ] **T071** [P] [-] Add `security-review` skill pass over the passphrase / seed / token handling paths. Address findings in a follow-up commit.

---

## Dependencies & Execution Order

### Phase Dependencies

- Setup (Phase 1) → no dependencies, can start immediately (frontend scaffolding alone).
- Foundational (Phase 2) → BLOCKED on `haex-crdt` extraction.
- User stories → BLOCKED on Foundational.
- Polish → depends on the user stories it decorates.

### User Story Dependencies

- **US4** (P1): Depends on Foundational. Chosen first because it exercises the smallest end-to-end path (Pinia + Sheet + i18n + one Tauri command).
- **US1** (P1): Depends on Foundational. Independent of US4 for the backend, but shares the landing page — merge order matters at the CTA level.
- **US2** (P1): Depends on Foundational + US1's `create_instance` skeleton (extends its enum branch). Otherwise independent.
- **US3** (P2): Depends on Foundational only.
- **US5** (P3): Depends on US1 (extends CreateSheet + create_instance).

### Parallel Opportunities

- All Phase 1 setup tasks marked `[P]` can run in parallel by different agents.
- Within a user story, `[P]` tasks target different files.
- US2 and US3 can proceed in parallel once US1's CreateSheet exists (US2 and US3 add sibling CTAs).

---

## Implementation Strategy

### MVP Definition

MVP = US4 + US1 + US2 shipped end-to-end, plus a passing `T042` E2E test using two running Tauri instances, real relay delivery, `haex-crdt` synchronization, and the two-device ping/pong walking-skeleton (`v1-scope-design.md §11`).

### Suggested Incremental Delivery

1. Complete Phase 1 (Setup). Landing page renders "hello Nuxt".
2. Wait on `haex-crdt`. In parallel, add Playwright + Vitest scaffolding.
3. Complete Phase 2 (Foundational). Empty landing shell + working store.
4. Complete Phase 3 (US4). Manual seed → unlock flow works.
5. Complete Phase 4 (US1). Anlegen (Genesis) → Unlock loop is closed.
6. Complete Phase 5 (US2). Two-device walking-skeleton reachable via UI.
7. Complete Phase 6 (US3). Portability.
8. Complete Phase 7 (US5). Recovery.
9. Complete Phase 8 (Polish). Ship v1.
