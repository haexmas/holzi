# Tasks: Voice Control (Local Speech-to-Text)

**Input**: Design documents from `specs/008-voice-control-stt/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/tauri-commands.md](contracts/tauri-commands.md), [quickstart.md](quickstart.md)

**Tests**: Included — this repo's constitution requires tests alongside behavior changes, and rust
tests always live in a dedicated `*_tests.rs` file next to the tested module (never inline
`#[cfg(test)] mod tests`). The repo currently has no frontend test runner (no `vitest`/component
harness); frontend verification uses `pnpm typecheck` plus the manual `quickstart.md` walkthroughs
per user story, matching how prior specs (e.g. 005) verify their frontend.

**Organization**: Tasks are grouped by user story (spec.md: US1/US2 both P1, US3 P2) so each story
is independently implementable and testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Maps the task to US1 / US2 / US3 from spec.md

## Phase 1: Setup

- [ ] T001 Add `cpal` and `candle-transformers` (pinned to 0.10.2, matching the already-pinned
      `candle-core`/`candle-nn` pulled in by `mistralrs`) to `src-tauri/Cargo.toml`, gated behind
      the existing `llm-cpu` feature (the feature that already governs whether the candle
      dependency tree is built at all — keeps `cargo build --no-default-features` skipping both
      LLM and STT candle work, per the existing dev-iteration escape hatch documented in
      `Cargo.toml`).

**Checkpoint**: Dependencies resolve; `cargo build --no-default-features` still skips the candle
tree entirely.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared infrastructure every user story needs. No user story can be verified
end-to-end until this phase is done.

- [ ] T002 [P] Add a `capability` column (`chat` | `transcription`, `NOT NULL DEFAULT 'chat'`) to
      the `providers` table via a new migration (alongside the existing migrations referenced from
      `src-tauri/src/storage/providers.rs`), backfilling existing rows to `chat`.
- [ ] T003 Update `find_local_provider` in `src-tauri/src/providers/local.rs` to filter on
      `kind = 'local' AND capability = 'chat'` (depends on T002 — once a local *transcription*
      provider row exists, the old unqualified `kind = 'local'` query can return the wrong row
      depending on `created_at` ordering).
- [ ] T004 [P] Define the `SttAdapter` trait (`transcribe(pcm: &[f32]) -> Result<String, SttError>`)
      in `src-tauri/src/stt/mod.rs` — narrower than `ProviderAdapter` (no streaming, no context
      window).
- [ ] T005 Implement `ensure_local_transcription_provider` in `src-tauri/src/providers/local.rs`,
      mirroring `ensure_local_provider` (kind `Local`, capability `Transcription`, adapter
      `"whisper-local"`, name `"Gebündelt (offline)"`); idempotent, singleton row (depends on T002,
      and touches the same file as T003 — sequenced after it).
- [ ] T006 [P] Implement audio capture in `src-tauri/src/audio/mod.rs`: `cpal`-based start/stop/
      cancel against an in-memory PCM buffer, enforcing the maximum recording duration (FR-017) by
      stopping capture (not the session) once the cap is hit.
- [ ] T007 [P] Implement the interrupt matcher in `src-tauri/src/stt/interrupt.rs`:
      `match_interrupt(transcript: &str) -> Option<InterruptCommand>`, normalizing (trim,
      lowercase) and requiring the **entire** transcript to equal `"stop"`, `"halt"`, or
      `"abbrechen"` (FR-006, FR-009).
- [ ] T008 Unit tests for the interrupt matcher in `src-tauri/src/stt/interrupt_tests.rs`: exact
      matches for all three words and their case variants; a sentence merely containing one of the
      words (e.g. "bitte nicht mehr stoppen mitten im Satz") must NOT match; empty string must not
      match (depends on T007).
- [ ] T009 Register the `start_voice_recording`, `stop_voice_recording`, and
      `cancel_voice_recording` Tauri commands (per
      [contracts/tauri-commands.md](contracts/tauri-commands.md)) in `src-tauri/src/lib.rs`, wiring
      them to the audio module (T006) and the interrupt matcher (T007); emit the
      `voice-recording-capped` event when the max-duration cap fires. Transcription dispatch itself
      is stubbed to a `NoProviderConfigured` error until US1 wires the local adapter (depends on
      T004, T006, T007).

**Checkpoint**: Schema, adapter trait, audio capture, interrupt matching, and command scaffolding
exist. No story is end-to-end functional yet — that starts in Phase 3.

---

## Phase 3: User Story 1 - Dictate a chat message (Priority: P1) 🎯 MVP

**Goal**: Press-hold-release the mic control, speak a sentence, see it transcribed into the chat
input field and sent (auto-send default on).

**Independent Test**: Record a short spoken sentence via the mic control; verify it appears in the
chat input field and is sent through the normal chat path.

### Tests for User Story 1

- [ ] T010 [P] [US1] `LocalWhisperAdapter` test against a small fixed audio fixture (checked into
      the repo, e.g. `src-tauri/src/stt/fixtures/`) with a known expected transcript, in
      `src-tauri/src/stt/local_tests.rs`. No real microphone, no network.

### Implementation for User Story 1

- [ ] T011 [US1] Implement `LocalWhisperAdapter` in `src-tauri/src/stt/local.rs`: loads the bundled
      Whisper checkpoint and implements `SttAdapter::transcribe` via `candle-transformers` (depends
      on T004, T010).
- [ ] T012 [US1] Bundle the Whisper checkpoint(s) as Tauri resources (`src-tauri/resources/whisper/`,
      declared in `tauri.conf.json` `bundle.resources`) so they ship inside the installer rather
      than being downloaded at first run (FR-010); select the tier (tiny/base/small) at load time
      using the existing hardware-detection logic from spec 002 (depends on T011).
- [ ] T013 [US1] Wire `stop_voice_recording` (T009) to dispatch to the currently active
      `SttAdapter` — defaulting to `LocalWhisperAdapter` via `ensure_local_transcription_provider`
      (T005) — and return the `TranscriptionResult` shape from the contract (depends on T009, T011,
      T012).
- [ ] T014 [P] [US1] `VoiceInputControl.vue` mic control in `src/components/chat/`: idle/recording/
      transcribing/error states, calling `start_voice_recording`/`stop_voice_recording`, writing a
      non-interrupt `text` result into the existing chat input field.
- [ ] T015 [US1] Auto-send wiring: read/write the `voice.auto_send` preference (vault-scoped) via
      the existing `get_pref`/`set_pref` commands; when enabled (default), send the transcribed
      message through the existing send-message path immediately; when disabled, leave it editable
      in the input field (FR-005, FR-006) (depends on T014).
- [ ] T016 [US1] Empty-transcript handling: when `text` is empty (no speech detected), leave the
      input field and message history untouched, and show a non-blocking indicator ("keine Sprache
      erkannt") (FR-014) (depends on T014).
- [ ] T017 [US1] Manual verification: run [quickstart.md](quickstart.md) §1 end to end.

**Checkpoint**: User Story 1 is fully functional and independently testable/demoable (MVP).

---

## Phase 4: User Story 2 - Interrupt the assistant by voice (Priority: P1)

**Goal**: Speaking "stop"/"halt"/"abbrechen" alone immediately stops an in-progress assistant
response, regardless of what the assistant is doing; the same words inside a longer dictated
sentence do not trigger it.

**Independent Test**: While the assistant is generating, record one of the three words and verify
the response stops immediately; then dictate a full sentence containing one of the words and
verify it is *not* interrupted.

### Tests for User Story 2

- [ ] T018 [P] [US2] Test that an interrupt command cancels an in-progress turn via the existing
      `CancellationToken`, independent of the turn/assistant's current state, in
      `src-tauri/src/chat/turn_tests.rs`.

### Implementation for User Story 2

- [ ] T019 [US2] In the `stop_voice_recording` handler (T013), when `match_interrupt` (T007)
      returns a command, trigger the existing `CancellationToken` for the active turn in
      `src-tauri/src/chat/session.rs` before returning — this path MUST NOT depend on the
      assistant/LLM being idle or available (FR-008) (depends on T013, T018).
- [ ] T020 [US2] In `VoiceInputControl.vue` (T014), when the result's `interrupt` field is set, do
      **not** write `text` to the input field or send anything — only reflect that an interrupt
      fired (depends on T014, T019).
- [ ] T021 [US2] Manual verification: run [quickstart.md](quickstart.md) §2 end to end, including
      the negative case (interrupt word inside a longer sentence must not trigger).

**Checkpoint**: User Stories 1 and 2 both work independently; voice interrupt is reliable
regardless of assistant state.

---

## Phase 5: User Story 3 - Choose a transcription source (Priority: P2)

**Goal**: Configure and activate an external transcription service as an alternative to the
bundled local one, with a persistent indicator while it is active, and switch back cleanly.

**Independent Test**: Add and activate an external transcription service with valid test
credentials, dictate a message, and confirm transcription happened via that service rather than
the local one.

### Tests for User Story 3

- [ ] T022 [P] [US3] `ExternalSttAdapter` test against a mocked HTTP boundary (`wiremock`) in
      `src-tauri/src/stt/external_tests.rs`: request shape, success mapping, `401` →
      `InvalidCredentials`, unreachable/timeout → a surfaced, non-panicking error.

### Implementation for User Story 3

- [ ] T023 [US3] Implement `ExternalSttAdapter` in `src-tauri/src/stt/external.rs`: the
      multipart transcription protocol decided in [research.md](research.md) §7, using
      `base_url` + credentials from the provider row (depends on T004, T022).
- [ ] T024 [US3] Add the additive `capability` field to `AddProviderArgs`/`ProviderPayload` in
      `src-tauri/src/providers/mod.rs` (default `chat`); wire `capability: "transcription"` +
      `kind: "api_key"` creation to build an `ExternalSttAdapter` (depends on T002, T023).
- [ ] T025 [US3] Resolve the active `SttAdapter` in the `stop_voice_recording` dispatch (T013) from
      the `voice.active_stt_provider` device preference, falling back to the local provider
      (T005) if the preference is missing or points at a deleted row (depends on T013, T024).
- [ ] T026 [P] [US3] Extend the existing "Modelle verwalten" area with a transcription tab: list/
      add/activate transcription providers, reusing the existing provider-management UI patterns;
      read/write `voice.active_stt_provider` via `get_pref`/`set_pref`.
- [ ] T027 [US3] In `VoiceInputControl.vue` (T014), show a persistent visible signal for as long as
      the active source is external (FR-021), sourced from the same preference T026 writes
      (depends on T014, T026).
- [ ] T028 [US3] Manual verification: run [quickstart.md](quickstart.md) §3 end to end, including
      the invalid-credentials negative case.

**Checkpoint**: All three user stories are independently functional.

---

## Final Phase: Polish & Cross-Cutting Concerns

- [ ] T029 [P] Manual verification: run [quickstart.md](quickstart.md) §4 (mic permission denied,
      backgrounding mid-recording, max-duration cap, fresh-install first use with zero
      configuration).
- [ ] T030 [P] Add the new user-facing strings (mic control states, permission errors, external-
      source indicator) to `src/i18n/locales/de.json` and `src/i18n/locales/en.json`.
- [ ] T031 Run `cargo test` (full workspace) and `pnpm typecheck`; confirm both are green before
      opening the PR.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup. Blocks every user story.
- **User Story 1 (Phase 3)**: Depends on Foundational only. This is the MVP slice.
- **User Story 2 (Phase 4)**: Depends on Foundational; T019/T020 additionally depend on US1's
  `stop_voice_recording` wiring (T013) and mic control (T014) existing, since it extends the same
  handler and component rather than duplicating them.
- **User Story 3 (Phase 5)**: Depends on Foundational; T025/T027 additionally depend on US1's T013
  /T014 for the same reason.
- **Polish (Final Phase)**: Depends on all desired user stories being complete.

Note: unlike a typical fully-independent-stories layout, US2 and US3 each extend one shared
handler (`stop_voice_recording`) and one shared component (`VoiceInputControl.vue`) that US1
introduces, rather than owning separate files — this mirrors the spec's own framing (voice input
is one pipeline with three behavioral facets, not three separate features). Each story is still
independently *testable* per its Independent Test above; US1 must simply land first.

### Within Each User Story

- Tests before implementation where a test task is listed.
- Backend adapter/handler work before the frontend task that calls it.
- Story complete and manually verified (quickstart.md) before moving to the next priority.

### Parallel Opportunities

- Foundational: T002, T004, T006, T007 touch independent files and can run in parallel; T003 and
  T005 both land in `providers/local.rs` and must be sequenced after T002 (and after each other).
- US1: T010 (test) and T014 (frontend component) can start in parallel with each other; T011–T013
  form a sequential backend chain.
- US2/US3 test tasks (T018, T022) can be written in parallel with each other and with unrelated
  Foundational/US1 work, once T007/T004 exist respectively.
- T029 and T030 in the Polish phase are independent of each other.

---

## Parallel Example: Foundational Phase

```bash
Task: "Add capability column migration in src-tauri/src/storage/providers.rs"
Task: "Define SttAdapter trait in src-tauri/src/stt/mod.rs"
Task: "Implement audio capture in src-tauri/src/audio/mod.rs"
Task: "Implement interrupt matcher in src-tauri/src/stt/interrupt.rs"
```

## Parallel Example: User Story 1

```bash
Task: "LocalWhisperAdapter test with fixed audio fixture in src-tauri/src/stt/local_tests.rs"
Task: "VoiceInputControl.vue mic control in src/components/chat/"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 (Setup) → Phase 2 (Foundational) → Phase 3 (US1).
2. Stop and validate US1 via quickstart.md §1 — dictation-to-send fully working, offline, with the
   bundled model.
3. This alone is a demoable MVP: voice replaces typing for sending chat messages.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. US1 → validate → demo (MVP: dictate and send).
3. US2 → validate → demo (adds reliable voice interrupt — important before voice becomes the
   primary input method for a user, since without it there is no hands-free way to stop the
   assistant).
4. US3 → validate → demo (adds external transcription choice — purely additive, no prior story's
   behavior changes when this is skipped or delayed).
5. Polish.
