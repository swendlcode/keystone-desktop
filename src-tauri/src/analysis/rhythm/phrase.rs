//! S8, part two: the phrase period and phase.
//!
//! DJ structure runs on 8, 16 and 32-bar phrases, so for the cue pipeline the
//! phrase period matters as much as the beat. It is also the weakest link:
//! Zehren reports roughly 90% reliability on EDM, and a wrong period accounted
//! for 45% of his false negatives. Poor on non-multiple-of-4 intros, on 3/4 and
//! 6/8, and on everything that is not electronic.

/// 4 bars of 4/4 is 16 beats, which is 8 strong beats — hence eight candidate
/// phases over the strong-beat grid.
pub const PHRASE_CANDIDATE_PHASES: usize = 8;

/// A longer phrase level has to beat the 4-bar baseline by this much before it
/// is promoted. Without a margin every track promotes to 32 bars, because a
/// sparser sample of the same boundaries is noisier and sometimes luckier.
const PROMOTE_MARGIN: f64 = 1.15;

#[derive(Debug, Clone, Copy)]
pub struct PhraseEstimate {
    /// 4, 8, 16 or 32.
    pub bars: u8,
    /// Offset in strong beats from the first strong beat.
    pub phase: u8,
    pub confidence: f64,
}

impl Default for PhraseEstimate {
    fn default() -> Self {
        Self { bars: 4, phase: 0, confidence: 0.0 }
    }
}

/// From the eight candidate strong-beat offsets, the one maximising the **RMS**
/// of novelty sampled 4 bars apart.
///
/// RMS, not a count of peaks: Zehren's modification of Vande Veire. A count
/// rewards a phase that happens to sit on many small bumps; RMS rewards the
/// phase that sits on the few large ones, which is what a phrase boundary is.
pub fn estimate(strong_beat_novelty: &[f32]) -> PhraseEstimate {
    let n = strong_beat_novelty.len();
    if n < PHRASE_CANDIDATE_PHASES * 2 {
        return PhraseEstimate::default();
    }
    let scores: Vec<f64> = (0..PHRASE_CANDIDATE_PHASES)
        .map(|phase| rms(strong_beat_novelty, phase, PHRASE_CANDIDATE_PHASES))
        .collect();

    let (phase, best) = argmax(&scores);
    let runner_up = scores
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != phase)
        .map(|(_, s)| *s)
        .fold(0.0f64, f64::max);
    let confidence = if best <= 1e-9 { 0.0 } else { (1.0 - runner_up / best).clamp(0.0, 1.0) };

    let (bars, phase) = promote(strong_beat_novelty, phase);
    PhraseEstimate { bars, phase: phase as u8, confidence }
}

/// Promotes a 4-bar boundary set to 8, 16 or 32 bars.
///
/// Each step splits the current boundary set in two and asks whether one half
/// is stronger than the other. Comparing a level against the 4-bar baseline
/// instead does not work: every level is a subset of the 4-bar boundaries, so a
/// track with clean 16-bar structure clears the margin at 32 bars too and
/// promotes all the way regardless. Asking "do these two halves differ" stops
/// where the structure stops saying anything.
fn promote(curve: &[f32], base_phase: usize) -> (u8, usize) {
    let mut bars = 4u8;
    let mut phase = base_phase;
    let mut stride = PHRASE_CANDIDATE_PHASES;
    for level in [8u8, 16, 32] {
        let next = stride * 2;
        if curve.len() < next * 2 {
            break;
        }
        let here = rms(curve, phase, next);
        let sibling = rms(curve, phase + stride, next);
        let (strong, weak) = if here >= sibling { (here, sibling) } else { (sibling, here) };
        if weak <= 1e-9 || strong / weak < PROMOTE_MARGIN {
            break;
        }
        phase = if here >= sibling { phase } else { phase + stride };
        stride = next;
        bars = level;
    }
    (bars, phase)
}

fn rms(curve: &[f32], phase: usize, stride: usize) -> f64 {
    if stride == 0 || phase >= curve.len() {
        return 0.0;
    }
    let mut acc = 0.0f64;
    let mut n = 0usize;
    let mut i = phase;
    while i < curve.len() {
        let v = curve[i] as f64;
        acc += v * v;
        n += 1;
        i += stride;
    }
    if n == 0 {
        0.0
    } else {
        (acc / n as f64).sqrt()
    }
}

fn argmax(values: &[f64]) -> (usize, f64) {
    let mut best = (0usize, f64::NEG_INFINITY);
    for (i, v) in values.iter().enumerate() {
        if *v > best.1 {
            best = (i, *v);
        }
    }
    if best.1.is_finite() {
        best
    } else {
        (0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strong-beat novelty with a spike every `period` strong beats at `phase`.
    fn spiked(len: usize, period: usize, phase: usize, height: f32) -> Vec<f32> {
        (0..len)
            .map(|i| if i >= phase && (i - phase) % period == 0 { height } else { 0.2 })
            .collect()
    }

    #[test]
    fn the_phase_is_the_offset_the_boundaries_actually_sit_on() {
        let curve = spiked(128, 8, 3, 4.0);
        let p = estimate(&curve);
        assert_eq!(p.phase, 3);
        assert_eq!(p.bars, 4);
        assert!(p.confidence > 0.5, "confidence {}", p.confidence);
    }

    #[test]
    fn a_sixteen_bar_structure_is_promoted_rather_than_reported_as_four() {
        // Spikes every 4 bars, four times taller every 16. A 4-bar answer is
        // not wrong, but the cue pipeline places drops on 16-bar lines, so the
        // longer level has to be found.
        let mut curve = spiked(256, 8, 0, 2.0);
        for i in (0..curve.len()).step_by(32) {
            curve[i] = 8.0;
        }
        let p = estimate(&curve);
        assert_eq!(p.bars, 16, "promoted to {} bars", p.bars);
        assert_eq!(p.phase % 8, 0);
    }

    #[test]
    fn a_flat_curve_falls_back_to_four_bars_with_no_confidence() {
        let p = estimate(&vec![1.0f32; 200]);
        assert_eq!(p.bars, 4);
        assert!(p.confidence < 0.05, "confidence {}", p.confidence);
    }

    #[test]
    fn too_short_to_hold_a_phrase_is_not_a_panic() {
        assert_eq!(estimate(&[]).bars, 4);
        assert_eq!(estimate(&[1.0, 2.0, 3.0]).confidence, 0.0);
    }
}
