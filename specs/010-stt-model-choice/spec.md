# Feature Specification: STT Model Choice

**Feature Branch**: `010-stt-model-choice`
**Created**: 2026-09-17
**Status**: Draft
**Input**: User description: "Let users choose their local speech-to-text (STT) model, with the same workflow the app already uses for the local chat/agent model — a hardware-fit-annotated tier catalog picked once during first-run onboarding and changeable later in Settings. STT model storage must not be Whisper-specific, since the local transcription backend could change later. External/API-based transcription providers stay out of scope (that's the separate, still-unbuilt US3 of spec 008)."

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Pick an STT model during first-run onboarding (Priority: P1)

A user sets up the vault on a new device. After naming the device and choosing a chat/agent model, they are shown a recommended set of local speech-to-text model options sized to their hardware, and pick one before entering the workspace.

**Why this priority**: This is the workflow gap the feature exists to close — today the transcription model is silently fixed with no say from the user, unlike every other local model choice in onboarding.

**Independent Test**: Complete first-run onboarding on a fresh device and confirm a speech-to-text model tier is presented and selectable, and that dictation subsequently uses the picked model.

**Acceptance Scenarios**:

1. **Given** a user has just chosen a chat/agent model during onboarding, **When** they proceed to the next step, **Then** they see a small set of speech-to-text model tiers recommended for their hardware, each showing enough information to judge the size/quality trade-off.
2. **Given** the speech-to-text tier list is shown, **When** the user picks one, **Then** that model is downloaded, becomes the active transcription model on this device, and the user proceeds to the workspace once the download completes.
3. **Given** the speech-to-text tier list is shown, **When** the user chooses to decide later instead of picking one, **Then** onboarding completes without downloading anything extra, and dictation still works using a sensible built-in default the first time it is used.

---

### User Story 2 - Change the active local STT model later (Priority: P2)

A user who already completed onboarding (or skipped the STT step) opens Settings and changes which local speech-to-text model is active on this device.

**Why this priority**: Needed so the choice isn't locked in at first run, and so users who skipped during onboarding aren't stuck with the default forever — matches the existing ability to change the default chat/agent model after onboarding.

**Independent Test**: From Settings, switch the active local speech-to-text model to a different tier and confirm the next dictation uses it, without restarting the app.

**Acceptance Scenarios**:

1. **Given** a user is on the Settings page, **When** they open the speech-to-text model section, **Then** they see which local model is currently active and which other tiers are available (installed or not yet downloaded).
2. **Given** the user picks a different, not-yet-downloaded tier, **When** the download finishes, **Then** it becomes the active model for this device.
3. **Given** the user switches to an already-downloaded tier, **When** they next dictate, **Then** transcription uses the newly active model without requiring an app restart.

---

### Edge Cases

- What happens if the user picks an STT tier during onboarding but the download fails or is interrupted? (Onboarding must not get stuck — the user needs a way to retry or skip and fall back to the default, consistent with how a failed chat-model download is handled today.)
- What happens if the device has no network access during the onboarding STT step? (Same fallback as skipping: proceed with the built-in default, downloaded lazily on first real use.)
- What happens if the currently active STT model's files are missing or corrupted on disk when a dictation starts (e.g. deleted by hand)? (Falls back to re-downloading the active tier rather than failing silently.)
- What happens if a user switches the active STT model while a recording/transcription is already in progress? (The in-flight transcription completes using the model that was active when it started; the switch takes effect starting with the next recording.)

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: The system MUST offer a small, curated set of local speech-to-text model tiers (not an open-ended search), each annotated with a hardware-fit recommendation for the current device, the same way the chat/agent model catalog already is.
- **FR-002**: The onboarding wizard MUST present the speech-to-text model choice as a fixed step immediately after the chat/agent model choice, on every first run on a new device — this three-step order (device name → chat/agent model → speech-to-text model) is the standing onboarding sequence going forward.
- **FR-003**: Users MUST be able to skip the speech-to-text model choice during onboarding and still get a fully working, offline dictation experience via a built-in default.
- **FR-004**: The system MUST persist the chosen speech-to-text model as a per-device setting, independent of the chat/agent model choice.
- **FR-005**: Users MUST be able to view and change the active local speech-to-text model from Settings at any time after onboarding, including switching to a tier not yet downloaded (triggering its download) or to one already installed.
- **FR-006**: A change to the active speech-to-text model MUST take effect for the next dictation without requiring the user to restart the application.
- **FR-007**: The system MUST continue to work exactly as it does today for a user who never interacts with this feature (no forced choice, same default model, same lazy first-use download behavior).
- **FR-008**: The set of speech-to-text tiers and how they are stored MUST NOT assume any particular transcription engine or file format — the underlying local transcription backend is expected to change over time, and the storage design must not need to change alongside it.
- **FR-009**: Choosing or configuring an external/API-based transcription service is explicitly out of scope for this feature (tracked separately as spec 008's unbuilt "external transcription source" story).

### Key Entities

- **STT Model Tier**: One selectable local speech-to-text model option — a display name, an approximate size/resource footprint, and a hardware-fit verdict for the current device. Analogous to a chat/agent model catalog entry, but for transcription.
- **Active STT Model (per device)**: The one STT Model Tier currently in effect for a given device, persisted as a device-scoped setting, defaulting to the smallest built-in tier when never explicitly chosen.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: A user completing first-run onboarding on a new device can see and choose a speech-to-text model in the same session, without visiting a separate settings screen.
- **SC-002**: A user can go from "wrong STT model active" to "correct one active and in use" in under two minutes for an already-downloaded tier, entirely from Settings.
- **SC-003**: 100% of users who skip the onboarding STT step still get working dictation on first use, with no error state reachable purely by skipping.
- **SC-004**: Switching the active local STT model never requires an application restart to take effect.

## Assumptions

- The three onboarding steps (device name, chat/agent model, speech-to-text model) are always shown in that fixed order on first run on a new device; this ordering itself is now a standing product decision, not specific to this feature.
- "Local" speech-to-text models only are in scope; external/API-based transcription providers are a separate, already-tracked, not-yet-built capability (spec 008 US3) and are unaffected by this feature.
- The number and sizing of speech-to-text tiers offered mirrors the existing three-tier (small/medium/large-equivalent) presentation used for chat/agent models, so the two choices feel consistent to the user.
- Storage and cataloging of speech-to-text models reuses the same general-purpose, backend-agnostic model storage location already used for chat/agent models, rather than a separate, transcription-engine-specific location — this is what makes FR-008 (no engine-specific assumptions) achievable without new infrastructure.
- No changes are made to how chat/agent models themselves are catalogued, stored, or selected, beyond factoring out logic that both the chat and speech-to-text tier recommendations can share.
