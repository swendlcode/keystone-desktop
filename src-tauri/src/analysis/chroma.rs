//! S3 — the 36-bin HPCP.
//!
//! Faraldo's settings exactly, plus bgate's additions in order. Note that the
//! hop is larger than the window: frames are *sampled* every 371 ms, each 93 ms
//! long, deliberately not overlapped. That is not a typo to be tidied up.

use crate::error::Result;

pub const WINDOW: usize = 4096;
/// Deliberately > WINDOW. See the module note.
pub const HOP: usize = 16_384;
pub const F_MIN_HZ: f32 = 25.0;
pub const F_MAX_HZ: f32 = 3500.0;
pub const BINS_PER_SEMITONE: usize = 3;
pub const N_BINS: usize = 36;
pub const WEIGHTING_SEMITONES: f32 = 1.0;

/// bgate's first addition: kill percussive low-frequency noise before the FFT.
pub const HIGH_PASS_HZ: f32 = 200.0;

/// bgate's gate, applied **once, after** the vector is normalised to sum 1.
/// Gating before normalising gates a different set of bins.
pub const GATE_THRESHOLD: f32 = 0.2;

/// A 36-bin pitch class profile, normalised to sum 1 and gated.
#[derive(Debug, Clone)]
pub struct Pcp {
    pub bins: [f32; N_BINS],
}

impl Pcp {
    /// Folds to 12 bins by summing each semitone's three sub-bins. The template
    /// profiles are 12-bin, so the match happens here.
    pub fn fold_to_12(&self) -> [f32; 12] {
        let mut out = [0.0f32; 12];
        for (i, v) in self.bins.iter().enumerate() {
            out[i / BINS_PER_SEMITONE] += *v;
        }
        out
    }
}

/// One PCP for the whole track.
///
/// Owes, in this order: 200 Hz high-pass, STFT, spectral whitening, HPCP
/// accumulation over 25-3500 Hz into 36 bins with 1-semitone weighting,
/// aggregate every frame, normalise to sum 1, gate at 0.2, then apply the
/// detune as a ±1 shift of the three bins per semitone.
pub fn pcp(_samples: &[f32], _sample_rate: u32, _tuning_cents: f32) -> Result<Pcp> {
    todo!("owes: the full HPCP chain in the documented order")
}

/// 12-bin chroma sampled at each strong beat. S8's downbeat phase scores chroma
/// change at candidate downbeats, and S9 uses it as its seventh feature.
pub fn chroma_per_beat(
    _samples: &[f32],
    _sample_rate: u32,
    _beats_ms: &[f64],
) -> Result<Vec<[f32; 12]>> {
    todo!("owes: beat-synchronous 12-bin chroma, 7 octaves from 27.5 Hz")
}
