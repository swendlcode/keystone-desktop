//! S7 — tempo, the octave rule, folding, and the two-decimal refit.
//!
//! The four steps are: comb with 1/k weighting, the capped log-Gaussian prior,
//! beat-level kick and hat evidence, then fold — **last**. Folding is a display
//! transform, never a way of choosing: pick a candidate because it lands in
//! range and a correct 170 BPM drum and bass track becomes indistinguishable
//! from a wrong 85 BPM estimate in the report, which hides the one number that
//! says the detector is broken.

use crate::analysis::rhythm::grid;
use crate::analysis::rhythm::octave::{self, OctaveDecision};
use crate::analysis::rhythm::onset::NoveltyCurves;
use crate::error::Result;

pub use crate::analysis::rhythm::octave::{
    perceptual_prior, resolve_octave, PRIOR_CENTRE_BPM, PRIOR_SALIENCE_CAP, PRIOR_SIGMA_OCTAVES,
};
pub use crate::analysis::rhythm::tempogram::{
    comb, densest_window, tempogram, Tempogram, BPM_GRID_STEP, BPM_MAX, BPM_MIN, COMB_HARMONICS,
    MAX_LAG_FRAMES, TEMPOGRAM_WINDOW_SECONDS,
};

pub use crate::analysis::rhythm::grid::refit_bpm;

/// Two decimals are only reported when the grid residual sd is under this.
///
/// Integer lag quantisation at 100 fps gives a BPM step of 1.32 at 90, 2.35 at
/// 120, 2.66 at 128 and 5.04 at 174, and 200 fps only halves it — so two
/// decimals **cannot** come from the tempogram at all. They come from the
/// least-squares refit, and without this gate the spec's two-decimal option is
/// false precision on exactly the material that needs the honesty most.
pub const TWO_DECIMAL_RESIDUAL_MS: f64 = 15.0;

/// The refit is trusted over the tempogram only while the two agree this
/// closely. Further apart means the grid wandered off onto some other
/// periodicity, and the tempogram's answer is the one with evidence behind it.
const REFIT_AGREEMENT: f64 = 0.05;

/// Default display range, from SPEC.md §3. Anything outside is doubled or
/// halved rather than reported wrong.
pub const DEFAULT_RANGE: (f64, f64) = (79.0, 192.0);

#[derive(Debug, Clone)]
pub struct TempoEstimate {
    pub raw_bpm: f64,
    pub octave_shift: i32,
    pub confidence: f64,
    pub residual_ms: f64,
    pub variable: bool,
}

#[derive(Debug, Clone)]
pub struct BeatGrid {
    pub beats_ms: Vec<f64>,
    pub bpm: f64,
    pub residual_ms: f64,
    /// Fraction of the track covered by a consistent grid. Feeds the confidence
    /// alongside the salience ratio.
    pub coverage: f64,
}

/// S7 end to end.
pub fn estimate(curves: &NoveltyCurves) -> Result<(TempoEstimate, BeatGrid)> {
    let decision = octave::resolve(&tempogram(curves)?, curves)?;
    let fps = curves.fps as f64;
    let period = 60.0 * fps / decision.bpm;
    let beats_ms = grid::track_beats(&curves.full, fps, period);

    let (fit_bpm, residual_ms) = grid::refit_bpm(&beats_ms);
    let coverage = grid::coverage(&beats_ms, curves.duration_ms());
    let agrees = fit_bpm > 0.0 && (fit_bpm / decision.bpm - 1.0).abs() < REFIT_AGREEMENT;
    let raw_bpm = if agrees { fit_bpm } else { decision.bpm };
    let residual_ms = if agrees { residual_ms } else { f64::INFINITY };

    let confidence = confidence(&decision, residual_ms, coverage, agrees);
    let (_, octave_shift) = fold_into_range(raw_bpm, DEFAULT_RANGE.0, DEFAULT_RANGE.1);

    Ok((
        TempoEstimate {
            raw_bpm,
            octave_shift,
            confidence,
            residual_ms,
            // Written as a negation so a non-finite residual — no grid at
            // all — counts as variable rather than slipping under the gate.
            variable: !(residual_ms < TWO_DECIMAL_RESIDUAL_MS),
        },
        BeatGrid { beats_ms, bpm: raw_bpm, residual_ms, coverage },
    ))
}

/// The salience margin over a competing grid, the grid residual, the fraction
/// of the track a consistent grid covers, and how settled the octave is.
///
/// Combined as the **minimum**, not the product. Each factor is a separate way
/// of being wrong and any one of them being bad is enough — a clean tempogram
/// over a grid that fits a third of the file is not a confident answer. A
/// product would be worse than the minimum in a different way: four factors
/// each honestly at 0.9 multiply to 0.66, so a track with nothing wrong with it
/// would read as a coin toss, and the number would stop meaning anything.
fn confidence(d: &OctaveDecision, residual_ms: f64, coverage: f64, agrees: bool) -> f64 {
    if !agrees {
        // The grid and the tempogram disagree about the period. Something in
        // this file is not a steady 4/4 pulse; ENGINE.md §8 says such material
        // gets a low-confidence result rather than a confident wrong answer.
        return 0.1 * d.salience_ratio;
    }
    let residual_factor = if residual_ms.is_finite() {
        (1.0 - residual_ms / (4.0 * TWO_DECIMAL_RESIDUAL_MS)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Contested means the kicks and the hats disagree about the octave. It
    // caps the answer rather than scaling it: the tempo may well be right, but
    // the user is owed the doubt.
    let octave_factor = if d.contested { 0.35 } else { 0.7 + 0.3 * d.evidence };
    [d.salience_ratio, residual_factor, coverage, octave_factor]
        .into_iter()
        .fold(1.0f64, f64::min)
        .clamp(0.0, 1.0)
}

/// Repeated x2 / /2 until the tempo lands in `[lo, hi]`.
pub fn fold_into_range(bpm: f64, lo: f64, hi: f64) -> (f64, i32) {
    if !bpm.is_finite() || bpm <= 0.0 || !(lo > 0.0) || !(hi >= lo) {
        return (bpm, 0);
    }
    let mut value = bpm;
    let mut shift = 0i32;
    // Bounded so a range narrower than one octave cannot spin.
    const MAX_STEPS: i32 = 8;
    while value < lo && shift > -MAX_STEPS {
        value *= 2.0;
        shift -= 1;
    }
    while value > hi && shift < MAX_STEPS {
        value /= 2.0;
        shift += 1;
    }
    (value, shift)
}

/// Re-folds a stored raw tempo for display. Pure, and deliberately so: changing
/// the Tempo range setting runs this over the database and never reads a file.
pub fn display_bpm(raw_bpm: f64, lo: f64, hi: f64) -> f64 {
    fold_into_range(raw_bpm, lo, hi).0
}

/// Formats for the table. Drops to one decimal when the grid residual does not
/// support two, whatever the setting says.
pub fn format_bpm(bpm: f64, residual_ms: Option<f64>, decimals: u8) -> String {
    if !bpm.is_finite() || bpm <= 0.0 {
        return "—".to_string();
    }
    let supported = residual_ms.is_some_and(|r| r < TWO_DECIMAL_RESIDUAL_MS);
    let places = if decimals >= 2 && !supported { 1 } else { decimals.min(2) };
    format!("{bpm:.*}", places as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rhythm::{fixtures, onset};

    fn run(f: &fixtures::Fixture) -> (TempoEstimate, BeatGrid) {
        let c = onset::novelty(&f.samples, fixtures::SAMPLE_RATE).unwrap();
        estimate(&c).unwrap()
    }

    fn at(bpm: f64, groove: fixtures::Groove) -> (TempoEstimate, BeatGrid) {
        run(&fixtures::click_track(bpm, 30.0, groove))
    }

    #[test]
    fn folding_reaches_the_range_and_reports_the_shift() {
        // A 174 BPM DnB track is already inside the default range and must not
        // be touched.
        assert_eq!(fold_into_range(174.0, 79.0, 192.0), (174.0, 0));
        assert_eq!(fold_into_range(70.0, 79.0, 192.0), (140.0, -1));
        assert_eq!(fold_into_range(256.0, 79.0, 192.0), (128.0, 1));
        assert_eq!(fold_into_range(35.0, 79.0, 192.0), (140.0, -2));
    }

    #[test]
    fn folding_is_the_inverse_of_the_shift_it_reports() {
        // The shift is what lets the table re-fold from tempo_raw with no
        // re-analysis, so it has to reconstruct the original exactly.
        for raw in [60.0, 70.0, 128.0, 174.0, 200.0, 256.0] {
            let (folded, shift) = fold_into_range(raw, 79.0, 192.0);
            let back = folded * 2f64.powi(shift);
            assert!((back - raw).abs() < 1e-9, "{raw} did not reconstruct");
        }
    }

    #[test]
    fn a_degenerate_range_cannot_spin() {
        assert_eq!(fold_into_range(128.0, 0.0, f64::INFINITY), (128.0, 0));
        assert_eq!(fold_into_range(f64::NAN, 79.0, 192.0).1, 0);
        assert_eq!(fold_into_range(0.0, 79.0, 192.0), (0.0, 0));
    }

    #[test]
    fn two_decimals_are_withheld_above_the_residual_gate() {
        assert_eq!(format_bpm(128.1437, Some(9.0), 2), "128.14");
        assert_eq!(format_bpm(128.1437, Some(22.0), 2), "128.1");
        assert_eq!(format_bpm(128.1437, None, 2), "128.1");
        assert_eq!(format_bpm(128.1437, Some(9.0), 0), "128");
        assert_eq!(format_bpm(128.1437, Some(9.0), 1), "128.1");
        assert_eq!(format_bpm(f64::NAN, Some(1.0), 2), "—");
    }

    #[test]
    fn a_128_bpm_house_loop_lands_on_128_with_a_tight_grid() {
        let (t, g) = at(128.0, fixtures::Groove::FourToTheFloor);
        assert!((t.raw_bpm - 128.0).abs() < 0.5, "bpm {}", t.raw_bpm);
        assert!(!t.variable);
        assert!(t.residual_ms < TWO_DECIMAL_RESIDUAL_MS, "residual {}", t.residual_ms);
        assert!(t.confidence > 0.5, "confidence {}", t.confidence);
        assert!(g.coverage > 0.8, "coverage {}", g.coverage);
    }

    #[test]
    fn a_174_bpm_drum_and_bass_loop_is_not_halved() {
        let (t, _) = at(174.0, fixtures::Groove::FourToTheFloor);
        assert!((t.raw_bpm - 174.0).abs() < 1.0, "bpm {}", t.raw_bpm);
        assert_eq!(t.octave_shift, 0, "174 is inside the default range");
    }

    #[test]
    fn a_70_bpm_loop_is_detected_at_70_and_displayed_at_140() {
        // Fold last: the stored tempo is what was detected, and the doubling is
        // a display decision the Tempo setting can change without re-analysis.
        let (t, _) = at(70.0, fixtures::Groove::KickOnly);
        assert!((t.raw_bpm - 70.0).abs() < 1.0, "bpm {}", t.raw_bpm);
        assert_eq!(t.octave_shift, -1);
        assert!((display_bpm(t.raw_bpm, 79.0, 192.0) - 140.0).abs() < 2.0);
    }

    #[test]
    fn half_time_resolves_to_the_same_display_tempo_either_way() {
        // Kick on 1 and 3 at 140. Whether the engine reads 70 or 140, the user
        // sees 140 after folding — but the confidence must admit the doubt.
        let (t, _) = at(140.0, fixtures::Groove::HalfTime);
        let shown = display_bpm(t.raw_bpm, 79.0, 192.0);
        assert!((shown - 140.0).abs() < 2.0, "displayed {shown}");
        assert!(t.confidence < 0.6, "confidence {} is too sure", t.confidence);
    }

    #[test]
    fn a_swung_groove_reports_low_confidence_rather_than_a_wrong_answer() {
        // Named in ENGINE.md §7.2: swing breaks the autocorrelation. The
        // requirement is an admitted doubt, not a correct number.
        let (t, _) = at(128.0, fixtures::Groove::Swung);
        let shown = display_bpm(t.raw_bpm, 79.0, 192.0);
        let plausible = [64.0, 85.33, 128.0, 170.67]
            .iter()
            .any(|b| (shown - display_bpm(*b, 79.0, 192.0)).abs() < 4.0);
        assert!(plausible || t.confidence < 0.5, "{shown} at {}", t.confidence);
    }

    #[test]
    fn the_sixty_four_one_twenty_eight_one_ninety_two_triple_resolves_each_way() {
        // The three readings of the same bar. 64 and 128 share every
        // autocorrelation peak and 192 is the dotted reading of 128, so the
        // comb alone cannot separate them; step 3 and the fold do.
        let (slow, _) = at(64.0, fixtures::Groove::KickOnly);
        assert!((slow.raw_bpm - 64.0).abs() < 1.0, "64 read as {}", slow.raw_bpm);
        assert!((display_bpm(slow.raw_bpm, 79.0, 192.0) - 128.0).abs() < 2.0);

        let (mid, _) = at(128.0, fixtures::Groove::AccentEveryTwo);
        assert!((mid.raw_bpm - 128.0).abs() < 1.0, "128 read as {}", mid.raw_bpm);

        // 192 sits exactly on the top of the default range and must not fold.
        let (fast, _) = at(192.0, fixtures::Groove::FourToTheFloor);
        assert!((fast.raw_bpm - 192.0).abs() < 2.0, "192 read as {}", fast.raw_bpm);
        assert_eq!(fast.octave_shift, 0);
    }

    #[test]
    fn a_tempo_change_mid_file_is_flagged_variable_and_loses_two_decimals() {
        let (t, _) = run(&fixtures::tempo_change(120.0, 140.0, 15.0));
        assert!(t.variable, "residual {} passed the gate", t.residual_ms);
        assert!(t.residual_ms >= TWO_DECIMAL_RESIDUAL_MS);
        assert_eq!(format_bpm(t.raw_bpm, Some(t.residual_ms), 2).matches('.').count(), 1);
        assert!(t.confidence < 0.8, "confidence {}", t.confidence);
    }

    #[test]
    fn silence_is_not_a_tempo() {
        let c = onset::novelty(&vec![0.0f32; 44_100 * 4], fixtures::SAMPLE_RATE).unwrap();
        let (t, _) = estimate(&c).unwrap();
        assert!(t.confidence < 0.3, "confidence {}", t.confidence);
    }
}
