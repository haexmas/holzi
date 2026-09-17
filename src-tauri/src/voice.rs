//! Voice control Tauri commands (spec 008-voice-control-stt, T009/T013):
//! start/stop/cancel a recording and dispatch the completed buffer to the
//! bundled local Whisper adapter. Registered unconditionally in
//! `lib.rs::run()`'s `generate_handler!` list; a `--no-default-features`
//! (no `voice`) build compiles the stub variants below instead of failing
//! to register the command names at all — matching the existing
//! `#[cfg(feature = "llm-cpu")]` pattern in `chat::model_loading::load_model_inner`.
//!
//! US3 (choosing an external transcription provider) is not implemented
//! yet: every recording dispatches to the bundled local adapter
//! unconditionally. US2 (the "stop"/"halt"/"abbrechen" fast path
//! cancelling an in-progress assistant turn) is also not implemented yet —
//! `match_interrupt` still runs on the final transcript so the wire shape
//! matches the contract, but nothing consumes `interrupt` to actually
//! cancel anything, and the frontend does not yet suppress `text` for it
//! (T019/T020).

#[cfg(feature = "voice")]
mod imp {
    use std::sync::Arc;

    use serde::Serialize;
    use tauri::{AppHandle, Emitter, Manager, State};
    use tokio::sync::{watch, Mutex as AsyncMutex};

    use crate::error::{HolziError, Result};
    use crate::stt::interrupt::{match_interrupt, InterruptCommand};

    /// How often the cap-watcher polls `Capture::has_capped()` after
    /// `start_voice_recording`. Coarse on purpose — FR-017 only needs the
    /// cap to fire "soon after" 60s, not to the millisecond.
    const CAP_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TranscriptionResultWire {
        pub text: String,
        pub interrupt: Option<InterruptCommand>,
    }

    type StopOutcome = std::result::Result<TranscriptionResultWire, String>;

    enum VoiceSlot {
        Idle,
        Recording(crate::audio::Capture),
        Transcribing(watch::Receiver<Option<StopOutcome>>),
    }

    /// Per-process voice pipeline state — at most one active recording or
    /// pending transcription at a time (contracts/tauri-commands.md
    /// `start_voice_recording`).
    pub struct VoiceState {
        slot: AsyncMutex<VoiceSlot>,
        /// The bundled Whisper model, loaded once on first use and kept
        /// warm — reloading it (a multi-second operation) per transcription
        /// would blow SC-001's 5-second budget.
        #[cfg(feature = "llm-cpu")]
        whisper: AsyncMutex<Option<Arc<crate::stt::local::LocalWhisperAdapter>>>,
    }

    impl VoiceState {
        pub fn new() -> Self {
            Self {
                slot: AsyncMutex::new(VoiceSlot::Idle),
                #[cfg(feature = "llm-cpu")]
                whisper: AsyncMutex::new(None),
            }
        }
    }

    impl Default for VoiceState {
        fn default() -> Self {
            Self::new()
        }
    }

    #[tauri::command]
    pub async fn start_voice_recording(app: AppHandle, voice: State<'_, VoiceState>) -> Result<()> {
        {
            let mut slot = voice.slot.lock().await;
            if !matches!(&*slot, VoiceSlot::Idle) {
                return Err(HolziError::AlreadyRecording);
            }
            let capture = crate::audio::Capture::start().map_err(|e| match e {
                crate::audio::AudioError::DeviceUnavailable => HolziError::DeviceUnavailable,
                // cpal does not surface a distinct OS-permission-denied error
                // uniformly across desktop backends; a dedicated
                // `PermissionDenied` UX needs real platform permission
                // integration (tracked with the T032/T033 mobile gate).
                crate::audio::AudioError::ConfigUnavailable { .. }
                | crate::audio::AudioError::StartFailed { .. } => HolziError::DeviceUnavailable,
            })?;
            *slot = VoiceSlot::Recording(capture);
        }
        spawn_cap_watcher(app);
        Ok(())
    }

    #[tauri::command]
    pub async fn stop_voice_recording(
        app: AppHandle,
        voice: State<'_, VoiceState>,
    ) -> Result<TranscriptionResultWire> {
        do_stop(&app, &voice).await
    }

    #[tauri::command]
    pub async fn cancel_voice_recording(voice: State<'_, VoiceState>) -> Result<()> {
        let mut slot = voice.slot.lock().await;
        if let VoiceSlot::Recording(_) = &*slot {
            let VoiceSlot::Recording(capture) = std::mem::replace(&mut *slot, VoiceSlot::Idle)
            else {
                unreachable!()
            };
            capture.cancel();
        }
        // Idempotent: cancelling while idle or already transcribing is not
        // an error (contracts/tauri-commands.md).
        Ok(())
    }

    /// Shared by the manual `stop_voice_recording` command and the
    /// max-duration cap watcher. The first caller to find `Recording` owns
    /// the transition to `Transcribing` and spawns the actual work;
    /// anyone else (including a concurrent caller here) observes
    /// `Transcribing` and awaits the same coalesced result instead of
    /// starting a second transcription or returning `NotRecording`
    /// (data-model.md "Backend-seitig wird der Puffer... atomar...").
    async fn do_stop(app: &AppHandle, voice: &VoiceState) -> Result<TranscriptionResultWire> {
        let mut rx = {
            let mut slot = voice.slot.lock().await;
            match std::mem::replace(&mut *slot, VoiceSlot::Idle) {
                VoiceSlot::Idle => return Err(HolziError::NotRecording),
                VoiceSlot::Recording(capture) => {
                    let pcm = capture.stop();
                    let (tx, rx) = watch::channel(None);
                    *slot = VoiceSlot::Transcribing(rx.clone());
                    let app_task = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let voice_task = app_task.state::<VoiceState>();
                        let result = run_transcription(&app_task, &voice_task, pcm).await;
                        let _ = tx.send(Some(result));
                        *voice_task.slot.lock().await = VoiceSlot::Idle;
                    });
                    rx
                }
                VoiceSlot::Transcribing(rx) => {
                    *slot = VoiceSlot::Transcribing(rx.clone());
                    rx
                }
            }
        };
        loop {
            if let Some(result) = rx.borrow().clone() {
                return result.map_err(|reason| HolziError::TranscriptionFailed { reason });
            }
            if rx.changed().await.is_err() {
                return Err(HolziError::TranscriptionFailed {
                    reason: "voice pipeline closed unexpectedly".into(),
                });
            }
        }
    }

    /// Polls the max-duration cap (FR-017), runs the same coalesced stop path
    /// a manual `stop_voice_recording` call would, and emits the completed
    /// result as the event payload. Emitting the result after `do_stop`
    /// completes lets the renderer consume it without issuing a second stop
    /// request after the slot has returned to `Idle`.
    fn spawn_cap_watcher(app: AppHandle) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(CAP_POLL_INTERVAL).await;
                let voice = app.state::<VoiceState>();
                let capped = {
                    let slot = voice.slot.lock().await;
                    match &*slot {
                        VoiceSlot::Recording(capture) => {
                            if capture.has_capped() {
                                true
                            } else {
                                continue;
                            }
                        }
                        _ => return,
                    }
                };
                if capped {
                    let outcome = do_stop(&app, &voice).await;
                    let payload = capped_event_payload(outcome);
                    let _ = app.emit("voice-recording-capped", payload);
                    return;
                }
            }
        });
    }

    /// Converts the completed cap-watcher outcome into the optional event
    /// payload consumed by the renderer. Failed transcriptions use `None` so
    /// the renderer can enter its generic error state without retrying stop.
    pub(crate) fn capped_event_payload(
        outcome: crate::error::Result<TranscriptionResultWire>,
    ) -> Option<TranscriptionResultWire> {
        outcome.ok()
    }

    /// Resolves the active transcription adapter and runs it. US3 (external
    /// provider selection) is not implemented — always the bundled local
    /// adapter for now.
    async fn run_transcription(
        app: &AppHandle,
        voice: &VoiceState,
        pcm: crate::stt::CanonicalPcm,
    ) -> StopOutcome {
        let adapter = resolve_local_adapter(app, voice)
            .await
            .map_err(|e| e.to_string())?;
        let text = adapter.transcribe(&pcm).await.map_err(|e| e.to_string())?;
        let interrupt = match_interrupt(&text);
        Ok(TranscriptionResultWire { text, interrupt })
    }

    /// Loads the bundled Whisper model on first use and keeps it warm
    /// (`voice.whisper`) — reloading it per transcription would blow
    /// SC-001's 5-second budget. Returns a trait object so the
    /// `llm-cpu`-off branch (`voice` enabled without `llm-cpu`) can report
    /// `LocalSttUnavailable` without a second concrete adapter type.
    async fn resolve_local_adapter(
        #[cfg_attr(not(feature = "llm-cpu"), allow(unused_variables))] app: &AppHandle,
        #[cfg_attr(not(feature = "llm-cpu"), allow(unused_variables))] voice: &VoiceState,
    ) -> std::result::Result<Arc<dyn crate::stt::SttAdapter>, crate::stt::SttError> {
        #[cfg(feature = "llm-cpu")]
        {
            let mut guard = voice.whisper.lock().await;
            if let Some(adapter) = guard.as_ref() {
                return Ok(Arc::clone(adapter) as Arc<dyn crate::stt::SttAdapter>);
            }
            let adapter = Arc::new(crate::stt::local::LocalWhisperAdapter::load(app).await?);
            *guard = Some(Arc::clone(&adapter));
            Ok(adapter as Arc<dyn crate::stt::SttAdapter>)
        }
        #[cfg(not(feature = "llm-cpu"))]
        {
            Err(crate::stt::SttError::LocalUnavailable {
                reason: "local transcription is not enabled in this build".into(),
            })
        }
    }
}

#[cfg(all(test, feature = "voice"))]
#[path = "voice_tests.rs"]
mod voice_tests;

#[cfg(feature = "voice")]
pub use imp::{
    cancel_voice_recording, start_voice_recording, stop_voice_recording, TranscriptionResultWire,
    VoiceState,
};

#[cfg(not(feature = "voice"))]
mod stub {
    use crate::error::{HolziError, Result};

    fn voice_disabled() -> HolziError {
        HolziError::InvalidInput {
            reason: "voice control is not enabled in this build".into(),
        }
    }

    #[tauri::command]
    pub async fn start_voice_recording() -> Result<()> {
        Err(voice_disabled())
    }

    #[tauri::command]
    pub async fn stop_voice_recording() -> Result<super::TranscriptionResultWire> {
        Err(voice_disabled())
    }

    #[tauri::command]
    pub async fn cancel_voice_recording() -> Result<()> {
        Err(voice_disabled())
    }
}

#[cfg(not(feature = "voice"))]
pub use stub::{cancel_voice_recording, start_voice_recording, stop_voice_recording};

#[cfg(not(feature = "voice"))]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResultWire {
    pub text: String,
    pub interrupt: Option<()>,
}
