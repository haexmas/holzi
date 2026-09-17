//! Local Whisper transcription (spec 008-voice-control-stt), running
//! `candle-transformers`' Whisper on the same candle-core/candle-nn runtime
//! `mistralrs` already pulls in for chat inference (research.md §1).
//!
//! **Deviation from spec.md FR-010/SC-003 and quickstart.md, decided with
//! the operator 2026-09-17**: the spec calls for the model to ship inside
//! the installer with zero first-run download. This implementation instead
//! downloads the bundled model on demand (mirroring how `models::huggingface`/
//! `models::download` already fetch GGUF chat models), to avoid committing
//! a large binary checkpoint into git history. After that first download,
//! transcription is fully offline (FR-003/SC-004 still hold).
//!
//! **Spec 010 update**: which tier (`tiny`/`base`/`small`) is loaded is now a
//! per-device choice (`voice.stt_model_id` preference, resolved in
//! `voice.rs`) instead of hardcoded to `tiny` for every device — the
//! previous per-device-tiering gap this module used to document is closed by
//! `stt::catalog`'s generalized tier selector. Model files also moved off a
//! Whisper-specific path onto the same generic `models::paths` root chat
//! models already use — see `resolve_or_migrate_model_dir` below for the
//! one remaining reference to the old path, kept only to migrate an
//! already-downloaded install.

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
use crate::models::paths as model_paths;

use super::catalog::SttCatalogEntry;
use super::{CanonicalPcm, SttAdapter, SttError};

/// Legacy path this feature's pre-spec-010 implementation downloaded
/// `whisper-tiny` into, before STT models moved onto the shared
/// `models::paths` root chat models already use (research.md §4). This is
/// the *only* remaining reference to that path — [`resolve_or_migrate_model_dir`]
/// uses it solely to migrate an already-downloaded install instead of
/// silently abandoning it.
const LEGACY_WHISPER_TINY_DIR: &str = "whisper/tiny/169d4a4341b33bc18d8881c4b69c2e104e1cc0af";

const CONFIG_FILENAME: &str = "config.json";
const TOKENIZER_FILENAME: &str = "tokenizer.json";
const WEIGHTS_FILENAME: &str = "model.safetensors";

/// Candidate languages for `detect_language`, restricted to the app's own
/// supported UI locales (`nuxt.config.ts`'s `i18n.locales` — keep in sync if
/// that list ever grows) instead of Whisper's full 99-language set.
///
/// The `tiny` model's language-ID is weak on short, largely-silence-padded
/// push-to-talk clips (verified 2026-09-17: a spoken German command was
/// detected as English) — scoring all 99 `<|xx|>` tokens gives a short,
/// ambiguous utterance 98 ways to lose to some other language by a small
/// margin. Restricting the argmax to only the languages we actually care
/// about can only help: it removes irrelevant distractors, never the
/// correct answer, from the comparison.
const SUPPORTED_LANGUAGES: [&str; 2] = ["de", "en"];

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
    /// Ensures `entry`'s files are present under its `models::paths` slug
    /// directory (downloading any missing/incomplete file, migrating a
    /// legacy install first if applicable) and loads it into memory.
    pub async fn load(app: &AppHandle, entry: &SttCatalogEntry) -> Result<Self, SttError> {
        let dir = resolve_or_migrate_model_dir(app, entry)?;
        ensure_model_files(&dir, entry).await?;
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

    let (_, _, frame_count) = mel.dims3().map_err(tensor_err)?;
    let mut decoded_chunks = Vec::new();
    for start in (0..frame_count).step_by(m::N_FRAMES) {
        let chunk_len = (frame_count - start).min(m::N_FRAMES);
        let mel_chunk = mel.narrow(2, start, chunk_len).map_err(tensor_err)?;

        // Flush the encoder and decoder caches for every independent audio
        // segment. Whisper's positional embedding only supports N_FRAMES,
        // while Capture permits recordings up to 60 seconds.
        let audio_features = inner
            .model
            .encoder
            .forward(&mel_chunk, true)
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
        if !text.trim().is_empty() {
            decoded_chunks.push(text.trim().to_string());
        }
    }

    Ok(decoded_chunks.join(" "))
}

/// Mirrors candle's whisper example `multilingual::detect_language`: a
/// single decoder step over just `[sot_token]`, picking whichever `<|xx|>`
/// language token scores highest — restricted to [`SUPPORTED_LANGUAGES`]
/// rather than Whisper's full language set.
fn detect_language(
    inner: &mut Inner,
    audio_features: &Tensor,
    sot_token: u32,
) -> Result<u32, SttError> {
    let device = &inner.device;
    let language_token_ids = SUPPORTED_LANGUAGES
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

/// A file counts as a complete download only if it exists, is a regular
/// file, and has nonzero size — guards against a previous run's
/// interrupted/truncated download (or a hand-deleted/corrupted file) being
/// mistaken for an installed one. No content validation beyond that.
pub fn is_complete_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// A model directory is complete only when all three files Whisper needs
/// are each individually complete (see [`is_complete_file`]). Shared with
/// `stt::commands::list_installed_stt_models`, which uses the same
/// predicate so listing, loading, and downloading always agree on the same
/// installed/not-installed state.
pub fn is_complete_model(dir: &Path) -> bool {
    [CONFIG_FILENAME, TOKENIZER_FILENAME, WEIGHTS_FILENAME]
        .iter()
        .all(|filename| is_complete_file(&dir.join(filename)))
}

/// Resolves `entry`'s canonical `models::paths` slug directory, creating it
/// if missing. If that directory isn't already complete and `entry` is
/// `whisper-tiny`, first checks the pre-spec-010
/// [`LEGACY_WHISPER_TINY_DIR`] for a complete install and copies its files
/// into the canonical directory — best-effort: on any copy failure the
/// canonical directory is left as-is and `ensure_model_files` downloads
/// whatever didn't make it across. An incomplete legacy install is never
/// treated as valid and is left untouched.
///
/// Called before every disk-touching operation (load, install-status
/// listing, download) so all three agree on the same installed state.
pub fn resolve_or_migrate_model_dir(
    app: &AppHandle,
    entry: &SttCatalogEntry,
) -> Result<PathBuf, SttError> {
    let dir = model_paths::slug_dir(app, &entry.id).map_err(|e| SttError::LocalUnavailable {
        reason: format!("resolve model dir for {}: {e}", entry.id),
    })?;

    if entry.id == "whisper-tiny" && !is_complete_model(&dir) {
        if let Ok(legacy_dir) = app
            .path()
            .resolve(LEGACY_WHISPER_TINY_DIR, BaseDirectory::AppLocalData)
        {
            migrate_legacy_if_present(&legacy_dir, &dir);
        }
    }

    Ok(dir)
}

/// Copies a complete legacy install's files into `canonical_dir`,
/// best-effort (a failed individual copy just leaves that file for
/// `ensure_model_files` to download fresh). No-op if `legacy_dir` isn't
/// itself a complete install — an incomplete legacy install is never
/// treated as valid. Split out from [`resolve_or_migrate_model_dir`] as a
/// pure `&Path`-based helper so it's unit-testable without a Tauri
/// `AppHandle` — `pub` for the same reason as [`ensure_model_files`].
pub fn migrate_legacy_if_present(legacy_dir: &Path, canonical_dir: &Path) {
    if !is_complete_model(legacy_dir) {
        return;
    }
    for filename in [CONFIG_FILENAME, TOKENIZER_FILENAME, WEIGHTS_FILENAME] {
        let _ = std::fs::copy(legacy_dir.join(filename), canonical_dir.join(filename));
    }
}

/// Downloads whichever of `config.json`/`tokenizer.json`/`model.safetensors`
/// isn't already complete under `dir` (see [`is_complete_file`]) from
/// `entry`'s pinned HuggingFace revision. Idempotent — a second call with
/// all three files already complete makes no network request at all (FR-003
/// continues to hold once the first download completes). `pub` (rather than
/// private) so `local_tests.rs` can prime a fixture cache directly, without
/// needing an `AppHandle` the way [`LocalWhisperAdapter::load`] does.
pub async fn ensure_model_files(dir: &Path, entry: &SttCatalogEntry) -> Result<(), SttError> {
    for filename in [CONFIG_FILENAME, TOKENIZER_FILENAME, WEIGHTS_FILENAME] {
        let dest = dir.join(filename);
        if is_complete_file(&dest) {
            continue;
        }
        let url = format!(
            "https://huggingface.co/{}/resolve/{}/{filename}",
            entry.hf_repo, entry.hf_revision
        );
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
