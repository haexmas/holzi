//! Local Whisper transcription (spec 008-voice-control-stt), running
//! `candle-transformers`' Whisper on the same candle-core/candle-nn runtime
//! `mistralrs` already pulls in for chat inference (research.md §1).
//!
//! **Deviation from spec.md FR-010/SC-003 and quickstart.md, decided with
//! the operator 2026-09-17**: the spec calls for the model to ship inside
//! the installer with zero first-run download. This implementation instead
//! downloads the bundled model into `<AppLocalData>/whisper/tiny/` the
//! first time it is needed (mirroring how `models::huggingface`/
//! `models::download` already fetch GGUF chat models), to avoid committing
//! a large binary checkpoint into git history. After that first download,
//! transcription is fully offline (FR-003/SC-004 still hold).
//!
//! Only the `tiny` (multilingual) tier is used for every device — FR-011's
//! per-device tiering assumed a reusable hardware-tier selector from spec
//! 002 that, on inspection, does not exist as a standalone function
//! (`catalog::recommend_tiers` is hardcoded to the LLM catalog's
//! `CatalogEntry` type). Building a general-purpose tier selector is out of
//! scope for this pass; every device gets the smallest multilingual model.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use candle::{Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, audio, Config};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};
use tokenizers::Tokenizer;

use crate::models::download::download_to_file;

use super::{CanonicalPcm, SttAdapter, SttError};

const WHISPER_REPO: &str = "openai/whisper-tiny";
// Pinned to the reviewed model revision so config, tokenizer, and weights
// can never be mixed across mutable `main` updates.
const WHISPER_REVISION: &str = "169d4a4341b33bc18d8881c4b69c2e104e1cc0af";
const WHISPER_DIR: &str = "whisper/tiny/169d4a4341b33bc18d8881c4b69c2e104e1cc0af";

const CONFIG_FILENAME: &str = "config.json";
const TOKENIZER_FILENAME: &str = "tokenizer.json";
const WEIGHTS_FILENAME: &str = "model.safetensors";

/// Whisper's 99 supported language codes (`multilingual.rs` in candle's own
/// whisper example), used only to build the `<|xx|>` token-id list for
/// language detection — no display names needed here.
const LANGUAGES: [&str; 99] = [
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it",
    "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur",
    "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn",
    "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si",
    "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo",
    "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
    "ha", "ba", "jw", "su",
];

/// The bundled local STT adapter, resolved via
/// `providers::local::ensure_local_transcription_provider`'s `"whisper-local"`
/// discriminator. Expensive to construct (model load, ~seconds) — callers
/// load it once and keep it warm (see `VoiceState` in `lib.rs`) rather than
/// reconstructing it per transcription.
pub struct LocalWhisperAdapter {
    inner: Arc<StdMutex<Inner>>,
}

struct Inner {
    model: m::model::Whisper,
    tokenizer: Tokenizer,
    mel_filters: Vec<f32>,
    device: Device,
}

impl LocalWhisperAdapter {
    /// Ensures the bundled model is present under `<AppLocalData>/whisper/tiny/`
    /// (downloading any missing file once) and loads it into memory.
    pub async fn load(app: &AppHandle) -> Result<Self, SttError> {
        let dir = model_dir(app)?;
        ensure_model_files(&dir).await?;
        tauri::async_runtime::spawn_blocking(move || Self::load_from_dir(&dir))
            .await
            .map_err(|e| SttError::LocalUnavailable {
                reason: format!("whisper load task: {e}"),
            })?
    }

    /// Loads an already-downloaded model from `dir`. Split out from
    /// [`Self::load`] so a test can point it at a pre-populated fixture
    /// directory without a network call.
    pub fn load_from_dir(dir: &Path) -> Result<Self, SttError> {
        let config: Config =
            serde_json::from_str(&std::fs::read_to_string(dir.join(CONFIG_FILENAME)).map_err(
                |e| SttError::LocalUnavailable {
                    reason: format!("read {CONFIG_FILENAME}: {e}"),
                },
            )?)
            .map_err(|e| SttError::LocalUnavailable {
                reason: format!("parse {CONFIG_FILENAME}: {e}"),
            })?;

        let tokenizer = Tokenizer::from_file(dir.join(TOKENIZER_FILENAME)).map_err(|e| {
            SttError::LocalUnavailable {
                reason: format!("load {TOKENIZER_FILENAME}: {e}"),
            }
        })?;

        let device = Device::Cpu;
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[dir.join(WEIGHTS_FILENAME)], m::DTYPE, &device)
        }
        .map_err(|e| SttError::LocalUnavailable {
            reason: format!("load {WEIGHTS_FILENAME}: {e}"),
        })?;
        let mel_filters = mel_filterbank(m::SAMPLE_RATE as u32, m::N_FFT, config.num_mel_bins);
        let model =
            m::model::Whisper::load(&vb, config).map_err(|e| SttError::LocalUnavailable {
                reason: format!("build whisper model: {e}"),
            })?;

        Ok(Self {
            inner: Arc::new(StdMutex::new(Inner {
                model,
                tokenizer,
                mel_filters,
                device,
            })),
        })
    }
}

#[async_trait]
impl SttAdapter for LocalWhisperAdapter {
    async fn transcribe(&self, audio_in: &CanonicalPcm) -> Result<String, SttError> {
        let inner = Arc::clone(&self.inner);
        // Candle inference is synchronous/CPU-bound; `samples` is cloned so
        // the closure can be `'static` for `spawn_blocking` (matches this
        // codebase's established blocking-work convention, e.g.
        // `storage::preferences_commands`).
        let samples = audio_in.samples.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut inner = inner.lock().map_err(|e| SttError::Failed {
                reason: format!("whisper state poisoned: {e}"),
            })?;
            transcribe_samples(&mut inner, &samples)
        })
        .await
        .map_err(|e| SttError::Failed {
            reason: format!("whisper inference task: {e}"),
        })?
    }
}

fn transcribe_samples(inner: &mut Inner, samples: &[f32]) -> Result<String, SttError> {
    let num_mel_bins = inner.model.config.num_mel_bins;
    let mel = audio::pcm_to_mel(&inner.model.config, samples, &inner.mel_filters);
    let mel_len = mel.len();
    let mel = Tensor::from_vec(
        mel,
        (1, num_mel_bins, mel_len / num_mel_bins),
        &inner.device,
    )
    .map_err(tensor_err)?;

    let sot_token = token_id(&inner.tokenizer, m::SOT_TOKEN)?;
    let transcribe_token = token_id(&inner.tokenizer, m::TRANSCRIBE_TOKEN)?;
    let no_timestamps_token = token_id(&inner.tokenizer, m::NO_TIMESTAMPS_TOKEN)?;
    let eot_token = token_id(&inner.tokenizer, m::EOT_TOKEN)?;

    // One encoder pass feeds both language detection and the decode loop
    // below (candle's own whisper example runs the encoder twice — once
    // per use — since it calls them as two independently invoked steps;
    // reusing the tensor here is equivalent and avoids the redundant pass).
    let audio_features = inner
        .model
        .encoder
        .forward(&mel, true)
        .map_err(tensor_err)?;
    let language_token = detect_language(inner, &audio_features, sot_token)?;

    let mut tokens = vec![
        sot_token,
        language_token,
        transcribe_token,
        no_timestamps_token,
    ];
    let sample_len = inner.model.config.max_target_positions / 2;

    for i in 0..sample_len {
        let tokens_t = Tensor::new(tokens.as_slice(), &inner.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(tensor_err)?;
        let ys = inner
            .model
            .decoder
            .forward(&tokens_t, &audio_features, i == 0)
            .map_err(tensor_err)?;
        let (_, seq_len, _) = ys.dims3().map_err(tensor_err)?;
        let logits = inner
            .model
            .decoder
            .final_linear(&ys.i((..1, seq_len - 1..)).map_err(tensor_err)?)
            .map_err(tensor_err)?
            .i(0)
            .map_err(tensor_err)?
            .i(0)
            .map_err(tensor_err)?;
        let logits_v: Vec<f32> = logits.to_vec1().map_err(tensor_err)?;
        let next_token = argmax(&logits_v);
        tokens.push(next_token);
        if next_token == eot_token || tokens.len() > inner.model.config.max_target_positions {
            break;
        }
    }

    let text = inner
        .tokenizer
        .decode(&tokens, true)
        .map_err(|e| SttError::Failed {
            reason: format!("tokenizer decode: {e}"),
        })?;
    Ok(text.trim().to_string())
}

/// Mirrors candle's whisper example `multilingual::detect_language`: a
/// single decoder step over just `[sot_token]`, picking whichever `<|xx|>`
/// language token scores highest.
fn detect_language(
    inner: &mut Inner,
    audio_features: &Tensor,
    sot_token: u32,
) -> Result<u32, SttError> {
    let device = &inner.device;
    let language_token_ids = LANGUAGES
        .iter()
        .map(|code| token_id(&inner.tokenizer, &format!("<|{code}|>")))
        .collect::<Result<Vec<_>, _>>()?;
    let tokens = Tensor::new(&[[sot_token]], device).map_err(tensor_err)?;
    let ys = inner
        .model
        .decoder
        .forward(&tokens, audio_features, true)
        .map_err(tensor_err)?;
    let logits = inner
        .model
        .decoder
        .final_linear(&ys.i(..1).map_err(tensor_err)?)
        .map_err(tensor_err)?
        .i(0)
        .map_err(tensor_err)?
        .i(0)
        .map_err(tensor_err)?;
    let ids_tensor = Tensor::new(language_token_ids.as_slice(), device).map_err(tensor_err)?;
    let logits = logits.index_select(&ids_tensor, 0).map_err(tensor_err)?;
    let logits_v: Vec<f32> = logits.to_vec1().map_err(tensor_err)?;
    Ok(language_token_ids[argmax(&logits_v) as usize])
}

fn argmax(values: &[f32]) -> u32 {
    values
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(i, _)| i as u32)
        .unwrap_or(0)
}

fn token_id(tokenizer: &Tokenizer, token: &str) -> Result<u32, SttError> {
    tokenizer
        .token_to_id(token)
        .ok_or_else(|| SttError::Failed {
            reason: format!("no token id for {token}"),
        })
}

fn tensor_err(e: candle::Error) -> SttError {
    SttError::Failed {
        reason: format!("whisper tensor op: {e}"),
    }
}

/// Resolves `<AppLocalData>/whisper/tiny/`, creating it if missing.
fn model_dir(app: &AppHandle) -> Result<PathBuf, SttError> {
    let dir = app
        .path()
        .resolve(WHISPER_DIR, BaseDirectory::AppLocalData)
        .map_err(|e| SttError::LocalUnavailable {
            reason: format!("resolve whisper model dir: {e}"),
        })?;
    std::fs::create_dir_all(&dir).map_err(|e| SttError::LocalUnavailable {
        reason: format!("create whisper model dir: {e}"),
    })?;
    Ok(dir)
}

/// Downloads whichever of `config.json`/`tokenizer.json`/`model.safetensors`
/// is not already present under `dir`. Idempotent — a second call with all
/// three files already on disk makes no network request at all (FR-003
/// continues to hold once the first download completes). `pub` (rather than
/// private) so `local_tests.rs` can prime a fixture cache directly, without
/// needing an `AppHandle` the way [`LocalWhisperAdapter::load`] does.
pub async fn ensure_model_files(dir: &Path) -> Result<(), SttError> {
    for filename in [CONFIG_FILENAME, TOKENIZER_FILENAME, WEIGHTS_FILENAME] {
        let dest = dir.join(filename);
        if dest.exists() {
            continue;
        }
        let url =
            format!("https://huggingface.co/{WHISPER_REPO}/resolve/{WHISPER_REVISION}/{filename}");
        download_to_file(&url, dest, |_progress| {})
            .await
            .map_err(|e| SttError::LocalUnavailable {
                reason: format!("download {filename}: {e}"),
            })?;
    }
    Ok(())
}

/// Computes the Slaney-normalized mel filterbank matrix Whisper expects
/// (`librosa.filters.mel(sr, n_fft, n_mels)` with its default `norm =
/// "slaney"`, `htk = False`) — `candle-transformers`' `audio::pcm_to_mel`
/// takes this as a caller-supplied argument rather than computing it
/// itself. candle's own whisper example ships this as a precomputed binary
/// asset (`melfilters.bytes`) fetched from its own repo; this computes it
/// on the fly instead, to avoid vendoring a second binary blob and a second
/// external download host. Verified bit-for-bit (up to sign-of-zero and one
/// ULP of summation-order noise) against `librosa.filters.mel` for the
/// tiny/base/small (80-bin) shape used here.
///
/// Row-major, shape `(n_mels, n_fft / 2 + 1)` — the layout
/// `audio::pcm_to_mel` expects.
fn mel_filterbank(sample_rate: u32, n_fft: usize, n_mels: usize) -> Vec<f32> {
    let n_freqs = n_fft / 2 + 1;
    let fmax = sample_rate as f64 / 2.0;

    let fft_freqs: Vec<f64> = (0..n_freqs)
        .map(|i| i as f64 * fmax / (n_freqs - 1) as f64)
        .collect();

    let mel_min = hz_to_mel(0.0);
    let mel_max = hz_to_mel(fmax);
    let hz_points: Vec<f64> = (0..n_mels + 2)
        .map(|i| mel_to_hz(mel_min + (mel_max - mel_min) * i as f64 / (n_mels + 1) as f64))
        .collect();
    let fdiff: Vec<f64> = hz_points.windows(2).map(|w| w[1] - w[0]).collect();

    let mut weights = vec![0f32; n_mels * n_freqs];
    for i in 0..n_mels {
        for (j, &freq) in fft_freqs.iter().enumerate() {
            let lower = (freq - hz_points[i]) / fdiff[i];
            let upper = (hz_points[i + 2] - freq) / fdiff[i + 1];
            weights[i * n_freqs + j] = lower.min(upper).max(0.0) as f32;
        }
        // Slaney-style area normalization so each filter integrates to
        // (roughly) the same energy regardless of its width.
        let enorm = (2.0 / (hz_points[i + 2] - hz_points[i])) as f32;
        for w in &mut weights[i * n_freqs..(i + 1) * n_freqs] {
            *w *= enorm;
        }
    }
    weights
}

/// Slaney mel scale (`librosa.core.convert.hz_to_mel`, `htk=False`):
/// linear below 1 kHz, logarithmic above.
fn hz_to_mel(hz: f64) -> f64 {
    const F_SP: f64 = 200.0 / 3.0;
    const MIN_LOG_HZ: f64 = 1000.0;
    let min_log_mel = MIN_LOG_HZ / F_SP;
    let logstep = 6.4f64.ln() / 27.0;
    if hz >= MIN_LOG_HZ {
        min_log_mel + (hz / MIN_LOG_HZ).ln() / logstep
    } else {
        hz / F_SP
    }
}

/// Inverse of [`hz_to_mel`] (`librosa.core.convert.mel_to_hz`, `htk=False`).
fn mel_to_hz(mel: f64) -> f64 {
    const F_SP: f64 = 200.0 / 3.0;
    const MIN_LOG_HZ: f64 = 1000.0;
    let min_log_mel = MIN_LOG_HZ / F_SP;
    let logstep = 6.4f64.ln() / 27.0;
    if mel >= min_log_mel {
        MIN_LOG_HZ * (logstep * (mel - min_log_mel)).exp()
    } else {
        F_SP * mel
    }
}

#[cfg(test)]
mod tests {
    use super::mel_filterbank;

    /// Cross-checked against `librosa.filters.mel(sr=16000, n_fft=400,
    /// n_mels=80, norm="slaney", htk=False)` — the values Whisper's
    /// preprocessing was trained against.
    #[test]
    fn mel_filterbank_matches_known_shape_and_support() {
        let n_mels = 80;
        let n_freqs = 400 / 2 + 1;
        let filters = mel_filterbank(16_000, 400, n_mels);
        assert_eq!(filters.len(), n_mels * n_freqs);
        assert!(filters.iter().all(|&w| w.is_finite() && w >= 0.0));

        // First filter sits right at the bottom of the spectrum.
        let row0 = &filters[0..n_freqs];
        let first_nonzero = row0.iter().position(|&w| w > 0.0).unwrap();
        assert!(first_nonzero <= 1);

        // Last filter sits right at the top, near the Nyquist bin.
        let row_last = &filters[(n_mels - 1) * n_freqs..n_mels * n_freqs];
        let last_nonzero = row_last.iter().rposition(|&w| w > 0.0).unwrap();
        assert!(last_nonzero >= n_freqs - 20);
    }
}
