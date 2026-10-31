//! Tests for S2 and S3 against signals whose answer is known.

use super::chroma::{self, Pcp, CENTS_PER_BIN, N_BINS};
use super::stft::{hann, spectral_peaks, Peak, Stft, StftOpts, DEFAULT_PEAK_FLOOR_RATIO};
use super::test_signals as sig;
use super::test_unpitched as unp;
use super::tuning;
use super::vqt::{Vqt, VqtSpec};

const SR: u32 = sig::TEST_RATE;

fn top_three_pitch_classes(chroma: &[f32; 12]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..12).collect();
    idx.sort_by(|a, b| chroma[*b].partial_cmp(&chroma[*a]).unwrap());
    idx.truncate(3);
    idx.sort_unstable();
    idx
}

#[test]
fn hann_is_symmetric_and_zero_at_the_edges() {
    let w = hann(8, false);
    assert!(w[0].abs() < 1e-6);
    assert!(w[7].abs() < 1e-6);
    for i in 0..4 {
        assert!((w[i] - w[7 - i]).abs() < 1e-6);
    }
    // The periodic form is not zero at the far edge; that is the difference.
    let p = hann(8, true);
    assert!(p[7] > 0.0);
}

#[test]
fn interpolated_peaks_beat_bin_resolution() {
    // 440 Hz is deliberately between bins at this rate and window: one bin is
    // 5.4 Hz, which at 440 Hz is 21 cents, so an uninterpolated peak could not
    // pass this.
    let samples = sig::tone(69, SR, 1.0, 0.0);
    let mut stft = Stft::new(StftOpts::new(4096, 1024));
    let bin_hz = stft.bin_hz(SR);
    let mut peaks: Vec<Peak> = Vec::new();
    let mut found = f32::NAN;
    stft.process(
        &samples,
        |_| {},
        |_, mag| {
            spectral_peaks(mag, bin_hz, 300.0, 600.0, DEFAULT_PEAK_FLOOR_RATIO, &mut peaks);
            if let Some(p) = peaks.first() {
                found = p.freq_hz;
            }
        },
    );
    assert!(found.is_finite(), "no peak found for a pure 440 Hz tone");
    let cents = 1200.0 * (found / 440.0).log2();
    assert!(cents.abs() < 5.0, "440 Hz read as {found} Hz ({cents} cents off)");
}

#[test]
fn a_tone_at_a440_reads_as_a440() {
    let samples = sig::tone(69, SR, 2.0, 0.0);
    let est = tuning::estimate(&samples, SR).unwrap();
    assert!(est.cents.abs() < 4.0, "A440 read as {} cents", est.cents);
    assert!(!est.needs_correction);
    assert!(est.concentration > 0.8, "concentration {}", est.concentration);
}

#[test]
fn a_track_detuned_by_a_known_amount_reports_that_amount() {
    for detune in [-25.0f32, -12.0, 14.0, 28.0] {
        let samples = sig::progression(&sig::major_pop(), 0, SR, 4.0, detune);
        let est = tuning::estimate(&samples, SR).unwrap();
        assert!(
            (est.cents - detune).abs() < 6.0,
            "{detune} cents detune read as {} cents",
            est.cents
        );
        assert_eq!(
            est.needs_correction,
            detune.abs() > tuning::CORRECTION_THRESHOLD_CENTS,
            "needs_correction wrong at {detune} cents"
        );
        let implied = 440.0 * 2f32.powf(detune / 1200.0);
        assert!((est.reference_hz - implied).abs() < 3.0);
    }
}

#[test]
fn correcting_a_detune_puts_the_track_back_at_a440() {
    // The sign of the resample is the thing this pins: correcting a flat track
    // the wrong way doubles the error instead of removing it, and nothing
    // downstream would report that as anything but a wrong key.
    let detune = -25.0f32;
    let samples = sig::tone(69, SR, 2.0, detune);
    let before = tuning::estimate(&samples, SR).unwrap();
    assert!(before.cents < -20.0);
    let fixed = tuning::correct(&samples, SR, before.cents).unwrap();
    assert!(fixed.len() < samples.len(), "a flat track must come out shorter");
    let after = tuning::estimate(&fixed, SR).unwrap();
    assert!(
        after.cents.abs() < 5.0,
        "after correction the offset was still {} cents",
        after.cents
    );
}

#[test]
fn percussion_gives_no_tuning_estimate_worth_acting_on() {
    let samples = unp::percussion(SR, 6.0);
    let est = tuning::estimate(&samples, SR).unwrap();
    assert!(
        est.concentration < tuning::MIN_CONCENTRATION,
        "unpitched material reported concentration {}",
        est.concentration
    );
    assert!(!est.needs_correction, "percussion must never trigger a resample");
}

#[test]
fn silence_is_reported_as_a440_rather_than_as_an_error() {
    let est = tuning::estimate(&unp::silence(SR, 2.0), SR).unwrap();
    assert_eq!(est.cents, 0.0);
    assert_eq!(est.reference_hz, 440.0);
    assert!(!est.needs_correction);
    assert_eq!(est.concentration, 0.0);
    assert!(tuning::estimate(&[], SR).unwrap().concentration == 0.0);
}

#[test]
fn a_c_major_triad_puts_its_energy_on_c_e_and_g() {
    let samples = sig::progression(
        &[sig::Chord { root: 0, intervals: &sig::MAJOR_TRIAD, beats: 4 }],
        0,
        SR,
        4.0,
        0.0,
    );
    let pcp = chroma::pcp(&samples, SR, 0.0).unwrap();
    assert!(!pcp.is_empty());
    let c = pcp.fold_to_12();
    assert_eq!(
        (0..12).max_by(|a, b| c[*a].partial_cmp(&c[*b]).unwrap()),
        Some(0),
        "the tonic is not the strongest pitch class: {c:?}"
    );
    for member in [0usize, 4, 7] {
        assert!(c[member] > 0.0, "triad member {member} was gated away: {c:?}");
    }
    assert!(
        c[0] + c[4] + c[7] > 2.0 * (c.iter().sum::<f32>() - c[0] - c[4] - c[7]),
        "more energy landed outside the triad than in it: {c:?}"
    );
    // The leading tone must not outrank a member of the triad it is only a
    // sideband of. This is the assertion that fails on a `i / 3` fold, where B
    // collects the tonic's own flat-side weighting and ranks second.
    assert!(c[11] < c[7], "B ({}) outranked G ({}): {c:?}", c[11], c[7]);
}

#[test]
fn the_gate_never_empties_a_vector_that_had_energy_in_it() {
    for tonic in 0..12 {
        let samples = sig::progression(&sig::major_pop(), tonic, SR, 3.0, 0.0);
        let pcp = chroma::pcp(&samples, SR, 0.0).unwrap();
        assert!(!pcp.is_empty(), "tonic {tonic} gated to nothing");
        let live = pcp.bins.iter().filter(|v| **v > 0.0).count();
        assert!(live >= 3, "tonic {tonic} kept only {live} bins");
        assert!(live < N_BINS, "tonic {tonic}: the gate removed nothing at all");
    }
}

#[test]
fn silence_produces_an_empty_pcp_rather_than_a_key() {
    assert!(chroma::pcp(&unp::silence(SR, 2.0), SR, 0.0).unwrap().is_empty());
    assert!(chroma::pcp(&[], SR, 0.0).unwrap().is_empty());
}

#[test]
fn the_detune_shift_moves_energy_the_opposite_way_to_the_detune() {
    assert_eq!(chroma::detune_bin_shift(0.0), 0);
    assert_eq!(chroma::detune_bin_shift(CENTS_PER_BIN), -1);
    assert_eq!(chroma::detune_bin_shift(-CENTS_PER_BIN), 1);
    // Clamped at one sub-bin, which is all three bins per semitone can express.
    assert_eq!(chroma::detune_bin_shift(90.0), -1);
    assert_eq!(chroma::detune_bin_shift(f32::NAN), 0);
}

#[test]
fn rotation_wraps_and_round_trips() {
    let mut bins = [0.0f32; N_BINS];
    bins[0] = 1.0;
    bins[35] = 0.5;
    let p = Pcp { bins };
    let up = p.rotate(1);
    assert_eq!(up.bins[1], 1.0);
    assert_eq!(up.bins[0], 0.5);
    assert_eq!(up.rotate(-1).bins, p.bins);
}

#[test]
fn a_detuned_track_lands_on_the_same_bins_once_the_shift_is_applied() {
    // A third of a semitone flat is exactly one sub-bin: without the shift the
    // tonic's energy folds into the semitone below.
    let detune = -CENTS_PER_BIN;
    let tuned = sig::progression(&sig::major_pop(), 0, SR, 4.0, 0.0);
    let flat = sig::progression(&sig::major_pop(), 0, SR, 4.0, detune);
    let reference = chroma::pcp(&tuned, SR, 0.0).unwrap().fold_to_12();
    let uncorrected = chroma::pcp(&flat, SR, 0.0).unwrap().fold_to_12();
    let corrected = chroma::pcp(&flat, SR, detune).unwrap().fold_to_12();
    assert_eq!(top_three_pitch_classes(&reference), vec![0, 4, 7]);
    assert_eq!(top_three_pitch_classes(&corrected), vec![0, 4, 7]);
    assert_ne!(
        top_three_pitch_classes(&uncorrected),
        top_three_pitch_classes(&corrected),
        "the shift made no difference, so it is not being applied"
    );
}

#[test]
fn the_vqt_folds_onto_the_right_pitch_class() {
    let n_fft = 8192;
    let vqt = Vqt::new(VqtSpec::CHROMA_7_OCTAVES, SR, n_fft);
    assert_eq!(vqt.n_bins(), 84);
    assert!((vqt.center_hz(0) - 27.5).abs() < 0.01);
    assert!((vqt.center_hz(12) - 55.0).abs() < 0.05);
    let samples = sig::tone(69, SR, 1.0, 0.0);
    let mut stft = Stft::new(StftOpts::new(n_fft, 2048));
    let mut bins = vec![0.0f32; vqt.n_bins()];
    stft.process(&samples, |_| {}, |_, mag| vqt.accumulate(mag, &mut bins));
    let chroma = vqt.fold_to_chroma(&bins);
    let argmax = (0..12).max_by(|a, b| chroma[*a].partial_cmp(&chroma[*b]).unwrap()).unwrap();
    assert_eq!(argmax, 9, "a 440 Hz tone must fold onto A");
}

#[test]
fn beat_chroma_has_one_row_per_beat() {
    let samples = sig::progression(&sig::major_pop(), 0, SR, 4.0, 0.0);
    let beats: Vec<f64> = (0..8).map(|i| i as f64 * 500.0).collect();
    let rows = chroma::chroma_per_beat(&samples, SR, &beats).unwrap();
    assert_eq!(rows.len(), beats.len());
    for row in &rows {
        let sum: f32 = row.iter().sum();
        assert!(sum == 0.0 || (sum - 1.0).abs() < 1e-3, "row sum {sum}");
    }
    assert!(chroma::chroma_per_beat(&samples, SR, &[]).unwrap().is_empty());
}
