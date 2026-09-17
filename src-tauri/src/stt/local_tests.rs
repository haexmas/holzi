//! `LocalWhisperAdapter` against a fixed, checked-in audio fixture with a
//! known transcript (T010, spec 008 research.md §7 "Testing"). No real
//! microphone is involved. Downloading the real model *is* — see the
//! `#[ignore]` reason below, a direct consequence of the operator's
//! 2026-09-17 decision to fetch the bundled model on first use rather than
//! bundle it in the installer (see the module doc on `local.rs`).

use std::io::Read;
use std::path::Path;

use super::catalog::SttCatalogEntry;
use super::local::{ensure_model_files, LocalWhisperAdapter};
use super::{CanonicalPcm, SttAdapter};

fn whisper_tiny_entry() -> SttCatalogEntry {
    super::catalog::get("whisper-tiny")
        .expect("whisper-tiny catalog entry")
        .clone()
}

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
    ensure_model_files(&model_dir, &whisper_tiny_entry())
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

#[tokio::test]
#[ignore = "downloads the real whisper-tiny model from Hugging Face on first \
            run (~75MB) and runs real CPU inference — a direct consequence \
            of downloading the bundled model on first use instead of \
            shipping it in the installer. Run explicitly with \
            `cargo test --features llm-cpu -- --ignored transcribes_german_fixture`."]
async fn transcribes_german_fixture() {
    // 9.5s clip trimmed from a public-domain LibriVox German reading
    // (grimm_maerchen_1_librivox, archive.org) — lands on the standard
    // German LibriVox disclaimer read before the story itself ("Alle
    // LibriVox-Aufnahmen sind... öffentlichem Besitz..."), not the fairy
    // tale text, but that's immaterial here: it's unambiguous German.
    // Added 2026-09-17 as a regression test for `detect_language`
    // misidentifying spoken German as English when scored against
    // Whisper's full 99-language set (see `SUPPORTED_LANGUAGES` in
    // `local.rs`).
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stt/fixtures/de_sample.wav");
    let pcm = read_wav_as_canonical_pcm(&fixture);

    let model_dir = std::env::temp_dir().join("holzi-whisper-tiny-test-cache");
    std::fs::create_dir_all(&model_dir).unwrap();
    ensure_model_files(&model_dir, &whisper_tiny_entry())
        .await
        .expect("failed to fetch whisper-tiny fixture cache");

    let adapter =
        LocalWhisperAdapter::load_from_dir(&model_dir).expect("failed to load whisper-tiny");
    let text = adapter
        .transcribe(&pcm)
        .await
        .expect("transcription failed")
        .to_lowercase();

    assert!(text.contains("librivox"), "unexpected transcript: {text}");
    assert!(
        text.contains("öffentlichem"),
        "unexpected transcript: {text}"
    );
}

/// Spec 010: `is_complete_file`/`is_complete_model` guard against a
/// previous run's interrupted/truncated download (or a hand-deleted file)
/// being mistaken for an installed one. No network involved — pure
/// filesystem checks.
mod completeness {
    use std::path::Path;

    use tempfile::tempdir;

    use super::super::local::{is_complete_file, is_complete_model};

    const CONFIG_FILENAME: &str = "config.json";
    const TOKENIZER_FILENAME: &str = "tokenizer.json";
    const WEIGHTS_FILENAME: &str = "model.safetensors";

    fn write(dir: &Path, filename: &str, contents: &[u8]) {
        std::fs::write(dir.join(filename), contents).unwrap();
    }

    #[test]
    fn missing_file_is_incomplete() {
        let dir = tempdir().unwrap();
        assert!(!is_complete_file(&dir.path().join("config.json")));
    }

    #[test]
    fn zero_byte_file_is_incomplete() {
        let dir = tempdir().unwrap();
        write(dir.path(), "config.json", b"");
        assert!(!is_complete_file(&dir.path().join("config.json")));
    }

    #[test]
    fn nonempty_file_is_complete() {
        let dir = tempdir().unwrap();
        write(dir.path(), "config.json", b"{}");
        assert!(is_complete_file(&dir.path().join("config.json")));
    }

    #[test]
    fn model_with_one_missing_file_is_incomplete() {
        let dir = tempdir().unwrap();
        write(dir.path(), CONFIG_FILENAME, b"{}");
        write(dir.path(), TOKENIZER_FILENAME, b"{}");
        // WEIGHTS_FILENAME intentionally absent.
        assert!(!is_complete_model(dir.path()));
    }

    #[test]
    fn model_with_one_zero_byte_file_is_incomplete() {
        let dir = tempdir().unwrap();
        write(dir.path(), CONFIG_FILENAME, b"{}");
        write(dir.path(), TOKENIZER_FILENAME, b"{}");
        write(dir.path(), WEIGHTS_FILENAME, b""); // truncated download
        assert!(!is_complete_model(dir.path()));
    }

    #[test]
    fn model_with_all_three_nonempty_files_is_complete() {
        let dir = tempdir().unwrap();
        write(dir.path(), CONFIG_FILENAME, b"{}");
        write(dir.path(), TOKENIZER_FILENAME, b"{}");
        write(dir.path(), WEIGHTS_FILENAME, b"weights");
        assert!(is_complete_model(dir.path()));
    }
}

/// Spec 010: a complete legacy `whisper-tiny` install is migrated into the
/// canonical directory; an incomplete one is left alone. No network
/// involved — pure filesystem checks via the `&Path`-based
/// `migrate_legacy_if_present` (the `AppHandle`-resolving wrapper,
/// `resolve_or_migrate_model_dir`, isn't unit-tested here for the same
/// reason no other `AppHandle`-taking function in this codebase is: there
/// is no Tauri app mocking convention in this repo).
mod legacy_migration {
    use tempfile::tempdir;

    use super::super::local::{is_complete_model, migrate_legacy_if_present};

    const CONFIG_FILENAME: &str = "config.json";
    const TOKENIZER_FILENAME: &str = "tokenizer.json";
    const WEIGHTS_FILENAME: &str = "model.safetensors";

    #[test]
    fn complete_legacy_install_is_copied_into_canonical_dir() {
        let legacy = tempdir().unwrap();
        let canonical = tempdir().unwrap();
        std::fs::write(legacy.path().join(CONFIG_FILENAME), b"{}").unwrap();
        std::fs::write(legacy.path().join(TOKENIZER_FILENAME), b"{}").unwrap();
        std::fs::write(legacy.path().join(WEIGHTS_FILENAME), b"weights").unwrap();

        migrate_legacy_if_present(legacy.path(), canonical.path());

        assert!(is_complete_model(canonical.path()));
        assert_eq!(
            std::fs::read(canonical.path().join(WEIGHTS_FILENAME)).unwrap(),
            b"weights"
        );
    }

    #[test]
    fn incomplete_legacy_install_is_not_copied() {
        let legacy = tempdir().unwrap();
        let canonical = tempdir().unwrap();
        std::fs::write(legacy.path().join(CONFIG_FILENAME), b"{}").unwrap();
        // Legacy install missing tokenizer/weights — not complete.

        migrate_legacy_if_present(legacy.path(), canonical.path());

        assert!(!canonical.path().join(CONFIG_FILENAME).exists());
        assert!(!is_complete_model(canonical.path()));
    }

    #[test]
    fn does_not_itself_guard_against_overwriting_an_already_complete_canonical_dir() {
        // `migrate_legacy_if_present` only checks *legacy*'s completeness —
        // it is `resolve_or_migrate_model_dir` that guards the call with
        // `!is_complete_model(&dir)` so this is never reached once the
        // canonical dir is already complete. Documented here so that
        // invariant stays visible next to the function it protects.
        let legacy = tempdir().unwrap();
        let canonical = tempdir().unwrap();
        std::fs::write(legacy.path().join(CONFIG_FILENAME), b"legacy").unwrap();
        std::fs::write(legacy.path().join(TOKENIZER_FILENAME), b"legacy").unwrap();
        std::fs::write(legacy.path().join(WEIGHTS_FILENAME), b"legacy").unwrap();
        std::fs::write(canonical.path().join(CONFIG_FILENAME), b"canonical").unwrap();
        std::fs::write(canonical.path().join(TOKENIZER_FILENAME), b"canonical").unwrap();
        std::fs::write(canonical.path().join(WEIGHTS_FILENAME), b"canonical").unwrap();

        migrate_legacy_if_present(legacy.path(), canonical.path());

        assert_eq!(
            std::fs::read(canonical.path().join(CONFIG_FILENAME)).unwrap(),
            b"legacy"
        );
    }
}
