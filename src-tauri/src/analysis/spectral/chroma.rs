//! S3 — the 36-bin HPCP, and the beat-synchronous 12-bin chroma.
//!
//! Faraldo's settings exactly, plus bgate's additions in the order they are
//! listed. Note that the hop is larger than the window: frames are *sampled*
//! every 371 ms, each 93 ms long, deliberately not overlapped. That is not a
//! typo to be tidied up — the stage is measuring which pitch classes a track
//! contains, not when, and sampling is both cheaper and less biased toward
//! whichever section happens to be densest.

use std::f32::consts::PI;

use crate::analysis::spectral::stft::{
    spectral_peaks, Peak, Stft, StftOpts, DEFAULT_PEAK_FLOOR_RATIO,
};
use crate::analysis::spectral::vqt::{Vqt, VqtSpec};
use crate::analysis::spectral::whitening::{high_pass_frame, Whitener, BANDS_PER_OCTAVE};
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
/// Note that it sits above `F_MIN_HZ`, so the effective band is 200-3500 Hz. A
/// bass note below the cutoff still reaches its own pitch class through its
/// second harmonic, which is why removing the fundamental costs less than the
/// kick leakage it removes.
pub const HIGH_PASS_HZ: f32 = 200.0;

/// bgate's gate, applied **once, after** the vector is normalised.
///
/// Relative to the largest bin, not an absolute floor. A 36-bin vector
/// normalised to sum 1 has a mean bin of 0.028 and, on real music, a maximum
/// around 0.05 to 0.15 — an absolute 0.2 would zero the entire vector and the
/// stage would return silence for every track. The gate can only be read
/// relative to the maximum, and the test below pins that: gating never empties
/// a vector that had energy in it.
pub const GATE_THRESHOLD: f32 = 0.2;

/// Cents per sub-bin at three bins per semitone. The detune shift is ±1 of
/// these, which is also why S2 clamps the implied reference to ±1/3 semitone:
/// the two numbers are the same number.
pub const CENTS_PER_BIN: f32 = 100.0 / BINS_PER_SEMITONE as f32;

/// Reference for the A440 grid. Pitch class 9 is A.
const REF_A_HZ: f32 = 440.0;
const REF_A_PITCH_CLASS: f32 = 9.0;

/// A 36-bin pitch class profile, normalised and gated.
#[derive(Debug, Clone, Copy)]
pub struct Pcp {
    pub bins: [f32; N_BINS],
}

impl Pcp {
    pub fn empty() -> Self {
        Self { bins: [0.0; N_BINS] }
    }

    /// True when nothing reached the vector — silence, or material with no
    /// resolvable partials at all. Callers must treat this as "no answer"
    /// rather than as a key, which is what the ambiguity path in `key` does.
    pub fn is_empty(&self) -> bool {
        !self.bins.iter().any(|v| *v > 0.0)
    }

    /// Folds to 12 bins by summing each semitone's three sub-bins. The template
    /// profiles are 12-bin, so the match happens here.
    ///
    /// The grouping is **centred**, `(i + 1) / 3`, not `i / 3`. Bin `i` sits at
    /// `i/3` semitones above C, so the three bins belonging to pitch class `p`
    /// are `3p - 1`, `3p` and `3p + 1` — one a third of a semitone flat, one on
    /// the note, one a third sharp. Grouping `i / 3` instead puts `3p + 2` with
    /// `p` even though it is a third of a semitone *below* `p + 1`, and since
    /// the 1-semitone weighting window spreads every peak over its neighbours
    /// that misalignment is not a rounding detail: a C major triad measured
    /// with `i / 3` folded its C energy into B strongly enough that B outranked
    /// E in the result.
    pub fn fold_to_12(&self) -> [f32; 12] {
        let mut out = [0.0f32; 12];
        for (i, v) in self.bins.iter().enumerate() {
            out[((i + 1) / BINS_PER_SEMITONE) % 12] += *v;
        }
        out
    }

    /// Rotates by whole sub-bins, wrapping. Positive moves energy up in pitch.
    pub fn rotate(&self, shift: i32) -> Pcp {
        if shift == 0 {
            return *self;
        }
        let n = N_BINS as i32;
        let mut out = [0.0f32; N_BINS];
        for (i, v) in self.bins.iter().enumerate() {
            out[((i as i32 + shift).rem_euclid(n)) as usize] = *v;
        }
        Pcp { bins: out }
    }
}

/// How far to rotate the gated vector to undo a detune, in sub-bins.
///
/// A track `+c` cents sharp puts the energy that belongs in bin `b` into bin
/// `b + c/33.3`, so the correction rotates *down* by that much — hence the
/// negation. Clamped to ±1 because that is the whole correction the three
/// bins per semitone can express, and because S2's reference clamp already
/// bounds the input to ±33.4 cents.
pub fn detune_bin_shift(tuning_cents: f32) -> i32 {
    if !tuning_cents.is_finite() {
        return 0;
    }
    (-(tuning_cents / CENTS_PER_BIN).round() as i32).clamp(-1, 1)
}

/// One PCP for the whole of `samples`.
///
/// `tuning_cents` is the offset of **the buffer being handed in**: pass S2's
/// estimate when the audio was left alone, or zero when the caller already
/// resampled it. Getting that wrong corrects twice and shifts the result a
/// third of a semitone, which on a borderline track moves the badge.
///
/// Order, and it is the order bgate specifies: 200 Hz high-pass, FFT, spectral
/// whitening, HPCP accumulation over 25-3500 Hz into 36 bins with 1-semitone
/// weighting, aggregate every frame, normalise, gate once, then apply the
/// detune shift.
pub fn pcp(samples: &[f32], sample_rate: u32, tuning_cents: f32) -> Result<Pcp> {
    if samples.is_empty() || sample_rate == 0 {
        return Ok(Pcp::empty());
    }
    let mut stft = Stft::new(StftOpts::new(WINDOW, HOP));
    let bin_hz = stft.bin_hz(sample_rate);
    let f_max = F_MAX_HZ.min(sample_rate as f32 * 0.45);
    let mut whitener =
        Whitener::new(stft.n_bins(), bin_hz, F_MIN_HZ, f_max, BANDS_PER_OCTAVE);
    let mut peaks: Vec<Peak> = Vec::with_capacity(64);
    // f64 because a long track sums hundreds of frames of squared magnitudes
    // and the small bins are the ones the gate decides on.
    let mut acc = [0.0f64; N_BINS];

    stft.process(
        samples,
        |frame| high_pass_frame(frame, sample_rate, HIGH_PASS_HZ),
        |_, magnitude| {
            whitener.apply(magnitude);
            spectral_peaks(
                magnitude,
                bin_hz,
                F_MIN_HZ,
                f_max,
                DEFAULT_PEAK_FLOOR_RATIO,
                &mut peaks,
            );
            for p in peaks.iter() {
                accumulate_peak(&mut acc, p);
            }
        },
    );

    let total: f64 = acc.iter().sum();
    if !(total > 0.0) {
        return Ok(Pcp::empty());
    }
    let mut bins = [0.0f32; N_BINS];
    for (b, a) in bins.iter_mut().zip(acc.iter()) {
        *b = (*a / total) as f32;
    }
    let loudest = bins.iter().copied().fold(0.0f32, f32::max);
    let threshold = GATE_THRESHOLD * loudest;
    for v in bins.iter_mut() {
        if *v < threshold {
            *v = 0.0;
        }
    }
    Ok(Pcp { bins }.rotate(detune_bin_shift(tuning_cents)))
}

/// Spreads one peak's energy over the sub-bins within one semitone of it,
/// `cos^2`-weighted (Gómez's HPCP weighting).
///
/// Energy, not magnitude: the published definition sums squared amplitudes, and
/// after whitening the difference decides which bins survive the gate.
fn accumulate_peak(acc: &mut [f64; N_BINS], p: &Peak) {
    if p.freq_hz <= 0.0 || !p.magnitude.is_finite() || p.magnitude <= 0.0 {
        return;
    }
    let semitones = 12.0 * (p.freq_hz / REF_A_HZ).log2() + REF_A_PITCH_CLASS;
    let pos = (semitones * BINS_PER_SEMITONE as f32).rem_euclid(N_BINS as f32);
    let support = WEIGHTING_SEMITONES * BINS_PER_SEMITONE as f32;
    let energy = (p.magnitude * p.magnitude) as f64;
    let lo = (pos - support).floor() as i32;
    let hi = (pos + support).ceil() as i32;
    for b in lo..=hi {
        let d = circular_distance(b as f32, pos, N_BINS as f32);
        if d > support {
            continue;
        }
        let w = (0.5 * PI * d / support).cos();
        acc[(b.rem_euclid(N_BINS as i32)) as usize] += energy * (w * w) as f64;
    }
}

fn circular_distance(a: f32, b: f32, n: f32) -> f32 {
    let d = (a - b).abs() % n;
    d.min(n - d)
}

/// Window and hop for the beat-synchronous chroma. Shorter window than the
/// track PCP because a beat at 128 BPM is only 469 ms and the feature has to
/// resolve a chord change inside one.
pub const BEAT_CHROMA_WINDOW: usize = 8192;
pub const BEAT_CHROMA_HOP: usize = 1024;

/// 12-bin chroma sampled at each beat, over seven octaves from A0.
///
/// Used twice downstream: the downbeat phase score counts chroma change at
/// candidate downbeats, and the structure stage takes it as its seventh
/// feature. Built on the VQT rather than on HPCP peak picking, because here the
/// question is how much the harmony *changed* between two adjacent beats, and a
/// filterbank answers that more stably than a peak list whose membership can
/// turn over completely between frames.
///
/// Neither high-passed nor whitened, also deliberately: a bass note moving is
/// most of the signal this feature exists to catch.
pub fn chroma_per_beat(
    samples: &[f32],
    sample_rate: u32,
    beats_ms: &[f64],
) -> Result<Vec<[f32; 12]>> {
    if beats_ms.is_empty() || samples.is_empty() || sample_rate == 0 {
        return Ok(Vec::new());
    }
    let mut stft = Stft::new(StftOpts::new(BEAT_CHROMA_WINDOW, BEAT_CHROMA_HOP));
    let vqt = Vqt::new(VqtSpec::CHROMA_7_OCTAVES, sample_rate, BEAT_CHROMA_WINDOW);
    let n_bins = vqt.n_bins();
    let mut per_beat = vec![vec![0.0f32; n_bins]; beats_ms.len()];
    let centre_offset = BEAT_CHROMA_WINDOW as f64 / 2.0;
    let rate = sample_rate as f64;

    stft.process(
        samples,
        |_frame| {},
        |frame_index, magnitude| {
            let t_ms =
                ((frame_index * BEAT_CHROMA_HOP) as f64 + centre_offset) / rate * 1000.0;
            let beat = match beats_ms.partition_point(|b| *b <= t_ms) {
                0 => return,
                k => k - 1,
            };
            vqt.accumulate(magnitude, &mut per_beat[beat]);
        },
    );

    Ok(per_beat
        .into_iter()
        .map(|bins| {
            let mut chroma = vqt.fold_to_chroma(&bins);
            let sum: f32 = chroma.iter().sum();
            if sum > 0.0 {
                for v in chroma.iter_mut() {
                    *v /= sum;
                }
            }
            chroma
        })
        .collect())
}
