//! Microphone capture (spec 008-voice-control-stt, T006): `cpal`-based
//! start/stop against an in-memory canonical PCM buffer.
//!
//! Capture accumulates raw, native-format interleaved samples (converted to
//! `f32` at the callback boundary only — no resampling/downmixing there)
//! and converts the whole buffer to the canonical contract
//! (`stt::{CanonicalPcm, SAMPLE_RATE_HZ}`, 16 kHz mono) once, in
//! [`Capture::stop`]. A push-to-talk recording is at most
//! [`MAX_RECORDING_SECS`] long, so holding the native-rate buffer
//! momentarily (a few tens of MB, worst case) is simpler and far easier to
//! get right than a stateful streaming resampler that must join
//! consecutive audio callbacks without a discontinuity — and this way the
//! conversion math (`downmix_to_mono`, `resample_linear`) stays two pure,
//! independently testable functions instead of being entangled with the
//! realtime callback.
//!
//! `cpal::Stream` is safe to hold here rather than on a dedicated thread:
//! every backend cpal ships (ALSA, CoreAudio, WASAPI, AAudio, ...) asserts
//! its `Stream` type is `Send + Sync` via cpal's own
//! `assert_stream_send!`/`assert_stream_sync!` macros.

#[cfg(test)]
mod audio_tests;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};

use crate::stt::{CanonicalPcm, SAMPLE_RATE_HZ};

/// FR-017: hitting this stops capture (not the whole recording session)
/// and transcribes whatever was captured up to the cap.
pub const MAX_RECORDING_SECS: u64 = 60;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no microphone input device available")]
    DeviceUnavailable,
    #[error("failed to query input device configuration: {reason}")]
    ConfigUnavailable { reason: String },
    #[error("failed to start capture: {reason}")]
    StartFailed { reason: String },
}

/// One running capture. Owns the live `cpal::Stream` — dropping it (via
/// [`Capture::stop`] or [`Capture::cancel`]) halts the device callback.
pub struct Capture {
    stream: cpal::Stream,
    raw_buffer: Arc<StdMutex<Vec<f32>>>,
    channels: u16,
    input_rate: u32,
    capped: Arc<AtomicBool>,
}

impl Capture {
    /// Starts microphone capture immediately on the default input device.
    /// Returns once the stream is confirmed playing (or failed); capture
    /// then continues on cpal's own realtime audio thread.
    pub fn start() -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioError::DeviceUnavailable)?;
        let supported_config =
            device
                .default_input_config()
                .map_err(|e| AudioError::ConfigUnavailable {
                    reason: e.to_string(),
                })?;

        let channels = supported_config.channels();
        let input_rate = supported_config.sample_rate();
        let sample_format = supported_config.sample_format();
        let stream_config: cpal::StreamConfig = supported_config.into();

        let raw_buffer = Arc::new(StdMutex::new(Vec::new()));
        let capped = Arc::new(AtomicBool::new(false));
        let max_input_samples =
            (MAX_RECORDING_SECS * input_rate as u64) as usize * channels as usize;

        let stream = match sample_format {
            cpal::SampleFormat::I8 => build_capture_stream::<i8>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::F32 => build_capture_stream::<f32>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::I16 => build_capture_stream::<i16>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::I24 => build_capture_stream::<cpal::I24>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::I32 => build_capture_stream::<i32>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::I64 => build_capture_stream::<i64>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::U8 => build_capture_stream::<u8>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::U16 => build_capture_stream::<u16>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::U24 => build_capture_stream::<cpal::U24>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::U32 => build_capture_stream::<u32>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::U64 => build_capture_stream::<u64>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            cpal::SampleFormat::F64 => build_capture_stream::<f64>(
                &device,
                &stream_config,
                Arc::clone(&raw_buffer),
                Arc::clone(&capped),
                max_input_samples,
            ),
            other => {
                return Err(AudioError::StartFailed {
                    reason: format!("unsupported input sample format: {other}"),
                })
            }
        }
        .map_err(|e| AudioError::StartFailed {
            reason: e.to_string(),
        })?;

        stream.play().map_err(|e| AudioError::StartFailed {
            reason: e.to_string(),
        })?;

        Ok(Self {
            stream,
            raw_buffer,
            channels,
            input_rate,
            capped,
        })
    }

    /// True once the max-duration cap (FR-017) has stopped new samples
    /// from being appended. The stream is still technically running (its
    /// callback just discards further data) until [`Capture::stop`] drops
    /// it — callers poll this to know when to auto-trigger a stop.
    pub fn has_capped(&self) -> bool {
        self.capped.load(Ordering::Relaxed)
    }

    /// Stops capture (dropping the stream halts the device callback) and
    /// converts whatever was captured to the canonical 16 kHz mono
    /// contract.
    pub fn stop(self) -> CanonicalPcm {
        drop(self.stream);
        let interleaved = std::mem::take(&mut *self.raw_buffer.lock().unwrap_or_else(|e| {
            // A panic inside the audio callback would poison this — not
            // expected (the callback only does arithmetic and a Vec push),
            // but recovering the last-good buffer is strictly better than
            // propagating a poisoned-lock panic through `stop_voice_recording`.
            e.into_inner()
        }));
        let mono = downmix_to_mono(&interleaved, self.channels);
        CanonicalPcm {
            samples: resample_linear(&mono, self.input_rate, SAMPLE_RATE_HZ),
        }
    }

    /// Stops capture and discards whatever was captured (cancel path,
    /// FR-016).
    pub fn cancel(self) {
        drop(self.stream);
    }
}

fn build_capture_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    raw_buffer: Arc<StdMutex<Vec<f32>>>,
    capped: Arc<AtomicBool>,
    max_input_samples: usize,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        *config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            if capped.load(Ordering::Relaxed) {
                return;
            }
            let Ok(mut buf) = raw_buffer.lock() else {
                return;
            };
            buf.extend(data.iter().map(|&s| f32::from_sample(s)));
            if buf.len() >= max_input_samples {
                buf.truncate(max_input_samples);
                capped.store(true, Ordering::Relaxed);
            }
        },
        |err| log::warn!("voice capture stream error: {err}"),
        None,
    )
}

/// Averages `channels` interleaved channels down to mono. A no-op copy
/// when the device is already mono.
fn downmix_to_mono(interleaved: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    let channels = channels as usize;
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Linear-interpolation resampling from `from_rate` to `to_rate`. Adequate
/// for push-to-talk speech capture feeding a Whisper model — not intended
/// as a high-fidelity audio resampler.
fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() || from_rate == to_rate {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = ((input.len() as f64) / ratio).round() as usize;
    let last = input.len() - 1;
    (0..out_len)
        .map(|i| {
            let src_pos = i as f64 * ratio;
            let idx = (src_pos.floor() as usize).min(last);
            let frac = (src_pos - idx as f64) as f32;
            let a = input[idx];
            let b = input[(idx + 1).min(last)];
            a + (b - a) * frac
        })
        .collect()
}
