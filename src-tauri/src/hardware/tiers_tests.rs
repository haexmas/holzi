//! Unit tests for the generic tier-picking algorithm. Uses a synthetic
//! candidate type so these tests don't depend on either concrete catalog —
//! `catalog/catalog_tests.rs` and `stt/catalog_tests.rs` cover the
//! per-catalog wiring on top of this.

use super::tiers::{pick_three, Tier};
use super::Fit;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    id: &'static str,
    size: u64,
}

fn candidates(items: &[(&'static str, u64, Fit)]) -> Vec<(Candidate, Fit)> {
    items
        .iter()
        .map(|(id, size, fit)| (Candidate { id, size: *size }, *fit))
        .collect()
}

fn size_of(c: &Candidate) -> u64 {
    c.size
}

fn id_of(c: &Candidate) -> &str {
    c.id
}

#[test]
fn none_for_empty_candidates() {
    assert!(pick_three(Vec::<(Candidate, Fit)>::new(), size_of, id_of).is_none());
}

#[test]
fn everything_fits_easy_is_smallest_sweet_and_max_are_largest() {
    let input = candidates(&[
        ("small", 1, Fit::Fits),
        ("medium", 2, Fit::Fits),
        ("large", 3, Fit::Fits),
    ]);
    let [easy, sweet, max] = pick_three(input, size_of, id_of).expect("non-empty");
    assert_eq!(
        easy,
        (
            Tier::Easy,
            Candidate {
                id: "small",
                size: 1
            },
            Fit::Fits
        )
    );
    assert_eq!(
        sweet,
        (
            Tier::Sweet,
            Candidate {
                id: "large",
                size: 3
            },
            Fit::Fits
        )
    );
    assert_eq!(
        max,
        (
            Tier::Max,
            Candidate {
                id: "large",
                size: 3
            },
            Fit::Fits
        )
    );
}

#[test]
fn falls_back_through_tight_unknown_too_big_when_nothing_fits() {
    let input = candidates(&[
        ("a", 1, Fit::TooBig),
        ("b", 2, Fit::Unknown),
        ("c", 3, Fit::Tight),
    ]);
    let [easy, sweet, max] = pick_three(input, size_of, id_of).expect("non-empty");
    // Easy prefers Fits > Tight > Unknown > TooBig; only Tight is present.
    assert_eq!(easy.0, Tier::Easy);
    assert_eq!(easy.1.id, "c");
    assert_eq!(easy.2, Fit::Tight);
    // Sweet has no Fits candidate, falls back to the median (index 1 of 3).
    assert_eq!(sweet.1.id, "b");
    // Max accepts Fits or Tight; "c" is the only Tight candidate.
    assert_eq!(max.1.id, "c");
}

#[test]
fn returns_three_even_with_a_single_candidate() {
    let input = candidates(&[("only", 1, Fit::Fits)]);
    let picked = pick_three(input, size_of, id_of).expect("non-empty");
    assert_eq!(picked.len(), 3);
    assert!(picked.iter().all(|(_, c, _)| c.id == "only"));
}

#[test]
fn sorts_by_size_then_id_for_deterministic_ties() {
    let input = candidates(&[("b", 1, Fit::TooBig), ("a", 1, Fit::TooBig)]);
    let [easy, ..] = pick_three(input, size_of, id_of).expect("non-empty");
    // Same size -> tie-broken by id ascending -> "a" sorts first.
    assert_eq!(easy.1.id, "a");
}
