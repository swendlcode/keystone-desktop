//! S3 — the 36-bin HPCP and the beat-synchronous 12-bin chroma.
//!
//! The stage lives in [`crate::analysis::spectral::chroma`]. This module is the
//! name the rest of the engine reaches it by and nothing else. See
//! [`crate::analysis::tuning`] for why S2 and S3 are aliases rather than a
//! second implementation: both were duplicate `todo!()` copies shadowing the
//! finished stage, the same way S6-S8 once were.
//!
//! S3 is the worse of the two to duplicate. Everything above it — both key
//! voters, the fusion, the eval harness — is written against
//! `spectral::chroma::Pcp`, and this copy's `Pcp` had neither `is_empty` nor
//! `rotate`, so it could not express "no answer" and could not apply the detune
//! at all. The copy's `fold_to_12` also grouped sub-bins as `i / 3` rather than
//! the centred `(i + 1) / 3`: that is a third of a semitone of misalignment,
//! and with the 1-semitone weighting window spreading each peak over its
//! neighbours it is enough to rank B above G on a C major triad. A silent
//! half-semitone error in the layer that decides the product's key badge is the
//! exact failure this file now cannot reintroduce.
//!
//! One implementation, two paths to it.

pub use crate::analysis::spectral::chroma::*;

#[cfg(test)]
mod tests {
    //! Synthesised tones, reached through this path on purpose. A single note
    //! must land one dominant bin, a triad three, and a note detuned by a known
    //! amount must land on the right bin once the tuning stage has had its say.

    use super::{chroma_per_beat, detune_bin_shift, pcp, BINS_PER_SEMITONE, CENTS_PER_BIN};
    use crate::analysis::spectral::{test_signals as sig, tuning};

    const SR: u32 = sig::TEST_RATE;

    /// A bare sinusoid rather than `sig::tone`, whose six partials put real
    /// energy on the fifth and the major third. "One dominant bin" is only a
    /// meaningful claim about a signal that contains one pitch.
    fn sine(note: i32, seconds: f32, detune_cents: f32) -> Vec<f32> {
        let n = (SR as f32 * seconds) as usize;
        let mut osc = sig::Osc::new(sig::midi_hz(note, detune_cents), SR);
        (0..n).map(|_| 0.3 * osc.next()).collect()
    }

    fn argmax(v: &[f32]) -> usize {
        (0..v.len()).max_by(|a, b| v[*a].partial_cmp(&v[*b]).unwrap()).unwrap()
    }

    fn top_three(chroma: &[f32; 12]) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..12).collect();
        idx.sort_by(|a, b| chroma[*b].partial_cmp(&chroma[*a]).unwrap());
        idx.truncate(3);
        idx.sort_unstable();
        idx
    }

    #[test]
    fn a_single_note_produces_one_dominant_bin() {
        // A440 is pitch class 9, so its centre sub-bin is 3 * 9 = 27.
        let p = pcp(&sine(69, 2.0, 0.0), SR, 0.0).unwrap();
        assert!(!p.is_empty());
        assert_eq!(argmax(&p.bins), 9 * BINS_PER_SEMITONE, "bins {:?}", p.bins);

        let c = p.fold_to_12();
        assert_eq!(argmax(&c), 9, "a 440 Hz sine did not fold onto A: {c:?}");
        let total: f32 = c.iter().sum();
        assert!(c[9] > 0.7 * total, "A held only {} of {total}: {c:?}", c[9]);
        // The weighting window reaches a semitone either side, so the
        // neighbours are not zero; they must not be comparable.
        assert!(c[9] > 3.0 * c[8].max(c[10]), "the neighbours rival the note: {c:?}");
    }

    #[test]
    fn a_major_triad_produces_three() {
        let samples = sig::progression(
            &[sig::Chord { root: 0, intervals: &sig::MAJOR_TRIAD, beats: 4 }],
            0,
            SR,
            4.0,
            0.0,
        );
        let c = pcp(&samples, SR, 0.0).unwrap().fold_to_12();
        assert_eq!(top_three(&c), vec![0, 4, 7], "C major triad folded to {c:?}");
        let members = c[0] + c[4] + c[7];
        assert!(
            members > 2.0 * (c.iter().sum::<f32>() - members),
            "more energy outside the triad than in it: {c:?}"
        );
    }

    #[test]
    fn a_note_detuned_by_a_known_amount_still_lands_in_the_right_bin() {
        // One whole sub-bin flat: without the correction this note's energy
        // folds into the semitone below, which is the error that reads a track
        // as its own neighbour.
        let detune = -CENTS_PER_BIN;
        let flat = sine(69, 2.0, detune);
        let target = 9 * BINS_PER_SEMITONE;

        // Path 1: hand S2's estimate to S3 and let it rotate the bin grid.
        let est = tuning::estimate(&flat, SR).unwrap();
        assert!((est.cents - detune).abs() < 8.0, "S2 read {} cents", est.cents);
        assert_eq!(detune_bin_shift(est.cents), 1);
        let shifted = pcp(&flat, SR, est.cents).unwrap();
        assert_eq!(argmax(&shifted.bins), target, "bin grid not rotated: {:?}", shifted.bins);
        assert_eq!(argmax(&shifted.fold_to_12()), 9);

        // Path 2: resample first, then pass zero — what the engine does once the
        // offset clears the threshold. Both must reach the same bin.
        let corrected = tuning::correct(&flat, SR, est.cents).unwrap();
        let resampled = pcp(&corrected, SR, 0.0).unwrap();
        assert_eq!(argmax(&resampled.bins), target, "resample missed: {:?}", resampled.bins);

        // And the uncorrected vector really is on the wrong bin, or neither
        // path above proved anything.
        assert_ne!(argmax(&pcp(&flat, SR, 0.0).unwrap().bins), target);
    }

    #[test]
    fn beat_chroma_returns_one_row_per_beat() {
        let samples = sig::progression(&sig::major_pop(), 0, SR, 4.0, 0.0);
        let beats: Vec<f64> = (0..8).map(|i| i as f64 * 500.0).collect();
        assert_eq!(chroma_per_beat(&samples, SR, &beats).unwrap().len(), beats.len());
    }
}
