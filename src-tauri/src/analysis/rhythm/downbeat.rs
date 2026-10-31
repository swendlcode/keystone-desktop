//! S8, part one: choosing the downbeat phase.
//!
//! Split out of `beats` because the scoring is the substance and the struct is
//! not: three terms, each normalised across the four candidates, and a pair of
//! two-way decisions taken in the order that makes the reliable one first.

use crate::analysis::rhythm::onset::{self, NoveltyCurves};

/// Four candidate downbeat phases in 4/4.
pub const DOWNBEAT_PHASES: usize = 4;

/// Novelty is sampled with this much slack, in frames, because an onset peak
/// sits a frame or two after the nominal beat.
const BEAT_TOLERANCE_FRAMES: usize = 3;

/// Score each of the four candidate phases on kick flux, harmonic change and
/// bar-level self-similarity.
///
/// Each term is normalised **across the four candidates** rather than against
/// an absolute scale, because the question is only which phase is best on this
/// track: the absolute amount of kick flux varies by two orders of magnitude
/// between an ambient record and a techno one, and a fixed weighting would let
/// whichever term happened to be large decide every track.
pub fn phase_scores(beats_ms: &[f64], curves: &NoveltyCurves, chroma: &[[f32; 12]]) -> [f64; 4] {
    let flux = per_phase(beats_ms, |idx| sample_at(&curves.kick, beats_ms[idx], curves));
    let harmony = per_phase(beats_ms, |idx| chroma_change(chroma, idx));
    let similarity = bar_similarity(beats_ms, curves);

    let (flux, harmony, similarity) = (share(flux), share(harmony), share(similarity));
    let mut out = [0.0f64; 4];
    for p in 0..DOWNBEAT_PHASES {
        out[p] = flux[p] + harmony[p] + similarity[p];
    }
    out
}

/// Mean of `value` over the beats belonging to each candidate phase.
fn per_phase(beats_ms: &[f64], value: impl Fn(usize) -> f64) -> [f64; 4] {
    let mut sums = [0.0f64; 4];
    let mut counts = [0usize; 4];
    for idx in 0..beats_ms.len() {
        let p = idx % DOWNBEAT_PHASES;
        sums[p] += value(idx);
        counts[p] += 1;
    }
    for p in 0..DOWNBEAT_PHASES {
        if counts[p] > 0 {
            sums[p] /= counts[p] as f64;
        }
    }
    sums
}

/// How self-similar consecutive bars are under each candidate phase.
///
/// A bar is four consecutive kick-novelty values. The correct phase makes every
/// bar look like the one before it; a phase shifted by two beats cuts the
/// pattern in half and the correlation drops.
fn bar_similarity(beats_ms: &[f64], curves: &NoveltyCurves) -> [f64; 4] {
    let strength: Vec<f64> = beats_ms
        .iter()
        .map(|t| sample_at(&curves.kick, *t, curves))
        .collect();
    let mut out = [0.0f64; 4];
    for p in 0..DOWNBEAT_PHASES {
        if strength.len() <= p {
            continue;
        }
        let bars: Vec<&[f64]> = strength[p..].chunks_exact(DOWNBEAT_PHASES).collect();
        if bars.len() < 2 {
            continue;
        }
        let mut acc = 0.0f64;
        for pair in bars.windows(2) {
            acc += cosine(pair[0], pair[1]);
        }
        out[p] = acc / (bars.len() - 1) as f64;
    }
    out
}

/// L1 distance between a beat's chroma and the previous beat's.
///
/// Zero when the chroma stage has not run — key analysis is a separate stage
/// and may be absent, and the phase decision has to survive without it rather
/// than refuse.
fn chroma_change(chroma: &[[f32; 12]], idx: usize) -> f64 {
    if idx == 0 || idx >= chroma.len() {
        return 0.0;
    }
    chroma[idx]
        .iter()
        .zip(&chroma[idx - 1])
        .map(|(a, b)| (a - b).abs() as f64)
        .sum()
}

fn sample_at(curve: &[f32], ms: f64, curves: &NoveltyCurves) -> f64 {
    let frame = ms * curves.fps as f64 / 1000.0;
    onset::peak_near(curve, frame, BEAT_TOLERANCE_FRAMES) as f64
}

pub fn sample_curve(curve: &[f32], times_ms: &[f64], curves: &NoveltyCurves) -> Vec<f32> {
    times_ms
        .iter()
        .map(|t| sample_at(curve, *t, curves) as f32)
        .collect()
}

/// Shifts to a non-negative floor then normalises to sum 1, so three terms with
/// unrelated units can be added.
fn share(values: [f64; 4]) -> [f64; 4] {
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let mut shifted = [0.0f64; 4];
    for p in 0..4 {
        shifted[p] = values[p] - min;
    }
    let sum: f64 = shifted.iter().sum();
    if sum <= 1e-12 {
        return [0.25; 4];
    }
    for v in shifted.iter_mut() {
        *v /= sum;
    }
    shifted
}

/// Argmax of two candidates plus a 0..1 margin between them.
pub fn pick_pair(scores: [f64; 2]) -> (usize, f64) {
    let (winner, loser) = if scores[0] >= scores[1] { (0, 1) } else { (1, 0) };
    let total = scores[0].abs() + scores[1].abs();
    let margin = if total <= 1e-12 {
        0.0
    } else {
        ((scores[winner] - scores[loser]) / total).clamp(0.0, 1.0)
    };
    (winner, margin)
}

pub fn best_agreeing_phase(beats_ms: &[f64], reference: &[f64]) -> (usize, f64) {
    let mut best = (0usize, 0.0f64);
    for p in 0..DOWNBEAT_PHASES {
        let candidate: Vec<f64> = beats_ms.iter().skip(p).step_by(DOWNBEAT_PHASES).copied().collect();
        let rate = super::beats::agreement(&candidate, reference, super::beats::AGREEMENT_TOLERANCE_MS);
        if rate > best.1 {
            best = (p, rate);
        }
    }
    best
}

fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na <= 1e-12 || nb <= 1e-12 {
        0.0
    } else {
        dot / (na * nb)
    }
}

