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

**2026-09-17 implementation note**: this pass implements Phases 1–3 (Setup, Foundational, US1 —
the dictate-and-send MVP) only, per an explicit operator scope decision. Phase 4 (US2, voice
interrupt), Phase 5 (US3, external transcription provider), and the Final Phase (polish + mobile
readiness gate) are **not implemented** and remain unchecked below. Two further operator decisions
changed how US1 itself was built, deviating from what this file and the rest of `specs/008-...`
originally specified:

- **Model delivery** (deviates from FR-010/SC-003 and quickstart.md's "no download" framing): the
  bundled Whisper model is downloaded into `<AppLocalData>/whisper/tiny/` on first use instead of
  being bundled as a Tauri resource in the installer, to avoid committing a ~75MB binary checkpoint
  into git history. See `src-tauri/src/stt/local.rs`'s module doc for the full rationale. FR-003/
  SC-004 (offline transcription once configured) still hold after the first download.
- **Model tier**: only the `tiny` multilingual model is used for every device. FR-011's per-device
  tiering assumed a reusable hardware-tier selector from spec 002 that, on inspection, does not
  exist as a standalone function — building one was out of scope for this pass.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Maps the task to US1 / US2 / US3 from spec.md

## Phase 1: Setup

- [x] T001 Add an independent `voice` Cargo feature and gate `cpal`, the audio module, and the
      voice commands behind it. Gate `candle-transformers` and `LocalWhisperAdapter` behind the
      existing `llm-cpu` feature, with both features enabled by default. Verify that
      `cargo build --no-default-features` has no voice references and that
      `cargo build --no-default-features --features voice` still compiles the shared capture and
      external-STT path without Candle; the latter must report `LocalSttUnavailable` rather than
      making external transcription unavailable.

**Checkpoint**: Dependencies resolve; no-feature builds skip both voice and Candle, while the
`voice`-only build keeps the external capture path compilable.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared infrastructure every user story needs. No user story can be verified
end-to-end until this phase is done.

- [x] T002 [P] Add a `capability` column (`chat` | `transcription`, `NOT NULL DEFAULT 'chat'`) to
      the `providers` table via a new migration (alongside the existing migrations referenced from
      `src-tauri/src/storage/providers.rs`), backfilling existing rows to `chat`.
- [x] T003 Update `find_local_provider` in `src-tauri/src/providers/local.rs` to filter on
      `kind = 'local' AND capability = 'chat'` (depends on T002 — once a local _transcription_
      provider row exists, the old unqualified `kind = 'local'` query can return the wrong row
      depending on `created_at` ordering).
- [x] T004 [P] Define `CanonicalPcm` and the `SttAdapter` trait
      (`transcribe(audio: &CanonicalPcm) -> Result<String, SttError>`) in
      `src-tauri/src/stt/mod.rs` — 16 kHz, mono, normalized `f32` samples in `[-1.0, 1.0]`,
      narrower than `ProviderAdapter` (no streaming, no context window). Define `SttError` as the
      STT-facing mapping of the existing `AdapterError`: preserve `InvalidCredentials` for
      external 401/403 responses, map transport/timeouts consistently, and map all adapter failures
      at the command boundary to `TranscriptionFailed { reason }`.
- [x] T005 Implement `ensure_local_transcription_provider` in `src-tauri/src/providers/local.rs`,
      mirroring `ensure_local_provider` (kind `Local`, capability `Transcription`, adapter
      `"whisper-local"`, name `"Gebündelt (offline)"`); idempotent, singleton row (depends on T002,
      and touches the same file as T003 — sequenced after it).
- [x] T006 [P] Implement audio capture in `src-tauri/src/audio/mod.rs`: `cpal`-based start/stop/
      cancel against an in-memory `CanonicalPcm` buffer, converting native sample types, channel
      counts, and sample rates at the capture boundary. Enforce the maximum recording duration
      (FR-017) by stopping capture (not the session) once the cap is hit. Add focused tests for
      non-default sample rates and channel counts, asserting that the converted canonical buffer
      reaches `SttAdapter::transcribe`.
- [x] T007 [P] Implement the interrupt matcher in `src-tauri/src/stt/interrupt.rs`:
      `match_interrupt(transcript: &str) -> Option<InterruptCommand>`, normalizing (trim,
      lowercase) and requiring the **entire** transcript to equal `"stop"`, `"halt"`, or
      `"abbrechen"` (FR-007, FR-009).
- [x] T008 Unit tests for the interrupt matcher in `src-tauri/src/stt/interrupt_tests.rs`: exact
      matches for all three words and their case variants; a sentence merely containing one of the
      words (e.g. "bitte nicht mehr stoppen mitten im Satz") must NOT match; empty string must not
      match (depends on T007).
- [x] T009 Register the `start_voice_recording`, `stop_voice_recording`, and
      `cancel_voice_recording` Tauri commands (per
      [contracts/tauri-commands.md](contracts/tauri-commands.md)) in `src-tauri/src/lib.rs`, wiring
      them to the audio module (T006) and the interrupt matcher (T007), gated with the `voice`
      feature. Emit the `voice-recording-capped` event when the max-duration cap fires and expose
      `voice-interrupt-detected` for the local fast path. Transcription dispatch itself is stubbed
      to a `NoProviderConfigured`/`LocalSttUnavailable` error until US1 wires the local adapter;
      the `voice`-only build must retain external STT support (depends on T004, T006, T007).
      **Partial**: `voice-recording-capped` is implemented; `voice-interrupt-detected` and the
      "local fast path during capture" are not — both are US2 (T019) work, out of scope for this
      pass. `match_interrupt` still runs on the final transcript so the `TranscriptionResult` wire
      shape matches the contract, but nothing consumes it to cancel a turn yet.

**Checkpoint**: Schema, adapter trait, audio capture, interrupt matching, and command scaffolding
exist. No story is end-to-end functional yet — that starts in Phase 3.

---

## Phase 3: User Story 1 - Dictate a chat message (Priority: P1) 🎯 MVP

**Goal**: Press the mic control, speak a sentence, press send, and see it transcribed into the chat
input field and sent (auto-send default on). Cancel discards the recording instead.

**Independent Test**: Record a short spoken sentence via the mic control; verify it appears in the
chat input field and is sent through the normal chat path.

### Tests for User Story 1

- [x] T010 [P] [US1] `LocalWhisperAdapter` test against a small fixed audio fixture (checked into
      the repo, e.g. `src-tauri/src/stt/fixtures/`) with a known expected transcript, in
      `src-tauri/src/stt/local_tests.rs`. No real microphone; the test permits the approved
      first-use download of the pinned tiny model and then exercises the cached files.

### Implementation for User Story 1

- [x] T011 [US1] Implement `LocalWhisperAdapter` in `src-tauri/src/stt/local.rs`: loads the local
      Whisper checkpoint and implements `SttAdapter::transcribe(&CanonicalPcm)` via
      `candle-transformers`, gated with `llm-cpu` (depends on T004, T010).
- [x] T012 [US1] Download the pinned tiny Whisper checkpoint files on first use into
      `<AppLocalData>/whisper/tiny/<revision>/` and reuse them for subsequent transcriptions.
      This approved MVP behavior avoids bundling a large checkpoint and uses the tiny tier on
      every device; hardware-based tier selection is deferred (depends on T011).
- [x] T013 [US1] Wire `stop_voice_recording` (T009) to dispatch to the currently active
      `SttAdapter` — defaulting to `LocalWhisperAdapter` via `ensure_local_transcription_provider`
      (T005) — and return the `TranscriptionResult` shape from the contract (depends on T009, T011,
      T012).
- [x] T014 [P] [US1] `VoiceInputControl.vue` mic control in `src/components/chat/`: idle/recording/
      transcribing/error states, calling `start_voice_recording`/`stop_voice_recording`, writing a
      non-interrupt `text` result into the existing chat input field. While recording, the mic
      button gives way to icon-only cancel and send buttons (FR-022 to FR-024).
- [x] T015 [US1] Auto-send wiring: read/write the `voice.auto_send` preference (vault-scoped) via
      the existing `get_pref`/`set_pref` commands; when enabled (default), send the transcribed
      message through the existing send-message path immediately; when disabled, leave it editable
      in the input field (FR-005, FR-006) (depends on T014).
- [x] T016 [US1] Empty-transcript handling: when `text` is empty (no speech detected), leave the
      input field and message history untouched, and show a non-blocking indicator ("keine Sprache
      erkannt") (FR-014) (depends on T014).
- [ ] T017 [US1] Manual verification: run [quickstart.md](quickstart.md) §1 end to end. **Not run**
      — the sandboxed environment this pass ran in has no real microphone, so the actual
      press-mic/speak/see-it-transcribed loop still needs a human on real hardware. Everything
      short of that is verified: `cargo build`/`cargo test`/`cargo clippy --all-targets -D warnings`
      all pass on every feature combination (`--no-default-features`, `--features llm-cpu`,
      `--features voice`, and plain default `llm-cpu`+`voice` together, 213 tests), including
      `stt::local_tests::transcribes_known_fixture` — a real download of `openai/whisper-tiny` plus
      real CPU inference against a checked-in audio fixture, run explicitly with
      `cargo test --features llm-cpu -- --ignored`. Frontend `pnpm typecheck`/`lint`/`format:check`/
      `check:templates`/`generate` all pass too.

**Checkpoint**: User Story 1 is fully functional and independently testable/demoable (MVP).

---

## Phase 4: User Story 2 - Interrupt the assistant by voice (Priority: P1)

**Goal**: Speaking "stop"/"halt"/"abbrechen" alone immediately stops an in-progress assistant
response, regardless of what the assistant is doing; the same words inside a longer dictated
sentence do not trigger it.

**Independent Test**: While the assistant is generating, record one of the three words and verify
the response stops immediately; then dictate a full sentence containing one of the words and
verify it is _not_ interrupted.

### Tests for User Story 2

- [ ] T018 [P] [US2] Test that the local interrupt fast path cancels an in-progress turn via the
      existing `CancellationToken`, independent of the turn/assistant's current state and selected
      STT provider, in `src-tauri/src/chat/turn/step_tests.rs`. Cover the cap event arriving while a
      frontend stop await is pending and assert that the buffer is transcribed and auto-sent at
      most once.

### Implementation for User Story 2

- [ ] T019 [US2] In the backend recording pipeline (T009/T013), run a bounded local interrupt
      fast path during capture. At the local end-of-utterance boundary, an exact match from T007
      MUST trigger the existing `CancellationToken` in `src-tauri/src/chat/session.rs` and emit
      `voice-interrupt-detected` before waiting for the selected provider; the `stop_voice_recording`
      handler owns cancellation and coalesces the result, independent of assistant/LLM state or
      external-provider failure (FR-008) (depends on T013, T018).
- [ ] T020 [US2] In `VoiceInputControl.vue` (T014), when the interrupt event or result is set, do
      **not** write `text` to the input field or send anything — only reflect that an interrupt
      fired. Cancellation remains backend-owned; empty transcripts also leave the input unchanged
      (depends on T014, T019).
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
      `src-tauri/src/stt/external_tests.rs`: canonical PCM request shape, success mapping, `401`
      / `403` → `InvalidCredentials`, transport/timeout → the defined `SttError` mapping, and
      command-level adapter failures → `TranscriptionFailed { reason }` as a surfaced,
      non-panicking error.

### Implementation for User Story 3

- [ ] T023 [US3] Implement `ExternalSttAdapter` in `src-tauri/src/stt/external.rs`: the
      multipart transcription protocol decided in [research.md](research.md) §7, using
      `base_url` + credentials from the provider row (depends on T004, T022).
- [ ] T024 [US3] Add the additive `capability` field to `AddProviderArgs`/`ProviderPayload` in
      `src-tauri/src/providers/mod.rs`, with an explicit Serde default that normalizes omitted
      legacy input to `chat` before persistence. Add a contract test for a payload without
      `capability` and assert the stored value is `chat`; wire `capability: "transcription"` +
      `kind: "api_key"` creation to build an `ExternalSttAdapter` (depends on T002, T023).
- [ ] T025 [US3] Resolve the active `SttAdapter` in the `stop_voice_recording` dispatch (T013) from
      the `voice.active_stt_provider` device preference only after parsing and validating the
      referenced row as `capability = transcription` with supported `kind` (`local` or `api_key`).
      Fall back to the local provider (T005) if the preference is missing, malformed, deleted, a
      chat provider, or otherwise incompatible (depends on T013, T024).
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
- [ ] T031 Run `cargo test` (full workspace), `cargo build --no-default-features`,
      `cargo build --no-default-features --features voice`, and `pnpm typecheck`; confirm all are
      green before opening the PR.
- [ ] T032 [P] [US1] Declare microphone permissions in the Tauri iOS and Android platform
      manifests (`NSMicrophoneUsageDescription` and Android `RECORD_AUDIO`) and verify the
      generated platform projects contain them.
- [ ] T033 [P] [US1] Verify the `cpal` capture backend and required target configuration for both
      iOS and Android, including native sample-format/channel conversion coverage; document any
      platform-specific `cfg` boundary needed by the audio module.
- [ ] T034 [US1] Run successful iOS and Android builds through the repository's Tauri mobile build
      commands after T032/T033. Do not claim either mobile platform as supported until both builds
      pass; if a target is unavailable, narrow the target platform and record the blocker.

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
independently _testable_ per its Independent Test above; US1 must simply land first.

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
