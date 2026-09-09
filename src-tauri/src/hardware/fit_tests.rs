//! Unit tests for the model-fit classifier. Pure functions, no I/O.

use super::fit::{classify, Fit, ModelFitInputs};
use super::{Backend, HardwareInfo};

fn hw(backend: Backend, ram: u64, vram: Option<u64>) -> HardwareInfo {
    HardwareInfo {
        backend,
        total_ram_bytes: ram,
        available_ram_bytes: ram,
        vram_bytes: vram,
    }
}

#[test]
fn cpu_fits_when_model_plus_kv_below_70_percent_ram() {
    let info = hw(Backend::Cpu, 16 * 1024 * 1024 * 1024, None);
    let m = ModelFitInputs {
        file_size_bytes: 400 * 1024 * 1024, // 400 MiB — Qwen2.5-0.5B Q4
        context_window: Some(4096),
    };
    assert_eq!(classify(&info, m), Fit::Fits);
}

#[test]
fn cpu_too_big_when_model_exceeds_ram() {
    let info = hw(Backend::Cpu, 4 * 1024 * 1024 * 1024, None);
    let m = ModelFitInputs {
        file_size_bytes: 20 * 1024 * 1024 * 1024, // 20 GiB — 30B Q4
        context_window: Some(4096),
    };
    assert_eq!(classify(&info, m), Fit::TooBig);
}

#[test]
fn cuda_uses_vram_not_ram() {
    // Small VRAM, plenty of RAM. Model should be TooBig despite the RAM.
    let info = hw(Backend::Cuda, 32 * 1024 * 1024 * 1024, Some(2 * 1024 * 1024 * 1024));
    let m = ModelFitInputs {
        file_size_bytes: 5 * 1024 * 1024 * 1024, // 5 GiB
        context_window: Some(4096),
    };
    assert_eq!(classify(&info, m), Fit::TooBig);
}

#[test]
fn cuda_without_vram_probe_is_unknown() {
    let info = hw(Backend::Cuda, 32 * 1024 * 1024 * 1024, None);
    let m = ModelFitInputs {
        file_size_bytes: 400 * 1024 * 1024,
        context_window: Some(4096),
    };
    assert_eq!(classify(&info, m), Fit::Unknown);
}

#[test]
fn tight_when_above_70_but_below_100_percent() {
    // 10 GB RAM. Model = 7.5 GB. That is 75 % of target, above 70 %.
    let info = hw(Backend::Cpu, 10 * 1024 * 1024 * 1024, None);
    let m = ModelFitInputs {
        file_size_bytes: (7.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        context_window: Some(1024),
    };
    assert_eq!(classify(&info, m), Fit::Tight);
}
