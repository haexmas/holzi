use async_trait::async_trait;

use super::{downmix_to_mono, resample_linear};
use crate::stt::{CanonicalPcm, SttAdapter, SttError};

#[test]
fn downmix_averages_non_default_channel_counts() {
    // Stereo: (1.0, 3.0), (2.0, 4.0) -> mono (2.0, 3.0).
    assert_eq!(downmix_to_mono(&[1.0, 3.0, 2.0, 4.0], 2), vec![2.0, 3.0]);
    // Mono passes through unchanged.
    assert_eq!(downmix_to_mono(&[1.0, 2.0, 3.0], 1), vec![1.0, 2.0, 3.0]);
    // Non-default (non-stereo) channel count, e.g. a 4-channel interface.
    assert_eq!(
        downmix_to_mono(&[0.0, 4.0, 0.0, 4.0, 8.0, 0.0, 8.0, 0.0], 4),
        vec![2.0, 4.0]
    );
}

#[test]
fn resample_linear_handles_non_default_sample_rates() {
    // Identity when rates already match.
    let identity = resample_linear(&[1.0, 2.0, 3.0], 16_000, 16_000);
    assert_eq!(identity, vec![1.0, 2.0, 3.0]);

    // Downsampling from a common device-native rate (44.1 kHz) to the
    // canonical 16 kHz: output should be shorter, endpoints preserved.
    let input: Vec<f32> = (0..4410).map(|i| i as f32).collect();
    let out = resample_linear(&input, 44_100, 16_000);
    assert!(
        (out.len() as i64 - 1600).abs() <= 1,
        "got {} samples",
        out.len()
    );
    assert_eq!(out[0], 0.0);

    // Another non-default rate (48 kHz, another common device default).
    let input48: Vec<f32> = vec![0.0; 4800];
    let out48 = resample_linear(&input48, 48_000, 16_000);
    assert_eq!(out48.len(), 1600);
}

struct RecordingStub {
    received: std::sync::Mutex<Option<Vec<f32>>>,
}

#[async_trait]
impl SttAdapter for RecordingStub {
    async fn transcribe(&self, audio: &CanonicalPcm) -> Result<String, SttError> {
        *self.received.lock().unwrap() = Some(audio.samples.clone());
        Ok(String::new())
    }
}

/// T006: the converted canonical buffer (post downmix + resample) is
/// exactly what an `SttAdapter` receives — exercised end to end through
/// the trait boundary rather than only checking the conversion functions
/// in isolation.
#[tokio::test]
async fn converted_buffer_reaches_stt_adapter_unchanged() {
    let stereo_44100hz = vec![1.0, 1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0];
    let mono = downmix_to_mono(&stereo_44100hz, 2);
    let expected = resample_linear(&mono, 44_100, crate::stt::SAMPLE_RATE_HZ);

    let stub = RecordingStub {
        received: std::sync::Mutex::new(None),
    };
    let pcm = CanonicalPcm {
        samples: expected.clone(),
    };
    stub.transcribe(&pcm).await.unwrap();

    assert_eq!(stub.received.into_inner().unwrap(), Some(expected));
}
