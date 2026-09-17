//! Generic hardware-fit tier-picking algorithm, shared between the LLM
//! catalog (`catalog::recommend_tiers`) and the STT catalog
//! (`stt::catalog::recommend_tiers`, spec 010). Extracted out of
//! `catalog::recommend_tiers` — `stt/local.rs`'s module doc already flagged
//! this as a pre-existing gap ("`catalog::recommend_tiers` is hardcoded to
//! the LLM catalog's `CatalogEntry` type... out of scope for this pass").

use serde::Serialize;

use super::Fit;

/// Which onboarding-tier a recommended entry represents. Lives in
/// `hardware` (not `catalog`) so `hardware` never depends on `catalog` —
/// `catalog`/`stt::catalog` depend on `hardware`, never the reverse.
/// Re-exported as `catalog::Tier` for existing call sites.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Smallest candidate that fits.
    Easy,
    /// Largest candidate that fits.
    Sweet,
    /// Largest candidate that fits or is `Tight`.
    Max,
}

/// Picks three tier-labelled candidates from an already-fit-classified
/// list — the algorithm `catalog::recommend_tiers` used before this
/// extraction, generalized over any candidate type (see
/// `contracts/tauri-commands.md` in spec 002 and spec 010 for the two
/// concrete catalogs this backs):
///
/// 1. Sort candidates ascending by `(size_of(candidate), id_of(candidate))`.
/// 2. `Easy` = smallest `Fits`; fallback smallest `Tight`, then `Unknown`,
///    then `TooBig`, in that total order.
/// 3. `Sweet` = largest `Fits`; fallback: median candidate at `(n - 1) / 2`.
/// 4. `Max` = largest `Fits` or `Tight`; fallback: the resolved `Sweet`.
/// 5. Missing tiers reuse their fallback so the return is always three
///    recommendations for a non-empty input.
///
/// Returns `None` only when `candidates` is empty.
pub fn pick_three<T: Clone>(
    candidates: Vec<(T, Fit)>,
    size_of: impl Fn(&T) -> u64,
    id_of: impl Fn(&T) -> &str,
) -> Option<[(Tier, T, Fit); 3]> {
    let mut sorted = candidates;
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| {
        size_of(&a.0)
            .cmp(&size_of(&b.0))
            .then_with(|| id_of(&a.0).cmp(id_of(&b.0)))
    });

    let smallest_fits = sorted.iter().find(|(_, f)| *f == Fit::Fits);
    let smallest_tight = sorted.iter().find(|(_, f)| *f == Fit::Tight);
    let smallest_unknown = sorted.iter().find(|(_, f)| *f == Fit::Unknown);
    let smallest_too_big = sorted.iter().find(|(_, f)| *f == Fit::TooBig);
    let easy = smallest_fits
        .or(smallest_tight)
        .or(smallest_unknown)
        .or(smallest_too_big)
        .cloned()
        .unwrap_or_else(|| sorted[0].clone());

    let largest_fits = sorted.iter().rev().find(|(_, f)| *f == Fit::Fits);
    let median = sorted[(sorted.len() - 1) / 2].clone();
    let sweet = largest_fits.cloned().unwrap_or(median);

    let largest_fits_or_tight = sorted
        .iter()
        .rev()
        .find(|(_, f)| matches!(f, Fit::Fits | Fit::Tight));
    let max = largest_fits_or_tight
        .cloned()
        .unwrap_or_else(|| sweet.clone());

    let mk = |tier: Tier, (entry, fit): (T, Fit)| (tier, entry, fit);
    Some([
        mk(Tier::Easy, easy),
        mk(Tier::Sweet, sweet),
        mk(Tier::Max, max),
    ])
}
