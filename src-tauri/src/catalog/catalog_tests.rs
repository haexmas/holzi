//! Unit tests for the tier-selection algorithm. Uses the compiled-in
//! catalog against synthetic `HardwareInfo` scenarios.

use super::{recommend_tiers, Tier};
use crate::hardware::{Backend, Fit, HardwareInfo};

fn hw(backend: Backend, ram: u64, vram: Option<u64>) -> HardwareInfo {
    HardwareInfo {
        backend,
        total_ram_bytes: ram,
        available_ram_bytes: ram,
        vram_bytes: vram,
    }
}

#[test]
/// Roomy CUDA host: every catalog entry fits, so Easy is the smallest,
/// Sweet is the largest, and Max mirrors Sweet.
fn recommend_tiers_on_large_vram_host() {
    let info = hw(
        Backend::Cuda,
        64 * 1024 * 1024 * 1024,
        Some(24 * 1024 * 1024 * 1024),
    );
    let tiers = recommend_tiers(&info).expect("non-empty catalog");
    assert_eq!(tiers[0].tier, Tier::Easy);
    assert_eq!(tiers[1].tier, Tier::Sweet);
    assert_eq!(tiers[2].tier, Tier::Max);
    assert!(tiers[0].entry.approx_size_bytes <= tiers[1].entry.approx_size_bytes);
    assert!(tiers[1].entry.approx_size_bytes <= tiers[2].entry.approx_size_bytes);
    assert_eq!(tiers[0].fit, Fit::Fits);
    // On a big host, Sweet must also be Fits.
    assert_eq!(tiers[1].fit, Fit::Fits);
}

#[test]
/// CPU-only host with modest RAM: the tier algorithm never returns
/// fewer than three chips even when several entries are TooBig.
fn recommend_tiers_returns_three_even_when_most_are_too_big() {
    let info = hw(Backend::Cpu, 3 * 1024 * 1024 * 1024, None);
    let tiers = recommend_tiers(&info).expect("non-empty catalog");
    assert_eq!(tiers.len(), 3);
    assert_eq!(tiers[0].tier, Tier::Easy);
}

#[test]
/// CUDA host with no VRAM probe: every entry is Fit::Unknown, so Easy
/// falls back to the smallest Unknown; Sweet falls back to the median.
fn recommend_tiers_falls_back_when_hardware_unknown() {
    let info = hw(Backend::Cuda, 32 * 1024 * 1024 * 1024, None);
    let tiers = recommend_tiers(&info).expect("non-empty catalog");
    assert_eq!(tiers[0].fit, Fit::Unknown);
    // Easy is the smallest entry when nothing fits; the deterministic
    // sort by (size, id) puts it first.
    let mut sorted_ids: Vec<String> = crate::catalog::entries()
        .iter()
        .map(|e| e.id.clone())
        .collect();
    sorted_ids.sort_by(|a, b| {
        let ea = crate::catalog::get(a).unwrap();
        let eb = crate::catalog::get(b).unwrap();
        ea.approx_size_bytes
            .cmp(&eb.approx_size_bytes)
            .then_with(|| a.cmp(b))
    });
    assert_eq!(tiers[0].entry.id, sorted_ids[0]);
}
