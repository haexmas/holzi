//! Unit tests for the built-in STT catalog and its tier recommendation.

use super::catalog::{entries, get, recommend_tiers};
use crate::hardware::tiers::Tier;
use crate::hardware::{Backend, HardwareInfo};

fn hw(backend: Backend, ram: u64, vram: Option<u64>) -> HardwareInfo {
    HardwareInfo {
        backend,
        total_ram_bytes: ram,
        available_ram_bytes: ram,
        vram_bytes: vram,
    }
}

#[test]
fn catalog_contains_the_three_whisper_tiers() {
    let tiny = get("whisper-tiny").expect("whisper-tiny entry");
    assert_eq!(tiny.hf_repo, "openai/whisper-tiny");
    assert!(!tiny.hf_revision.is_empty());
    assert_ne!(tiny.hf_revision, "main");

    let base = get("whisper-base").expect("whisper-base entry");
    assert_eq!(base.hf_repo, "openai/whisper-base");

    let small = get("whisper-small").expect("whisper-small entry");
    assert_eq!(small.hf_repo, "openai/whisper-small");

    assert_eq!(entries().len(), 3);
}

#[test]
fn get_returns_none_for_unknown_id() {
    assert!(get("whisper-does-not-exist").is_none());
}

#[test]
/// Every tier is tiny relative to any realistic RAM budget, so on a roomy
/// host all three should classify as `Fits`, and the recommendation should
/// still be exactly three chips (Easy = smallest, Sweet/Max = largest).
fn recommend_tiers_on_roomy_host_prefers_smallest_for_easy() {
    let info = hw(Backend::Cpu, 16 * 1024 * 1024 * 1024, None);
    let tiers = recommend_tiers(&info).expect("non-empty catalog");
    assert_eq!(tiers[0].tier, Tier::Easy);
    assert_eq!(tiers[0].entry.id, "whisper-tiny");
    assert_eq!(tiers[1].tier, Tier::Sweet);
    assert_eq!(tiers[2].tier, Tier::Max);
    assert_eq!(tiers[2].entry.id, "whisper-small");
}

#[test]
fn recommend_tiers_returns_three_even_on_a_tiny_host() {
    // Even a very constrained host still gets three tier chips — Whisper
    // sizes are small enough that this is mostly a defensive check.
    let info = hw(Backend::Cpu, 256 * 1024 * 1024, None);
    let tiers = recommend_tiers(&info).expect("non-empty catalog");
    assert_eq!(tiers.len(), 3);
}
