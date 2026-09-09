//! Minimal hardware detection used to filter the model catalog.
//!
//! The plan (§"Lokale Inferenz: Runtime und Modellbezug") is explicit:
//! hardware info is **information, not a gate**. The filter here labels
//! catalog entries with `Fit::Fits` / `Fit::Tight` / `Fit::TooBig` so
//! the UI can render them grouped — a load attempt with insufficient
//! memory still runs, and the failure surfaces from mistralrs directly.
//!
//! Backend detection is compile-time (whatever `mistralrs` was built
//! with). RAM comes from `sysinfo`. VRAM is best-effort: for CUDA hosts
//! we shell out to `nvidia-smi` (already present when CUDA is), for
//! Metal we return `None` and rely on unified memory ≈ RAM, for CPU we
//! return `None` because there is no separate device.

use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sysinfo::System;

mod fit;
#[cfg(test)]
mod fit_tests;

pub use fit::{classify, Fit, ModelFitInputs};

/// Tauri command: current hardware snapshot. Called once at onboarding
/// to size the model suggestions, and on demand from Settings.
#[tauri::command]
pub async fn get_hardware_info() -> HardwareInfo {
    // Cheap enough to run inline — see `probe()` for cost analysis.
    probe()
}

/// Snapshot of the host relevant to the model catalog UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub backend: Backend,
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    /// Only populated on CUDA hosts where `nvidia-smi` returned a
    /// value; `None` on CPU/Metal or when the probe failed.
    pub vram_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Cpu,
    Cuda,
    Metal,
}

impl Backend {
    pub const fn from_features() -> Self {
        if cfg!(feature = "llm-cuda") {
            Backend::Cuda
        } else if cfg!(feature = "llm-metal") {
            Backend::Metal
        } else {
            Backend::Cpu
        }
    }
}

/// Probes the host. Fast enough to call on every catalog listing (the
/// expensive branch is `nvidia-smi` on CUDA hosts, capped by a 500 ms
/// wall clock via `timeout`).
pub fn probe() -> HardwareInfo {
    let mut sys = System::new();
    sys.refresh_memory();
    let total_ram_bytes = sys.total_memory();
    let available_ram_bytes = sys.available_memory();

    let backend = Backend::from_features();
    let vram_bytes = match backend {
        Backend::Cuda => probe_cuda_vram(),
        Backend::Metal | Backend::Cpu => None,
    };

    HardwareInfo {
        backend,
        total_ram_bytes,
        available_ram_bytes,
        vram_bytes,
    }
}

/// Best-effort VRAM detection via `nvidia-smi`. Runs synchronously with
/// a hard cap of 500 ms; any failure (missing binary, non-zero exit,
/// unparseable output) returns `None` — VRAM is informational.
fn probe_cuda_vram() -> Option<u64> {
    // `nvidia-smi` on non-nvidia hosts still emits an error banner and
    // exits non-zero, so a plain Command::output with a short timeout
    // is safe. We use GNU `timeout(1)` to guarantee the wall clock cap
    // because `Command::output` has no built-in one and pulling in
    // wait4/select for one call is overkill.
    let out = Command::new("timeout")
        .arg("0.5")
        .arg("nvidia-smi")
        .arg("--query-gpu=memory.total")
        .arg("--format=csv,noheader,nounits")
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let stdout = String::from_utf8(out.stdout).ok()?;
    // First line, first GPU. Value is in MiB per the format spec.
    let mib: u64 = stdout.lines().next()?.trim().parse().ok()?;
    Some(mib * 1024 * 1024)
}

/// The wall-clock cap on the CUDA VRAM probe. Exported for tests.
#[allow(dead_code)]
pub const CUDA_PROBE_TIMEOUT: Duration = Duration::from_millis(500);
