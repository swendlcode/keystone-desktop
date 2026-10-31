//! Rendering internals for the synthetic fixture: how a section becomes frames
//! of magnitude, and how the two perturbations that are not a resample are
//! applied.

use ndarray::Array2;

use crate::analysis::beats::BeatAnalysis;
use crate::analysis::energy::loudness::ShortTerm;

use super::{Codec, Perturbation, Section};

#[allow(clippy::too_many_arguments)]
pub(super) fn render_section(
    harmonic: &mut Array2<f32>,
    percussive: &mut Array2<f32>,
    s: &Section,
    first_bar: usize,
    bar_ms: f64,
    beat_ms: f64,
    fps: f64,
    bin_hz: f32,
) {
    let kick = band(bin_hz, 40.0, 140.0);
    let snare = band(bin_hz, 180.0, 700.0);
    let hat = band(bin_hz, 6500.0, 9000.0);

    for b in 0..s.bars {
        let ramp = if s.ramp {
            0.25 + 0.75 * (b as f32 + 1.0) / s.bars as f32
        } else {
            1.0
        };
        let bar_start = (first_bar + b) as f64 * bar_ms;
        // A chord every four bars, so the chroma and log-magnitude SSMs have a
        // harmonic change to find and the breakdown is not one flat block.
        let chord = ((first_bar + b) / 4) % 3;
        fill_harmonic(harmonic, bar_start, bar_ms, fps, bin_hz, s.harm, chord);

        for beat in 0..4 {
            let t = frame_at(bar_start + beat as f64 * beat_ms, fps);
            impulse(percussive, t, &kick, s.kick * ramp);
            if beat % 2 == 1 {
                impulse(percussive, t, &snare, s.snare * ramp);
            }
            for eighth in 0..2 {
                let th = frame_at(bar_start + (beat as f64 + eighth as f64 * 0.5) * beat_ms, fps);
                impulse(percussive, th, &hat, s.hat * ramp);
            }
        }
    }
}

fn fill_harmonic(
    harmonic: &mut Array2<f32>,
    start_ms: f64,
    bar_ms: f64,
    fps: f64,
    bin_hz: f32,
    level: f32,
    chord: usize,
) {
    // Three partials per chord tone over a triad, plus the root two octaves
    // down. The sub-bass is not decoration: without a fundamental under 150 Hz
    // the `low_ratio` feature has nothing but the kick transient to measure, and
    // no real dance record is built that way.
    let roots = [[220.0, 277.2, 329.6], [246.9, 311.1, 370.0], [196.0, 246.9, 293.7]];
    let lo = frame_at(start_ms, fps);
    let hi = frame_at(start_ms + bar_ms, fps).min(harmonic.nrows());
    let bass = roots[chord % 3][0] / 4.0;
    // Wide enough to cover at least one FFT bin at any chord: a 4% window
    // around 50 Hz falls between bins at 10.8 Hz resolution and the bass
    // silently disappears for that chord.
    let bass_bins = band(bin_hz, bass * 0.80, bass * 1.25);
    for t in lo..hi {
        for bin in bass_bins.clone() {
            if bin < harmonic.ncols() {
                harmonic[[t, bin]] += level * 3.0;
            }
        }
    }
    for f in roots[chord % 3] {
        for harmonic_n in 1..=3 {
            let centre = f * harmonic_n as f32;
            let bins = band(bin_hz, centre * 0.98, centre * 1.02);
            let amp = level / harmonic_n as f32;
            for t in lo..hi {
                for bin in bins.clone() {
                    if bin < harmonic.ncols() {
                        harmonic[[t, bin]] += amp;
                    }
                }
            }
        }
    }
}

/// Bin range covering `[lo_hz, hi_hz)`. Unclamped at the top: every caller
/// writes through a bounds check against the matrix it is filling, and clamping
/// here against a constant is what tied the fixture to one sample rate.
fn band(bin_hz: f32, lo_hz: f32, hi_hz: f32) -> std::ops::Range<usize> {
    let lo = (lo_hz / bin_hz).ceil().max(0.0) as usize;
    let hi = (hi_hz / bin_hz).floor().max(0.0) as usize + 1;
    lo..hi.max(lo)
}

/// A three-frame exponential decay, which is what a transient looks like at
/// hop 512.
fn impulse(m: &mut Array2<f32>, frame: usize, bins: &std::ops::Range<usize>, amp: f32) {
    if amp <= 0.0 {
        return;
    }
    for (k, decay) in [1.0f32, 0.45, 0.2].iter().enumerate() {
        let t = frame + k;
        if t >= m.nrows() {
            return;
        }
        for bin in bins.clone() {
            if bin < m.ncols() {
                m[[t, bin]] += amp * decay;
            }
        }
    }
}

fn frame_at(ms: f64, fps: f64) -> usize {
    (ms * fps / 1000.0).round().max(0.0) as usize
}

/// Gain and the wide-Q tilt. Both are per-bin multipliers, which is exactly
/// what they are in a real signal path.
pub(super) fn apply_shaping(m: &mut Array2<f32>, bin_hz: f32, p: Perturbation) {
    if (p.gain - 1.0).abs() < f32::EPSILON && p.tilt_db.abs() < f32::EPSILON {
        return;
    }
    let gains: Vec<f32> = (0..m.ncols())
        .map(|bin| {
            let hz = ((bin as f32) * bin_hz).max(20.0);
            p.gain * 10f32.powf(p.tilt_db * (hz / 1000.0).log10() / 20.0)
        })
        .collect();
    // Frames outer, bins inner: the matrix is row-major and the other order
    // strides by a full row per access.
    for t in 0..m.nrows() {
        for (bin, g) in gains.iter().enumerate() {
            m[[t, bin]] *= *g;
        }
    }
}

/// A short-term loudness series derived from the render.
///
/// Not R128-exact — it has no K-weighting — but it is a 3 s moving average of
/// power in dB, so it carries the only two properties the energy model asks of
/// it: it moves with level, and the gap between two of its percentiles does not.
pub(super) fn short_term_from(magnitude: &Array2<f32>, fps: f64) -> ShortTerm {
    let step = (fps / 10.0).round().max(1.0) as usize;
    let window = (fps * 3.0).round().max(1.0) as usize;
    let power: Vec<f32> = magnitude
        .rows()
        .into_iter()
        .map(|r| {
            let s: f32 = r.iter().sum();
            s * s / r.len() as f32
        })
        .collect();

    let mut lufs = Vec::new();
    let mut end = step;
    while end <= power.len() {
        let lo = end.saturating_sub(window);
        let mean: f32 = power[lo..end].iter().sum::<f32>() / (end - lo) as f32;
        lufs.push(if mean > 0.0 {
            10.0 * mean.log10() - 0.691
        } else {
            f32::NEG_INFINITY
        });
        end += step;
    }
    ShortTerm::from_values(lufs, 100.0)
}

pub(super) fn grid_for(duration_ms: i64, beat_ms: f64) -> BeatAnalysis {
    let n_beats = (duration_ms as f64 / beat_ms).floor() as usize;
    let beats_ms: Vec<f64> = (0..n_beats).map(|i| i as f64 * beat_ms).collect();
    let strong_beats_ms: Vec<f64> = beats_ms.iter().step_by(2).copied().collect();
    BeatAnalysis {
        beats_ms,
        strong_beats_ms,
        beats_per_bar: 4,
        downbeat_phase: 0,
        downbeat_confidence: 0.9,
        // The fixture states its own grid, so both halves of the phase
        // decision are given rather than inferred.
        strong_beat_phase: 0,
        strong_beat_confidence: 0.95,
        phrase_bars: 4,
        phrase_phase: 0,
        phrase_confidence: 0.8,
    }
}

/// The lossy-codec surrogate: band-limit, discard the bins under the local
/// masking threshold, and put a quantisation noise floor where they were.
///
/// **Both thresholds are local to the frame and the neighbourhood, never a
/// fraction of the track's peak.** That is the whole difference between a
/// surrogate and nonsense: a flat floor at 0.4% of a kick transient, applied to
/// every bin of every frame, puts broadband noise into bands that were silent
/// and moved the synthetic techno track's energy by three steps — not because a
/// 128 kbps encode does that, but because no encoder puts noise where there is
/// no signal to mask it. Quantisation noise sits just under the signal in each
/// band, so the noise this adds is a fraction of the local magnitude.
///
/// Deterministic, because a test that fails one run in fifty is worse than no
/// test. See [`super::Codec`] for what this does and does not stand in for.
pub(super) fn apply_codec(m: &mut Array2<f32>, bin_hz: f32, codec: Codec, seed: u64) {
    /// Half-width of the masking neighbourhood, in bins. Roughly a critical
    /// band at the fixture's resolution.
    const NEIGHBOURHOOD: usize = 16;

    let cutoff_bin = (codec.cutoff_hz / bin_hz).ceil().max(0.0) as usize;
    let mut rng = Lcg(seed);
    let n_bins = m.ncols();
    let mut local = vec![0.0f32; n_bins];
    for t in 0..m.nrows() {
        for bin in 0..n_bins {
            let lo = bin.saturating_sub(NEIGHBOURHOOD);
            let hi = (bin + NEIGHBOURHOOD + 1).min(n_bins);
            local[bin] = (lo..hi).fold(0.0f32, |a, b| a.max(m[[t, b]]));
        }
        for bin in 0..n_bins {
            let v = &mut m[[t, bin]];
            if bin >= cutoff_bin {
                *v = 0.0;
                continue;
            }
            if *v < codec.hole_floor * local[bin] {
                *v = 0.0;
            }
            *v = (*v + codec.noise_floor * local[bin] * rng.unit()).max(0.0);
        }
    }
}

/// Fixed-seed LCG. A magnitude spectrum is non-negative, so the noise is too.
struct Lcg(u64);

impl Lcg {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 40) as f32 / 16_777_216.0
    }
}
