//! The seven energy features, each computed per bar and aggregated as a
//! 10%-trimmed mean.
//!
//! Six of the seven are ratios or per-beat rates, which is what buys genre
//! stability without a genre classifier: a 174 BPM drum and bass track and a
//! 124 BPM house track are compared on *proportions*, not on counts that scale
//! with tempo or on levels that scale with mastering. The seventh,
//! `dynamic_span`, is a difference of two loudness percentiles and so is also
//! unmoved by gain. Nothing here is normalised per genre — that would need a
//! genre detector, and the user's library is not a sample of the world.

use crate::analysis::energy::loudness::{self, ShortTerm};
use crate::analysis::energy::TRIM_FRACTION;
use crate::analysis::structure::boundaries::peak_pick_curve;
use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::spectra::Reductions;
use crate::error::{KeystoneError, Result};
use crate::models::EnergyFeatures;

/// Onset picking on the normalised flux curve: a local maximum within this many
/// frames, above [`ONSET_FRACTION`] of the curve's own maximum.
///
/// Three frames at hop 512 is ~35 ms, which separates a snare from its own
/// decay without splitting a flam into two onsets.
pub const ONSET_WINDOW_FRAMES: usize = 3;
pub const ONSET_FRACTION: f32 = 0.25;

pub fn features(
    r: &Reductions,
    grid: &Grid,
    st: &ShortTerm,
    span_ms: (f64, f64),
) -> Result<EnergyFeatures> {
    let bars = bar_windows(grid, span_ms);
    if bars.is_empty() {
        return Err(KeystoneError::Analysis(
            "energy needs at least one bar inside the span".into(),
        ));
    }

    let per_bar = per_bar_features(r, grid, &bars);
    let mut agg = per_bar.map(|mut v| {
        crate::analysis::energy::score::trimmed_mean(&mut v, TRIM_FRACTION)
    });
    // A ratio of exactly zero is a real measurement, but `ln 0` is not, and the
    // score's floor reads it as the bottom of the scale either way.
    agg.iter_mut().for_each(|v| *v = v.max(0.0));

    Ok(EnergyFeatures {
        onset_rate: agg[0],
        flux_mean: agg[1],
        low_ratio: agg[2],
        high_ratio: agg[3],
        centroid: agg[4],
        perc_ratio: agg[5],
        // Not a per-bar quantity: the R128 short-term window is 3 s, which is
        // longer than a bar at every tempo Keystone folds into. It is measured
        // once over the span, so its trimmed mean is itself.
        dynamic_span: loudness::dynamic_span(st, span_ms),
    })
}

/// The six spectral features, one value per bar each.
fn per_bar_features(r: &Reductions, grid: &Grid, bars: &[(f64, f64)]) -> [Vec<f32>; 6] {
    let onsets = onset_frames(r);
    let beats_per_bar = grid.beats_per_bar.max(1) as f32;
    let mut out: [Vec<f32>; 6] = Default::default();

    for (start, end) in bars {
        let lo = r.ms_to_frame(*start).min(r.n_frames());
        let hi = r.ms_to_frame(*end).min(r.n_frames()).max(lo + 1);
        let n_onsets = onsets.iter().filter(|f| **f >= lo && **f < hi).count() as f32;
        let power = sum(slice(&r.total_power, lo, hi));
        let perc = sum(slice(&r.percussive, lo, hi));
        let harm = sum(slice(&r.harmonic, lo, hi));

        out[0].push(n_onsets / beats_per_bar);
        out[1].push(mean(slice(&r.flux, lo, hi)));
        out[2].push(ratio(sum(slice(&r.low_power, lo, hi)), power));
        out[3].push(ratio(sum(slice(&r.high_power, lo, hi)), power));
        out[4].push(weighted_centroid(r, lo, hi));
        out[5].push(ratio(perc, perc + harm));
    }
    out
}

/// Bar windows inside the span, from the downbeat grid where it reaches and
/// from the mean bar length where it does not.
pub fn bar_windows(grid: &Grid, span_ms: (f64, f64)) -> Vec<(f64, f64)> {
    let inside: Vec<f64> = grid
        .downbeats_ms
        .iter()
        .copied()
        .filter(|d| *d >= span_ms.0 - 1.0 && *d <= span_ms.1 + 1.0)
        .collect();

    if inside.len() >= 2 {
        return inside.windows(2).map(|w| (w[0], w[1])).collect();
    }
    if grid.bar_ms <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut t = span_ms.0;
    while t + grid.bar_ms <= span_ms.1 + 1.0 {
        out.push((t, t + grid.bar_ms));
        t += grid.bar_ms;
    }
    if out.is_empty() && span_ms.1 > span_ms.0 {
        out.push(span_ms);
    }
    out
}

/// Onset frames over the whole track, picked once.
///
/// Once rather than per bar: the threshold is relative to the track's own flux
/// maximum, so picking inside a bar would make a quiet bar's loudest tick an
/// onset and destroy the comparison between bars that `onset_rate` exists for.
pub fn onset_frames(r: &Reductions) -> Vec<usize> {
    peak_pick_curve(&r.flux, ONSET_WINDOW_FRAMES, ONSET_FRACTION)
        .into_iter()
        .map(|(i, _)| i)
        .collect()
}

fn weighted_centroid(r: &Reductions, lo: usize, hi: usize) -> f32 {
    let total = sum(slice(&r.total, lo, hi));
    if total <= f32::EPSILON {
        return 0.0;
    }
    let weighted: f32 = slice(&r.centroid, lo, hi)
        .iter()
        .zip(slice(&r.total, lo, hi).iter())
        .map(|(c, t)| c * t)
        .sum();
    weighted / total
}

fn slice(series: &[f32], lo: usize, hi: usize) -> &[f32] {
    series.get(lo..hi.min(series.len())).unwrap_or(&[])
}

fn sum(values: &[f32]) -> f32 {
    values.iter().sum()
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f32>() / values.len() as f32
}

fn ratio(part: f32, whole: f32) -> f32 {
    if whole <= f32::EPSILON {
        0.0
    } else {
        part / whole
    }
}
