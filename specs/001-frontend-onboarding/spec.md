# Feature Specification: Frontend Onboarding — Landing, Create, Open, Connect

**Feature Branch**: `001-frontend-onboarding`
**Created**: 2026-09-04
**Status**: Draft
**Input**: Brainstorming session captured in this repository's conversation on 2026-09-04. Design orientation follows `haex-vault`'s landing/onboarding pattern, adapted to holzi's identity, storage, and pairing model.

**Relationship to prior docs**:

- Consumes decisions from [`docs/design/founding.md`](../../docs/design/founding.md) and [`docs/plans/2026-09-04-v1-scope-design.md`](../../docs/plans/2026-09-04-v1-scope-design.md).
- Revises `founding.md` §2.2 ("Device equals relay equals Tauri application") to allow multiple SQLite database files on disk per install, with exactly one active at runtime. See **Assumptions** below.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Anlegen (Genesis of a new federation) (Priority: P1) 🎯 MVP

The operator installs holzi for the first time on a device and creates a fresh federation. On the landing page, they choose **Anlegen**, name the instance, set a passphrase, are shown the paper-seed bundle, confirm they have recorded it, and are dropped into the running instance.

**Why this priority**: Without Anlegen, no federation exists. It is the entry point for every subsequent scenario and must ship for the v1 walking-skeleton.

**Independent Test**: On a fresh install with an empty `instances/` directory, complete the Anlegen sheet with valid inputs; verify (1) a new `<name>.db` file appears in `<AppLocalData>/instances/`, (2) the paper-seed is displayed exactly once, (3) after confirmation the app navigates to the federation surface and Nostr relay + iroh peer are running.

**Acceptance Scenarios**:

1. **Given** no existing instance and the Anlegen sheet open, **When** the operator submits name, passphrase, and paper-seed confirmation, **Then** the instance file is created, the app unlocks it, and navigates to the federation view.
2. **Given** the Anlegen sheet open, **When** the operator submits a name that already exists in `instances/`, **Then** an inline error is shown and no file is created.
3. **Given** the paper-seed display step, **When** the operator dismisses the sheet without confirming, **Then** the instance file is either not created or is discarded and removed from the list.

---

### User Story 2 — Verbinden (Join existing federation via pairing) (Priority: P1) 🎯 MVP

The operator has an already-attested device (parent) running holzi and installs holzi on a second device (joiner). On the joiner's landing they choose **Verbinden**, paste a short-lived token that the parent displays, set a local passphrase, and complete pairing. The joiner's new instance is created, the pairing transcript is co-signed, and federation state syncs from the parent via `haex-crdt`. Camera-based QR scanning is deferred to a follow-up spec.

**Why this priority**: This is the second half of the v1 walking-skeleton (`v1-scope-design.md §11`: two devices paired into one federation exchanging a ping). Without Verbinden, holzi is single-device.

**Independent Test**: With a parent device running an attested instance and displaying a valid pairing token, on the joiner complete the Verbinden sheet; verify (1) a new `<name>.db` file appears on the joiner, (2) the attestation ring in `haex-crdt` contains both devices on both sides after sync, (3) the joiner's presence event appears in the parent's registry.

**Acceptance Scenarios**:

1. **Given** a valid, unexpired, unused pairing token from the parent, **When** the joiner submits it with a local passphrase, **Then** pairing succeeds and the joiner is dropped into the federation view.
2. **Given** an expired or already-consumed token, **When** the joiner submits it, **Then** an explicit error identifies the token state and no instance file is created.
3. **Given** a token with a mismatched federation epoch (e.g., stale token), **When** the joiner submits it, **Then** pairing fails with a clear "federation state changed" error.

---

### User Story 3 — Öffnen (Import external `.db` file from filesystem) (Priority: P2)

The operator has an instance file (`.db`) on the host filesystem — for example moved from another machine on a USB stick, restored from backup, or checked out of a personal sync location — and wants holzi to manage it. On the landing they choose **Öffnen**, pick the file via OS file dialog, and holzi copies it into `<AppLocalData>/instances/` under its original name (or a de-duplicated variant). It then appears in the "Zuletzt verwendet" list and can be unlocked normally.

**Why this priority**: Portability. Holzi has no export command because the `.db` file *is* the export; Öffnen is the corresponding import. Not required for the walking-skeleton, but essential for cross-device migration in v1.

**Independent Test**: Place a valid holzi `.db` file outside `<AppLocalData>/instances/`; open the Öffnen sheet, pick the file; verify (1) the file is copied (not moved) into `instances/`, (2) the source file remains untouched, (3) the copied file appears in the list, (4) Unlock with the original passphrase succeeds.

**Acceptance Scenarios**:

1. **Given** a valid `.db` file at an arbitrary path, **When** the operator selects it in the Öffnen sheet, **Then** the file is copied into `instances/`, appears in the list, and unlocks with its original passphrase.
2. **Given** a file whose name conflicts with an existing instance, **When** the operator confirms the import, **Then** the operator is prompted for either overwrite, rename, or cancel — default behavior is rename with a numeric suffix, no silent overwrite.
3. **Given** a structurally valid regular `.db` file that was imported successfully, **When** the operator attempts to unlock it with an incorrect passphrase, **Then** `open_instance` rejects the attempt with an explicit error, retains the pending imported copy and marker for a later unlock attempt or explicit discard, and leaves any currently active instance unchanged; structural source failures are still rejected before copying.

---

### User Story 4 — Zuletzt verwendet + Unlock (Priority: P1) 🎯 MVP

On every subsequent launch, the landing shows the operator's instances (from `<AppLocalData>/instances/`, sorted by last-access descending). Clicking one opens the Unlock sheet; entering the passphrase brings up the federation view.

**Why this priority**: Without Unlock, every launch would force re-Anlegen. This is the daily entry point and must ship for v1.

**Independent Test**: With at least one instance in `instances/`, launch the app; verify (1) the instance appears in the list with its correct alias and last-access timestamp, (2) clicking opens the Unlock sheet, (3) correct passphrase unlocks and navigates to the federation view, (4) wrong passphrase shows an inline error without leaking whether the file exists or not.

**Acceptance Scenarios**:

1. **Given** one or more instances present, **When** the operator launches the app, **Then** the list is populated ordered by last-access desc.
2. **Given** the Unlock sheet with the correct passphrase, **When** the operator submits, **Then** the instance opens and navigation succeeds within 2 seconds on desktop hardware.
3. **Given** the Unlock sheet with a wrong passphrase, **When** the operator submits, **Then** an inline error appears and no state changes.
4. **Given** an active instance already open, **When** the operator opens a different instance from the list, **Then** the current instance is closed first (Nostr relay stopped, iroh peer stopped, SQLite closed) before the new one opens.

---

### User Story 5 — Aus Paper-Seed wiederherstellen (Recover) (Priority: P3)

No attested device survives, and the operator has only their paper-seed. From the Anlegen sheet they choose the "Recover" sub-mode, enter the seed, and confirm the federation-root fingerprint matches. A new local instance is created with the recovered federation-root; the attested-device registry is empty (no other devices survived). The operator can then Verbinden other newly-installed devices as normal.

**Why this priority**: Recovery is the "in case of emergency" path. It is functionally implied by the paper-seed design in v1-scope-design.md §4, but the initial walking-skeleton does not require it. Can ship post-MVP.

**Independent Test**: Delete all instance files; on a fresh install, choose Anlegen → Recover; enter a previously-recorded paper-seed; verify (1) the fingerprint displayed matches the one shown at Genesis, (2) upon confirmation an instance is created whose federation-root public key matches, (3) the attested-device registry contains only this device.

**Acceptance Scenarios**:

1. **Given** a valid paper-seed and passphrase, **When** the operator submits Recover, **Then** the instance is created and the fingerprint match is displayed before the final confirm.
2. **Given** a paper-seed whose derived public key does not match the fingerprint the operator claims, **When** the operator submits, **Then** recovery is aborted with an explicit fingerprint-mismatch error and no instance file is created.

---

### Edge Cases

- **Passphrase entered incorrectly during Create**: two-field entry with match check; submit disabled until match.
- **Passphrase field visibility**: masked by default, reveal on hold (mouse) or tap (mobile); never persisted in the browser autofill store.
- **Instance name collision with reserved filename** (`.trash`, files starting with `.`, path traversal): rejected client-side with clear error before any command is sent.
- **List refresh after external mutation** (e.g., another process or CLI creates a file in `instances/`): Tauri backend emits `instance-list-changed`; Pinia store re-syncs; list updates without page reload.
- **App closed mid-Anlegen (before paper-seed confirmation)**: on next launch, an orphan file may exist. Behavior: on startup, delete any Genesis file in `instances/` whose creation flag `.pending` still exists in the same directory. The pending flag is written before the DB and removed only by `confirm_create` after paper-seed confirmation. Imported files use a separate import-pending marker and remain available for unlock validation.
- **Two instances open concurrently**: `open_instance` serializes the close-and-open switch under the backend state lock. It validates the requested credentials while the current runtime remains active; a validation failure leaves that runtime and `AppState.active_instance` unchanged. The frontend does not call `close_instance` first; a concurrent request waits for the lock and then observes either the old or the new fully-active instance, never a half-switched state.
- **Mobile foreground/background** for Anlegen: if the app is backgrounded during Genesis before paper-seed confirmation, the same pending-flag mechanism applies. No changes to relay-lifetime rules for mobile beyond `v1-scope-design.md §7`.

## Requirements *(mandatory)*

### Functional Requirements

**Landing page (all first-launch and subsequent launch)**

- **FR-001**: The landing page MUST display three primary call-to-action buttons in fixed order: **Anlegen**, **Öffnen**, **Verbinden**.
- **FR-002**: The landing page MUST display a "Zuletzt verwendet" list of all instances in `<AppLocalData>/instances/`, sorted by last-access descending.
- **FR-003**: The landing page MUST render a version string sourced from `@tauri-apps/api/app::getVersion()`.
- **FR-004**: The landing page MUST work with an empty `instances/` directory (the list section MUST hide, not render an empty state that competes with the CTAs).

**Anlegen (Create)**

- **FR-005**: The Anlegen action MUST open a Sheet (side panel) with two mode choices: "Neue Federation" (Genesis, default) and "Aus Paper-Seed wiederherstellen" (Recover).
- **FR-006**: The Anlegen sheet MUST require: instance name (alphanumeric plus dash/underscore, ≤64 chars, unique within `instances/`), passphrase (min length per policy, entered twice), and paper-seed confirmation (Genesis) OR paper-seed entry with fingerprint match (Recover).
- **FR-007**: On successful submit, the backend MUST create `<AppLocalData>/instances/<name>.db`, initialize `haex-crdt` with the passphrase, run the mode-specific initialization, activate the Genesis runtime, and return the paper-seed (Genesis) or matched fingerprint (Recover). Genesis remains pending until the confirmation boundary.
- **FR-008**: The paper-seed MUST be displayed for reading in a Genesis flow; the operator MUST explicitly confirm they recorded it, causing the frontend to invoke `confirm_create` before the Anlegen sheet closes. `confirm_create` removes the Genesis `.pending` marker while leaving the instance active.
- **FR-009**: If Anlegen is cancelled after the DB file is created but before paper-seed confirmation, the frontend MUST invoke `abort_create`; the backend MUST stop the pending runtime and discard the file and marker. Startup cleanup MUST apply the same rule after a crash.

**Öffnen (Import external `.db` file)**

- **FR-010**: The Öffnen action MUST open a Sheet that invokes the OS file picker via `@tauri-apps/plugin-dialog`, restricted to `.db` extension.
- **FR-011**: Upon selection, the backend MUST validate the source as a regular `.db` file before copying. SQLCipher credential validation MUST occur in `open_instance`, using the passphrase entered in the Unlock sheet; a failed unlock attempt, including an incorrect passphrase, MUST return an explicit error without deleting the pending imported copy or marker. Deletion requires an explicit discard action or conclusive validation that the file is not a holzi instance.
- **FR-012**: The backend MUST copy (not move) the file into `<AppLocalData>/instances/` preserving its filename basename and mark the copy as pending validation until a successful unlock.
- **FR-013**: On name collision, the backend MUST prompt via return value; the frontend MUST offer overwrite / rename / cancel; default MUST be rename with numeric suffix (`<name>-2.db`). Silent overwrite is prohibited.
- **FR-014**: After successful copy, the `instances/` list MUST refresh (via `instance-list-changed` event) so the imported file appears immediately.

**Verbinden (Join federation via pairing)**

- **FR-015**: The Verbinden action MUST open a Sheet with a text-input field for the pairing token. QR/camera scanning is explicitly deferred from v1 to a follow-up spec.
- **FR-016**: The Verbinden sheet MUST require: instance name (as FR-006), passphrase (as FR-006), and pairing token entered as text.
- **FR-017**: On submit, the backend MUST create a new local instance, generate device-scoped Nostr and iroh keypairs, connect to the parent device's relay using the token's contact hint, sign the canonical pairing transcript, wait for the parent's co-signature, and persist the resulting attestation into `haex-crdt`.
- **FR-018**: If the pairing token is expired, already-consumed, or its transcript rejected by the parent, the backend MUST discard the local instance file (same guarantee as FR-009).

**Zuletzt verwendet + Unlock**

- **FR-019**: Clicking an instance in the list MUST open the Unlock sheet with only a passphrase field.
- **FR-020**: On correct passphrase, the backend MUST unlock SQLCipher, start the Nostr relay endpoint, start the iroh peer, and mark the instance as active in `AppState`. The frontend MUST navigate to `/federation/<instance-id>`.
- **FR-021**: On incorrect passphrase, the sheet MUST show an inline error without disclosing whether the file exists or the passphrase policy was violated (avoid oracle).
- **FR-022**: If an instance is already active, `open_instance` MUST close the active one and open the requested instance as one atomic, state-locked backend switch. The frontend MUST NOT orchestrate a separate close/open sequence.
- **FR-023**: The list MUST support a per-item context menu with "In Papierkorb verschieben". Removing an item while retaining its file is not supported in v1 because the list is a directory scan and no exclusion metadata is persisted. Silent hard-delete MUST NOT be an option.

**Cross-cutting**

- **FR-024**: All backend mutations to `instances/` (create, open, close, import, trash) and all external changes observed by the directory watcher MUST emit `instance-list-changed` events; the Pinia store MUST subscribe and re-sync.
- **FR-025**: The frontend MUST NEVER pass managed-instance paths to backend commands; all instance-management commands MUST use instance names (basename without `.db`). `import_instance_file` MAY receive the external `source_path` returned by the OS file picker, while destination resolution and source-file validation remain backend authority.
- **FR-026**: All UI copy MUST be locale-driven via `@nuxtjs/i18n` with `de` and `en` locales at minimum; `de` is the default.
- **FR-027**: All icons MUST be delivered from the local bundle (`@iconify-json/lucide` package installed offline); no runtime request to any external icon API is permitted.

### Key Entities

- **Instance**: a `<name>.db` file in `<AppLocalData>/instances/`, containing a SQLCipher-encrypted `haex-crdt` store with federation state (attestation ring, revocation epochs, capability grants, chat/session history). Each instance is one identity in one federation.
- **InstanceInfo**: metadata surface for the frontend list: `{ name: string, alias: string, lastAccess: ISO8601, sizeBytes: number }`. `name` is the filename basename without `.db`; `alias` is the non-secret user-visible label, initially defaulting to `name`. No secrets.
- **CreateMode**: `Genesis | Recover { seed: string, expectedFingerprint: string } | Join { token: string }` — the three initialization modes across Anlegen + Verbinden.
- **PairingToken**: opaque short-lived value issued by a parent device, carrying (in encoded form) a Nostr contact hint, a one-time nonce, an expiry, and the current federation epoch.
- **PaperSeed**: versioned bundle displayed once at Genesis, containing federation-root seed material and federation-root public-key fingerprint. Sole federation-scope recovery secret.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A fresh operator can complete Anlegen (Genesis) end-to-end in under 90 seconds on desktop hardware, including reading the paper-seed prompt.
- **SC-002**: With a valid pairing token, Verbinden completes in under 30 seconds on the same local network, from token entry to federation view.
- **SC-003**: The landing page renders and becomes interactive within 500ms of app launch on desktop, 1500ms on mid-range Android hardware.
- **SC-004**: Unlock of a healthy instance succeeds within 2 seconds on desktop hardware, 4 seconds on mobile.
- **SC-005**: No user-facing UI text is displayed in a language other than the active locale (verified by i18n key coverage ≥100% for `de` and `en`).
- **SC-006**: No network request originates from the client to any icon or font CDN (verified by Playwright network-log assertion in the E2E suite).
- **SC-007**: The landing page renders correctly with 0, 1, and 10+ instances in the list (visual regression via Playwright screenshot).

## Assumptions

- The founding-doc assertion "device equals relay equals Tauri application" (`founding.md` §2.2) is refined to: **the *active* instance equals the running Nostr relay endpoint equals the running iroh peer**. The Tauri application is a container that may hold multiple `.db` files on disk, with exactly one active at any given time. This revision was surfaced during the 2026-09-04 brainstorming and takes precedence for v1.
- `<AppLocalData>` on each platform is the app-private data directory Tauri resolves via `BaseDirectory::AppLocalData`: Linux `$XDG_DATA_HOME/<bundle_identifier>/` (normally `~/.local/share/<bundle_identifier>/`), macOS `~/Library/Application Support/<bundle_identifier>/`, Windows `%LOCALAPPDATA%\<bundle_identifier>\`, Android app-private storage, and iOS app-sandbox `Library/Application Support/<bundle_identifier>/`. The `<bundle_identifier>` is the identifier configured in `tauri.conf.json`; all platforms append it before `instances/`.
- `haex-crdt` (extracted from `haex-vault` per `v1-scope-design.md §5`) provides the SQLite + CRDT layer with SQLCipher at-rest. This spec assumes `haex-crdt` exposes an initialization surface accepting a passphrase and a mode (Genesis / Recover / Join). Exact API is a `haex-crdt` concern.
- Pairing uses text-token input in v1. QR scanning through a desktop webcam or native Android/iOS camera API is deferred to a follow-up spec and must retain the same token contract when implemented.
- shadcn-vue components are copy-in under `src/components/ui/`. The initial component set is: `button`, `card`, `sheet`, `dialog`, `input`, `label`, `radio-group`, `sonner` (toasts), plus a custom `stepper` composed from `progress` + `button`. Later stories may add more.
- No cross-device blob or stream transfer is required by any onboarding flow. Verbinden's initial `haex-crdt` sync uses whatever transport `haex-crdt` chooses internally — not `blob.offer` (post-v1 per `v1-scope-design.md §2`).
