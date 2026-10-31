//! S2 — the tuning estimate and the corrective resample.
//!
//! **This stage runs first and is not optional**, even though neither Faraldo
//! (neutral on GiantSteps) nor Sha'ath (who disabled tuning handling in
//! KeyFinder by default because it did not help on dance music) measured an
//! aggregate gain on A440 EDM. The reason is the tail, not the average: the key
//! model's front end is 12 bins per octave — one bin per semitone, no
//! sub-semitone resolution — and it was measured collapsing to the wrong tonic
//! *and* the wrong mode at ±50 cents, with correcting the detune restoring the
//! original prediction to three decimals. Vinyl rips, pitched edits, live
//! recordings and turntable-pitched hip-hop are the cases that break, and for
//! this audience that tail is the audience.
//!
//! The estimate is also a product surface in its own right: the UI shows the
//! offset in cents, so it is reported whether or not anything downstream acts
//! on it.

use crate::analysis::spectral::stft::{spectral_peaks, Peak, Stft, StftOpts};
use crate::error::Result;

pub const N_FFT: usize = 4096;
pub const HOP: usize = 1024;

/// Below this, correcting costs a resample and buys nothing.
pub const CORRECTION_THRESHOLD_CENTS: f32 = 10.0;

/// ±1/3 semitone around A440 (Faraldo's range). An implied reference outside it
/// is a bad estimate rather than an unusually tuned record, and clamping is
/// what stops one noisy track being transposed wholesale.
pub const REF_HZ_MIN: f32 = 431.6;
pub const REF_HZ_MAX: f32 = 448.5;

/// A0. The fold is expressed against `440 / 16` so that every octave of A lands
/// on an exact integer and the modulo has no accumulated error to inherit.
const A0_HZ: f32 = 440.0 / 16.0;

/// Below 200 Hz a bin is wider than the interval being measured even after
/// interpolation, and above 4 kHz partials of real instruments drift sharp
/// enough (inharmonicity, intermodulation) to pull the mean.
const PEAK_LO_HZ: f32 = 200.0;
const PEAK_HI_HZ: f32 = 4000.0;

/// Peaks under this fraction of the frame maximum are noise.
const PEAK_FLOOR_RATIO: f32 = 0.01;

/// Mean resultant length below this means the peak deviations were spread round
/// the circle rather than clustered, i.e. the material is not pitched enough for
/// the estimate to mean anything. Percussion and noise land here, and a track
/// that lands here must not be resampled on the strength of it.
///
/// Provisional: it separates synthetic tonal material from synthetic percussion
/// by a wide margin, but it has not been calibrated against a real corpus.
pub const MIN_CONCENTRATION: f32 = 0.25;

#[derive(Debug, Clone, Copy)]
pub struct TuningEstimate {
    /// In [-50, +50] before the reference clamp, in [-33.4, +33.1] after it.
    pub cents: f32,
    pub reference_hz: f32,
    /// Whether the caller should resample. False on unpitched material however
    /// large `cents` looks.
    pub needs_correction: bool,
    /// Circular concentration of the peak deviations, 0 to 1. The honest
    /// companion to `cents`: it says whether the number is worth believing.
    pub concentration: f32,
}

impl TuningEstimate {
    /// What to report for material that gave nothing to measure: A440, no
    /// correction, no confidence. Not an error — most of a DJ library is at
    /// A440 and an ambient intro simply has no peaks.
    pub fn a440() -> Self {
        Self { cents: 0.0, reference_hz: 440.0, needs_correction: false, concentration: 0.0 }
    }
}

/// Magnitude-weighted circular mean of `mod(12 * log2(f / (440/16)), 1)`, folded
/// to [-0.5, 0.5) semitones, over parabolically interpolated spectral peaks
/// (Dressler & Streich).
///
/// Circular, not linear. A track 48 cents flat produces deviations at -0.48 and,
/// wherever a partial lands just the other side of the fold, at +0.52 → -0.48;
/// a linear mean of values either side of the boundary cancels toward zero and
/// reports a perfectly tuned track. Summing unit vectors and taking the angle is
/// the only form that wraps correctly, and it hands back the concentration for
/// free.
pub fn estimate(samples: &[f32], sample_rate: u32) -> Result<TuningEstimate> {
    if samples.is_empty() || sample_rate == 0 {
        return Ok(TuningEstimate::a440());
    }
    let mut stft = Stft::new(StftOpts::new(N_FFT, HOP));
    let bin_hz = stft.bin_hz(sample_rate);
    let hi_hz = PEAK_HI_HZ.min(sample_rate as f32 * 0.45);
    let mut peaks: Vec<Peak> = Vec::with_capacity(64);
    let mut sum_cos = 0.0f64;
    let mut sum_sin = 0.0f64;
    let mut sum_weight = 0.0f64;

    stft.process(
        samples,
        |_frame| {},
        |_, magnitude| {
            spectral_peaks(magnitude, bin_hz, PEAK_LO_HZ, hi_hz, PEAK_FLOOR_RATIO, &mut peaks);
            for p in peaks.iter() {
                if p.freq_hz <= 0.0 || !p.magnitude.is_finite() || p.magnitude <= 0.0 {
                    continue;
                }
                let semitones = 12.0 * (p.freq_hz / A0_HZ).log2();
                let deviation = semitones - semitones.round();
                let theta = 2.0 * std::f32::consts::PI * deviation;
                let w = p.magnitude as f64;
                sum_cos += w * theta.cos() as f64;
                sum_sin += w * theta.sin() as f64;
                sum_weight += w;
            }
        },
    );

    if sum_weight <= 0.0 {
        return Ok(TuningEstimate::a440());
    }
    let resultant = (sum_cos * sum_cos + sum_sin * sum_sin).sqrt();
    let concentration = (resultant / sum_weight) as f32;
    let raw_cents = (sum_sin.atan2(sum_cos) / (2.0 * std::f64::consts::PI) * 100.0) as f32;

    // Clamp the implied reference, then read the cents back off the clamped
    // reference, so the two fields can never disagree.
    let reference_hz = (440.0 * 2f32.powf(raw_cents / 1200.0)).clamp(REF_HZ_MIN, REF_HZ_MAX);
    let cents = 1200.0 * (reference_hz / 440.0).log2();
    let needs_correction =
        cents.abs() > CORRECTION_THRESHOLD_CENTS && concentration >= MIN_CONCENTRATION;
    Ok(TuningEstimate { cents, reference_hz, needs_correction, concentration })
}

/// Half-width of the Lanczos kernel, in source samples.
const LANCZOS_A: i64 = 3;

/// Resamples so that every partial moves by `-cents`, i.e. so that the result
/// sits at A440.
///
/// The sign is the thing to get right here, because getting it wrong transposes
/// the user's whole library in the direction nobody will think to check.
/// Frequencies must be multiplied by `2^(-cents/1200)`; resampling to `R` times
/// as many output samples and playing them back at the original rate divides
/// every frequency by `R`, so `R = 2^(cents/1200)`. A track 25 cents flat
/// (`cents = -25`) therefore gets *shorter*.
///
/// A short Lanczos-3 kernel rather than `rubato`: the ratio is within ±2% of
/// unity across the whole clamped range, the consumer is a magnitude spectrum
/// binned at three bins per semitone, and keeping S2 free of a chunked
/// streaming resampler keeps that dependency in `decode`, which owns the real
/// 22.05 kHz conversion.
pub fn correct(samples: &[f32], _sample_rate: u32, cents: f32) -> Result<Vec<f32>> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let ratio = 2f64.powf(cents as f64 / 1200.0);
    if !(0.5..=2.0).contains(&ratio) {
        return Ok(samples.to_vec());
    }
    let out_len = ((samples.len() as f64) * ratio).round() as usize;
    let mut out = vec![0.0f32; out_len.max(1)];
    let n = samples.len() as i64;
    for (m, slot) in out.iter_mut().enumerate() {
        let src = m as f64 / ratio;
        let base = src.floor() as i64;
        let mut acc = 0.0f32;
        let mut norm = 0.0f32;
        for t in (base - LANCZOS_A + 1)..=(base + LANCZOS_A) {
            let w = lanczos((src - t as f64) as f32, LANCZOS_A as f32);
            if w == 0.0 || t < 0 || t >= n {
                continue;
            }
            acc += w * samples[t as usize];
            norm += w;
        }
        *slot = if norm.abs() > 1e-6 { acc / norm } else { 0.0 };
    }
    Ok(out)
}

fn lanczos(x: f32, a: f32) -> f32 {
    let ax = x.abs();
    if ax < 1e-7 {
        return 1.0;
    }
    if ax >= a {
        return 0.0;
    }
    let pix = std::f32::consts::PI * x;
    (pix.sin() / pix) * ((pix / a).sin() / (pix / a))
}
