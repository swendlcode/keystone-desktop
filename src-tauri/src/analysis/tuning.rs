//! S2 — tuning estimate and the corrective resample.
//!
//! **This stage runs first and is not optional**, even though neither Faraldo
//! nor Sha'ath measured an aggregate gain on A440 EDM. The key model's front end
//! is 12 bins per octave — one bin per semitone, no sub-semitone resolution —
//! and it was measured collapsing to the wrong tonic *and* the wrong mode at
//! ±50 cents, with correcting the detune restoring the original prediction to
//! three decimals. This is cheap insurance against catastrophic tail failures:
//! vinyl rips, pitched edits, live recordings, turntable-pitched hip-hop. For
//! this audience, that tail is the audience.

use crate::error::Result;

pub const N_FFT: usize = 4096;
pub const HOP: usize = 1024;

/// Below this, correcting costs a resample and buys nothing.
pub const CORRECTION_THRESHOLD_CENTS: f32 = 10.0;

/// ±1/3 semitone around A440 (Faraldo's range). Anything outside is a bad
/// estimate rather than an unusually tuned record, and clamping is what stops
/// one noisy track being transposed.
pub const REF_HZ_MIN: f32 = 431.6;
pub const REF_HZ_MAX: f32 = 448.5;

#[derive(Debug, Clone, Copy)]
pub struct TuningEstimate {
    /// In [-50, +50].
    pub cents: f32,
    pub reference_hz: f32,
    /// Whether the caller should resample.
    pub needs_correction: bool,
}

/// Magnitude-weighted circular mean of `mod(12 * log2(f / (440/16)), 1)` folded
/// to [-0.5, 0.5) semitones, over parabolically interpolated spectral peaks
/// (Dressler & Streich). Circular, not linear: a linear mean of values either
/// side of the fold cancels to zero.
pub fn estimate(_samples: &[f32], _sample_rate: u32) -> Result<TuningEstimate> {
    todo!("owes: interpolated peak picking and the magnitude-weighted circular mean")
}

/// Resamples by `2^(-cents/1200)`. Only called when the offset clears the
/// threshold.
pub fn correct(_samples: &[f32], _sample_rate: u32, _cents: f32) -> Result<Vec<f32>> {
    todo!("owes: rubato resample by the detune ratio")
}
