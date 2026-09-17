//! `LocalWhisperAdapter` against a fixed, checked-in audio fixture with a
//! known transcript (T010, spec 008 research.md §7 "Testing"). No real
//! microphone is involved. Downloading the real model *is* — see the
//! `#[ignore]` reason below, a direct consequence of the operator's
//! 2026-09-17 decision to fetch the bundled model on first use rather than
//! bundle it in the installer (see the module doc on `local.rs`).

use std::io::Read;
use std::path::Path;

use super::local::{ensure_model_files, LocalWhisperAdapter};
use super::{CanonicalPcm, SttAdapter};

/// Minimal RIFF/WAVE chunk walker for the checked-in fixture only — reads
/// 16-bit PCM straight into normalized `f32`. Not a general decoder: the
/// fixture is already 16 kHz mono (`stt::SAMPLE_RATE_HZ`), so this
/// deliberately does not exercise resampling/downmixing (that is
/// `audio::`'s job, tested there against synthetic buffers instead, T006).
fn read_wav_as_canonical_pcm(path: &Path) -> CanonicalPcm {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");

    let mut pos = 12;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body_start = pos + 8;
        if chunk_id == b"data" {
            data = Some(&bytes[body_start..body_start + chunk_size]);
            break;
        }
        // Chunks are padded to an even number of bytes.
        pos = body_start + chunk_size + (chunk_size % 2);
    }
    let data = data.expect("fixture wav has no data chunk");
    let samples = data
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
        .collect();
    CanonicalPcm { samples }
}

#[tokio::test]
#[ignore = "downloads the real whisper-tiny model from Hugging Face on first \
            run (~75MB) and runs real CPU inference — a direct consequence \
            of downloading the bundled model on first use instead of \
            shipping it in the installer. Run explicitly with \
            `cargo test --features llm-cpu -- --ignored transcribes_known_fixture`."]
async fn transcribes_known_fixture() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stt/fixtures/jfk.wav");
    let pcm = read_wav_as_canonical_pcm(&fixture);

    let model_dir = std::env::temp_dir().join("holzi-whisper-tiny-test-cache");
    std::fs::create_dir_all(&model_dir).unwrap();
    ensure_model_files(&model_dir)
        .await
        .expect("failed to fetch whisper-tiny fixture cache");

    let adapter =
        LocalWhisperAdapter::load_from_dir(&model_dir).expect("failed to load whisper-tiny");
    let text = adapter
        .transcribe(&pcm)
        .await
        .expect("transcription failed")
        .to_lowercase();

    // The JFK inaugural excerpt ("ask not what your country can do for
    // you, ask what you can do for your country") — checking for the
    // distinctive content words rather than an exact string match, since
    // greedy decoding without punctuation/casing normalization can differ
    // in capitalization or minor phrasing from the canonical transcript.
    assert!(text.contains("country"), "unexpected transcript: {text}");
    assert!(text.contains("ask"), "unexpected transcript: {text}");
}
