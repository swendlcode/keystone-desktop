//! The score chain: `z` → `Φ` → weighted sum → quantile map → 1-10.
//!
//! Every step is arithmetic over the constants in
//! [`super::constants`]. There is no stored corpus, no nearest neighbour and no
//! correlation against reference tracks with known energy values; a feature
//! vector in gives the same integer out on any machine, forever, at a given
//! `ENERGY_MODEL_VERSION`.

use crate::analysis::energy::constants::{FrozenConstants, Z_CLAMP};
use crate::analysis::version::ENERGY_MODEL_VERSION;
use crate::models::{EnergyFeatures, EnergyResult};

/// `ln x` needs a positive `x`, and a silent bar legitimately produces a zero
/// ratio. Flooring rather than erroring puts such a bar at `z = −3`, which is
/// the honest answer: no high-frequency content at all is the bottom of the
/// scale, not a failure.
const X_FLOOR: f32 = 1e-6;

pub fn feature_array(f: &EnergyFeatures) -> [f32; 7] {
    [
        f.onset_rate,
        f.flux_mean,
        f.low_ratio,
        f.high_ratio,
        f.centroid,
        f.perc_ratio,
        f.dynamic_span,
    ]
}

/// `raw = Σ wᶠ · Φ((ln xᶠ − μᶠ)/σᶠ)`, in [0, 1] because the weights sum to 1.
pub fn raw_score(features: &EnergyFeatures, k: &FrozenConstants) -> f32 {
    let x = feature_array(features);
    let mut raw = 0.0f32;
    for i in 0..7 {
        let sigma = if k.sigma[i].abs() > f32::EPSILON {
            k.sigma[i]
        } else {
            1.0
        };
        let z = ((x[i].max(X_FLOOR).ln() - k.mu[i]) / sigma).clamp(-Z_CLAMP, Z_CLAMP);
        raw += k.weights[i] * normal_cdf(z);
    }
    raw
}

pub fn score(features: &EnergyFeatures, k: &FrozenConstants) -> EnergyResult {
    let raw = raw_score(features, k);
    EnergyResult {
        energy: energy_from_raw(raw, k),
        raw: raw as f64,
        model_version: ENERGY_MODEL_VERSION,
        features: *features,
    }
}

pub fn energy_from_raw(raw: f32, k: &FrozenConstants) -> u8 {
    let q = quantile_map(raw, &k.quantiles, &k.quantile_targets);
    (1.0 + 9.0 * q).round().clamp(1.0, 10.0) as u8
}

/// Trimmed mean, trimming `fraction` from each tail.
///
/// 10% each side: one anomalous bar — a drop-out, a glitch, a tape stop — must
/// not move the number, while the forty bars of a drop still must.
pub fn trimmed_mean(values: &mut [f32], fraction: f32) -> f32 {
    let n = values.len();
    if n == 0 {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let cut = ((n as f32 * fraction).floor() as usize).min((n - 1) / 2);
    let kept = &values[cut..n - cut];
    kept.iter().sum::<f32>() / kept.len() as f32
}

/// Standard normal CDF: `0.5 · erfc(−z/√2)`.
pub fn normal_cdf(z: f32) -> f32 {
    0.5 * erfc(-z / std::f32::consts::SQRT_2)
}

/// Abramowitz & Stegun 7.1.26, |error| < 1.5e-7 — comfortably inside what a
/// 1-10 integer can notice, and it keeps the scale free of a special-function
/// dependency whose version could change the number under a user's library.
fn erfc(x: f32) -> f32 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-x * x).exp();
    1.0 - sign * erf
}

/// Monotone piecewise-linear interpolation from the corpus deciles onto the
/// integer boundaries.
pub fn quantile_map(raw: f32, quantiles: &[f32; 11], targets: &[f32; 11]) -> f32 {
    if raw <= quantiles[0] {
        return targets[0];
    }
    if raw >= quantiles[10] {
        return targets[10];
    }
    for i in 0..10 {
        let (lo, hi) = (quantiles[i], quantiles[i + 1]);
        if raw < hi {
            let span = hi - lo;
            let t = if span > f32::EPSILON {
                (raw - lo) / span
            } else {
                0.0
            };
            return targets[i] + t * (targets[i + 1] - targets[i]);
        }
    }
    targets[10]
}

/// Linear-interpolated percentile of an unsorted slice. Used for the core
/// region's 95th percentile and for `dynamic_span`.
pub fn percentile(values: &[f32], p: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v: Vec<f32> = values
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .collect();
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pos = (p.clamp(0.0, 1.0) * (v.len() - 1) as f32).clamp(0.0, (v.len() - 1) as f32);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    v[lo] + (pos - lo as f32) * (v[hi] - v[lo])
}
