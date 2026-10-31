//! S8 — beat grid, strong beats, downbeat phase, phrase period.
//!
//! **Build the cue pipeline on the strong-beat grid, not the bar grid.** Zehren
//! et al. report they could not reliably distinguish the downbeat from beat 3 in
//! EDM even using a neural tracker, and fell back to strong beats for exactly
//! that reason. A 2-beat grid is reliable; a 4-beat grid is not. Treat a
//! confident 4-beat downbeat as a bonus. Designing on the reliable grid is what
//! stops a systematic half-bar offset leaking into every cue in the product.
//!
//! The phase search is written to match that: the strong-beat parity is chosen
//! first, from two candidates, and the downbeat is then chosen from the two
//! phases consistent with it. The two decisions carry **separate** confidences,
//! so a track can have a trustworthy strong-beat grid and an admitted shrug
//! about which of its two halves is bar one.

use crate::analysis::rhythm::downbeat::{self, phase_scores, pick_pair};
use crate::analysis::rhythm::onset::NoveltyCurves;
use crate::analysis::rhythm::phrase;
use crate::analysis::rhythm::tempo::BeatGrid;
use crate::error::Result;

pub use crate::analysis::rhythm::downbeat::DOWNBEAT_PHASES;
pub use crate::analysis::rhythm::phrase::PHRASE_CANDIDATE_PHASES;

/// Agreement tolerance when comparing two grids, including against the gated
/// arbiter.
pub const AGREEMENT_TOLERANCE_MS: f64 = 70.0;

pub struct BeatAnalysis {
    pub beats_ms: Vec<f64>,
    /// Beats 1 and 3. Everything downstream indexes into this, not `beats_ms`.
    pub strong_beats_ms: Vec<f64>,
    pub beats_per_bar: u8,
    /// 0..3. Low confidence means cues snap to a strong beat instead.
    pub downbeat_phase: u8,
    pub downbeat_confidence: f64,
    /// 0 or 1: which parity of `beats_ms` the strong-beat grid starts on. Its
    /// own confidence, because this is the reliable half of the decision.
    pub strong_beat_phase: u8,
    pub strong_beat_confidence: f64,
    /// 4 bars, promoted to 8/16/32 by novelty strength at each level.
    pub phrase_bars: u8,
    pub phrase_phase: u8,
    pub phrase_confidence: f64,
}

impl BeatAnalysis {
    pub fn downbeats_ms(&self) -> Vec<f64> {
        let phase = self.downbeat_phase as usize;
        let per_bar = (self.beats_per_bar as usize).max(1);
        self.beats_ms
            .iter()
            .enumerate()
            .filter(|(i, _)| i % per_bar == phase)
            .map(|(_, t)| *t)
            .collect()
    }

    /// Bar number at a time, counted from the first downbeat.
    ///
    /// Signed and extrapolated rather than clamped: a cue the user drags before
    /// bar one still needs an answer, and a `None` there would be read as "no
    /// grid" by the exporters.
    pub fn bar_index_at(&self, ms: f64) -> Option<i32> {
        let downbeats = self.downbeats_ms();
        let first = *downbeats.first()?;
        let bar_ms = self.bar_duration_ms()?;
        Some(((ms - first) / bar_ms).floor() as i32)
    }

    pub fn bar_duration_ms(&self) -> Option<f64> {
        let beat = self.beat_duration_ms()?;
        Some(beat * self.beats_per_bar.max(1) as f64)
    }

    pub fn beat_duration_ms(&self) -> Option<f64> {
        if self.beats_ms.len() < 2 {
            return None;
        }
        let span = self.beats_ms.last()? - self.beats_ms.first()?;
        Some(span / (self.beats_ms.len() - 1) as f64)
    }
}

/// S8 on the classical path.
pub fn analyze(
    grid: &BeatGrid,
    curves: &NoveltyCurves,
    beat_chroma: &[[f32; 12]],
) -> Result<BeatAnalysis> {
    analyze_with_arbiter(grid, curves, beat_chroma, None)
}

/// S8, optionally arbitrated by an external downbeat sequence.
///
/// The arbiter is passed as times rather than as a model, so this module never
/// touches audio and the neural path stays a caller's problem. When one is
/// supplied the phase it agrees with wins outright and the agreement rate
/// becomes the confidence — which is also the measurement ENGINE.md §2's gate
/// needs before 190 MB is bundled.
pub fn analyze_with_arbiter(
    grid: &BeatGrid,
    curves: &NoveltyCurves,
    beat_chroma: &[[f32; 12]],
    arbiter_downbeats: Option<&[f64]>,
) -> Result<BeatAnalysis> {
    let beats_ms = grid.beats_ms.clone();
    let scores = phase_scores(&beats_ms, curves, beat_chroma);

    let (strong_phase, strong_confidence) = pick_pair([
        scores[0] + scores[2],
        scores[1] + scores[3],
    ]);
    let (which, downbeat_confidence) =
        pick_pair([scores[strong_phase], scores[strong_phase + 2]]);
    let mut downbeat_phase = strong_phase + which * 2;
    let mut downbeat_confidence = downbeat_confidence;

    if let Some(reference) = arbiter_downbeats {
        let (phase, rate) = downbeat::best_agreeing_phase(&beats_ms, reference);
        downbeat_phase = phase;
        downbeat_confidence = rate;
    }

    let strong_beats_ms: Vec<f64> = beats_ms
        .iter()
        .skip(downbeat_phase % 2)
        .step_by(2)
        .copied()
        .collect();
    let sampled = downbeat::sample_curve(&curves.full, &strong_beats_ms, curves);
    let phrase = phrase::estimate(&sampled);

    Ok(BeatAnalysis {
        beats_ms,
        strong_beats_ms,
        beats_per_bar: DOWNBEAT_PHASES as u8,
        downbeat_phase: downbeat_phase as u8,
        downbeat_confidence,
        strong_beat_phase: strong_phase as u8,
        strong_beat_confidence: strong_confidence,
        phrase_bars: phrase.bars,
        phrase_phase: phrase.phase,
        phrase_confidence: phrase.confidence,
    })
}

/// Fraction of `a` whose nearest `b` is within `tolerance_ms`.
///
/// This is the measurement the Beat This! gate turns on: run it over a
/// 200-track EDM fixture set, and if the classical phase agrees with the model
/// on more than 90% of tracks, the model is dropped from v1 and 190 MB is not
/// bundled.
pub fn agreement(a: &[f64], b: &[f64], tolerance_ms: f64) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut j = 0usize;
    let mut hits = 0usize;
    for t in a {
        while j + 1 < b.len() && (b[j + 1] - t).abs() <= (b[j] - t).abs() {
            j += 1;
        }
        if (b[j] - t).abs() <= tolerance_ms {
            hits += 1;
        }
    }
    hits as f64 / a.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rhythm::{fixtures, onset, tempo};

    fn analyse(f: &fixtures::Fixture) -> BeatAnalysis {
        let c = onset::novelty(&f.samples, fixtures::SAMPLE_RATE).unwrap();
        let (_, grid) = tempo::estimate(&c).unwrap();
        analyze(&grid, &c, &[]).unwrap()
    }

    #[test]
    fn the_strong_beat_grid_is_every_other_beat() {
        let b = analyse(&fixtures::click_track(128.0, 24.0, fixtures::Groove::FourToTheFloor));
        assert!(b.beats_ms.len() > 40, "{} beats", b.beats_ms.len());
        let skipped = (b.downbeat_phase % 2) as usize;
        let expected = (b.beats_ms.len() - skipped + 1) / 2;
        assert_eq!(b.strong_beats_ms.len(), expected);
        assert_eq!(b.beats_per_bar, 4);
        assert!(b.downbeat_phase < 4);
    }

    #[test]
    fn downbeats_are_one_bar_apart() {
        let b = analyse(&fixtures::click_track(128.0, 24.0, fixtures::Groove::FourToTheFloor));
        let downbeats = b.downbeats_ms();
        let bar = b.bar_duration_ms().unwrap();
        assert!((bar - 4.0 * 60_000.0 / 128.0).abs() < 20.0, "bar {bar} ms");
        for pair in downbeats.windows(2) {
            assert!((pair[1] - pair[0] - bar).abs() < 25.0);
        }
    }

    #[test]
    fn bar_index_counts_from_the_first_downbeat_and_extrapolates_backwards() {
        let b = analyse(&fixtures::click_track(128.0, 24.0, fixtures::Groove::FourToTheFloor));
        let first = b.downbeats_ms()[0];
        let bar = b.bar_duration_ms().unwrap();
        assert_eq!(b.bar_index_at(first + 1.0), Some(0));
        assert_eq!(b.bar_index_at(first + bar * 3.5), Some(3));
        assert_eq!(b.bar_index_at(first - bar * 0.5), Some(-1));
    }

    #[test]
    fn an_arbiter_overrules_the_classical_phase() {
        let f = fixtures::click_track(128.0, 24.0, fixtures::Groove::FourToTheFloor);
        let c = onset::novelty(&f.samples, fixtures::SAMPLE_RATE).unwrap();
        let (_, grid) = tempo::estimate(&c).unwrap();
        // A reference that marks beat index 1 of every bar.
        let reference: Vec<f64> = grid.beats_ms.iter().skip(1).step_by(4).copied().collect();
        let b = analyze_with_arbiter(&grid, &c, &[], Some(&reference)).unwrap();
        assert_eq!(b.downbeat_phase, 1);
        assert!(b.downbeat_confidence > 0.9, "rate {}", b.downbeat_confidence);
    }

    #[test]
    fn agreement_is_a_rate_and_empty_inputs_are_zero() {
        let a = vec![0.0, 1000.0, 2000.0, 3000.0];
        assert_eq!(agreement(&a, &a, AGREEMENT_TOLERANCE_MS), 1.0);
        let shifted: Vec<f64> = a.iter().map(|t| t + 50.0).collect();
        assert_eq!(agreement(&a, &shifted, AGREEMENT_TOLERANCE_MS), 1.0);
        let far: Vec<f64> = a.iter().map(|t| t + 200.0).collect();
        assert_eq!(agreement(&a, &far, AGREEMENT_TOLERANCE_MS), 0.0);
        assert_eq!(agreement(&[], &a, 70.0), 0.0);
        assert_eq!(agreement(&a, &[], 70.0), 0.0);
    }

    #[test]
    fn an_empty_grid_is_not_a_panic() {
        let c = onset::novelty(&vec![0.0f32; 44_100], fixtures::SAMPLE_RATE).unwrap();
        let grid = BeatGrid { beats_ms: Vec::new(), bpm: 0.0, residual_ms: f64::INFINITY, coverage: 0.0 };
        let b = analyze(&grid, &c, &[]).unwrap();
        assert!(b.downbeats_ms().is_empty());
        assert_eq!(b.bar_index_at(1000.0), None);
        assert_eq!(b.phrase_bars, 4);
    }
}
