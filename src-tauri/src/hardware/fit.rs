//! Model-fit classifier. Rule per operator decision (2026-09-09):
//! "VRAM-Regel + Kontextfenster-Reserve" — model weights plus a rough
//! KV-cache estimate for the model's default context window must fit
//! into the target memory. Target memory is VRAM on CUDA hosts (when
//! probed), else RAM. `Fit::Fits` needs to leave 30 % headroom;
//! `Fit::Tight` is under-budget but consumes more than 70 %;
//! `Fit::TooBig` overruns the target.

use serde::{Deserialize, Serialize};

use super::{Backend, HardwareInfo};

/// Rough KV-cache bytes per token, per layer, per attention head. This
/// is a very coarse approximation good enough to keep the catalog
/// filter honest: real KV footprint depends on model dimensions we do
/// not encode in the catalog. Undersizes for chunky context/heads,
/// oversizes for pruned models — both correct into a conservative
/// filter that leans against surprises at load time.
///
/// 2 (K + V) × 2 bytes (fp16) × 4 heads × 24 layers = 384 bytes/token.
/// Assume 24 layers as a compromise between 0.5B (24) and 7B (32).
const KV_BYTES_PER_TOKEN: u64 = 384;

/// How much of the target memory a "fits" model is allowed to fill.
const HEADROOM_RATIO: f64 = 0.70;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// Fits within the headroom ratio.
    Fits,
    /// Fits raw but exceeds the headroom ratio.
    Tight,
    /// Does not fit into the target memory at all.
    TooBig,
    /// Target memory could not be established (e.g. CUDA with no
    /// `nvidia-smi` output). UI should treat this as "unknown, allow
    /// with a note" rather than blocking.
    Unknown,
}

/// Everything the fit classifier needs about a model. Kept as a tiny
/// struct so callers can build it from either the catalog JSON or from
/// a downloaded-file size + `models.context_window` join.
#[derive(Debug, Clone, Copy)]
pub struct ModelFitInputs {
    pub file_size_bytes: u64,
    /// If `None`, we assume 4096 tokens — the smallest realistic
    /// context for a modern instruction-tuned model.
    pub context_window: Option<u64>,
}

/// Classifies a model against the current hardware snapshot.
pub fn classify(info: &HardwareInfo, m: ModelFitInputs) -> Fit {
    let target = target_bytes(info);
    let Some(target) = target else {
        return Fit::Unknown;
    };

    let ctx = m.context_window.unwrap_or(4096);
    let need = m.file_size_bytes.saturating_add(ctx.saturating_mul(KV_BYTES_PER_TOKEN));

    if need > target {
        Fit::TooBig
    } else if need as f64 > target as f64 * HEADROOM_RATIO {
        Fit::Tight
    } else {
        Fit::Fits
    }
}

fn target_bytes(info: &HardwareInfo) -> Option<u64> {
    match info.backend {
        Backend::Cuda => info.vram_bytes,
        Backend::Metal | Backend::Cpu => Some(info.available_ram_bytes),
    }
}
