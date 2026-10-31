//! S10 — energy, 1-10, as an integer.
//!
//! **It is not loudness.** Loudness alone explains arousal at R² ≈ 0.43-0.55
//! while timbre feature sets reach 0.69-0.75, and every modern master sits near
//! -8 LUFS, so integrated loudness is nearly constant across a commercial
//! library and would give almost every track the same number.
//!
//! **The formula, in full.** With the frozen constants from
//! [`constants`]:
//!
//! ```text
//! zᶠ     = clamp((ln xᶠ − μᶠ) / σᶠ, −3, +3)      per feature f
//! uᶠ     = Φ(zᶠ)                                 standard normal CDF
//! raw    = Σᶠ wᶠ · uᶠ                            Σ wᶠ = 1, so raw ∈ [0, 1]
//! energy = clamp(round(1 + 9 · Q(raw)), 1, 10)   Q = the decile ladder
//! ```
//!
//! The seven `xᶠ` are the per-bar features in [`features`], each aggregated
//! over the span as a 10%-trimmed mean. That is the whole model. **There is no
//! stored database of reference compositions and no correlation step**: the same
//! feature vector produces the same integer on any machine at a given
//! `ENERGY_MODEL_VERSION`, with nothing to look up.
//!
//! **Genre stability without a genre classifier.** Six of the seven features are
//! ratios or per-beat rates; the only absolute quantity, `dynamic_span`, carries
//! the lowest weight and is itself a difference of two percentiles. Do not
//! normalise per genre — that needs a genre detector, and the user's library is
//! not a sample of the world. Test the invariance instead; see `tests`.
//!
//! **Patent.** US 8,865,993 (Mixed In Key LLC, Active, anticipated expiry 2033)
//! has, in all three independent claims, the step of estimating energy by
//! correlating an attribute profile against *a stored database of reference
//! compositions with known energy values*. A direct feature-to-score formula
//! with published fixed coefficients, no reference store and no correlation step
//! reads outside the claims as written. That is why the constants are baked in
//! rather than the score coming from a nearest-neighbour regressor over
//! labelled references. This is an engineering opinion, not legal advice: get a
//! real opinion before the energy feature ships, and keep the definition
//! documented and reference-free in the meantime.

pub mod constants;
pub mod features;
pub mod loudness;
pub mod score;

#[cfg(test)]
mod tests;

use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::spectra::Reductions;
use crate::analysis::structure::Structure;
use crate::error::Result;
use crate::models::{DetectedRegion, EnergyFeatures, EnergyResult};

pub use constants::{constants, FrozenConstants, WEIGHTS};
pub use loudness::{ShortTerm, CORE_LU_WINDOW, SHORT_TERM_SECONDS};
pub use score::{normal_cdf, trimmed_mean};

/// Per-bar values are aggregated as a 10%-trimmed mean, so one anomalous bar
/// does not move the score and the drop still does.
pub const TRIM_FRACTION: f32 = 0.10;

/// A section shorter than this gets the track's own features rather than its
/// own: two bars is not enough to trim a mean over.
pub const MIN_REGION_BARS_FOR_OWN_ENERGY: usize = 4;

/// The core region, as a millisecond span.
pub fn core_region(short_term: &ShortTerm, downbeats_ms: &[f64]) -> Result<(f64, f64)> {
    loudness::core_region(short_term, downbeats_ms)
}

/// The seven features over one span, each per bar and then trimmed-mean
/// aggregated.
pub fn features(
    reductions: &Reductions,
    grid: &Grid,
    short_term: &ShortTerm,
    span_ms: (f64, f64),
) -> Result<EnergyFeatures> {
    features::features(reductions, grid, short_term, span_ms)
}

/// `z = clamp((ln x − mu)/sigma, −3, 3)`, `u = Phi(z)`, `raw = sum(w · u)`,
/// `energy = clamp(round(1 + 9·Q(raw)), 1, 10)`.
pub fn score(features: &EnergyFeatures, k: &FrozenConstants) -> EnergyResult {
    score::score(features, k)
}

/// The monotone piecewise-linear quantile map.
pub fn quantile_map(raw: f32, k: &FrozenConstants) -> f32 {
    score::quantile_map(raw, &k.quantiles, &k.quantile_targets)
}

/// Track energy is the trimmed mean over the **core**, not the mean of the
/// section energies.
///
/// It is therefore deliberately drop-weighted: the core excludes the intro and
/// outro, and the trimmed mean over what is left is dominated by whatever the
/// track spends most of its length doing. Document that wherever it surfaces,
/// because someone will file it as a bug.
pub fn track_energy(
    reductions: &Reductions,
    grid: &Grid,
    short_term: &ShortTerm,
    core_ms: (f64, f64),
) -> Result<EnergyResult> {
    let f = features(reductions, grid, short_term, core_ms)?;
    Ok(score(&f, &constants()))
}

/// Section energy, with the identical constants so it is comparable.
pub fn region_energy(
    reductions: &Reductions,
    grid: &Grid,
    short_term: &ShortTerm,
    span_ms: (f64, f64),
) -> Result<u8> {
    let f = features(reductions, grid, short_term, span_ms)?;
    Ok(score(&f, &constants()).energy)
}

/// S10 end to end: the core region, the track energy, and a number on every
/// region row.
///
/// The waveform displays detected energy levels per section, so the region rows
/// are not an extra — they are what that part of the player reads.
pub fn analyze(
    structure: &Structure,
    short_term: &ShortTerm,
    regions: &mut [DetectedRegion],
) -> Result<(EnergyResult, (f64, f64))> {
    let core = core_region(short_term, &structure.grid.downbeats_ms)?;
    let track = track_energy(&structure.reductions, &structure.grid, short_term, core)?;
    fill_region_energies(structure, short_term, regions, &track);
    Ok((track, core))
}

/// Fills `energy` on every region row, falling back to the track number for a
/// section too short to trim a mean over.
pub fn fill_region_energies(
    structure: &Structure,
    short_term: &ShortTerm,
    regions: &mut [DetectedRegion],
    track: &EnergyResult,
) {
    for region in regions.iter_mut() {
        // `max(0)` before the cast: a negative bar count would wrap to a huge
        // usize and make every short region look long enough.
        let long_enough =
            region.bars.unwrap_or(0).max(0) as usize >= MIN_REGION_BARS_FOR_OWN_ENERGY;
        region.energy = if long_enough {
            region_energy(
                &structure.reductions,
                &structure.grid,
                short_term,
                (region.start_ms, region.end_ms),
            )
            .ok()
            .or(Some(track.energy))
        } else {
            Some(track.energy)
        };
    }
}
