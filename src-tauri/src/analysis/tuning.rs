//! S2 — the tuning estimate and the corrective resample.
//!
//! The stage lives in [`crate::analysis::spectral::tuning`]. This module is the
//! name the rest of the engine reaches it by and nothing else.
//!
//! It was a second, independent copy of S2 — the same function names carrying
//! `todo!()`, sitting beside the finished implementation and reachable from the
//! path the product used, while the working code was reachable only from
//! `spectral::`. That is exactly the pathology [`crate::analysis`] already
//! records for S6-S8, where the duplicate trio compiled and did nothing.
//!
//! Finishing the copy instead of removing it would have been worse than leaving
//! it. The two copies had already drifted apart in their public shape: this one
//! returned a `TuningEstimate` with no `concentration` field, so it had no way
//! to say "this material is not pitched enough to measure" — and a `cents`
//! value with nothing qualifying it is precisely how percussion ends up
//! transposing a track. S2 is also a product surface twice over: the UI prints
//! the offset and the key stage resamples on it, so a second implementation
//! would drift from the one the tests and the eval harness actually measure,
//! and nothing anywhere would report the divergence.
//!
//! One implementation, two paths to it.

pub use crate::analysis::spectral::tuning::*;

#[cfg(test)]
mod tests {
    //! Reached through this path on purpose: these assertions fail if the alias
    //! is ever replaced by a fresh implementation of S2 that drifts from the one
    //! `spectral` tests.

    use super::{estimate, CORRECTION_THRESHOLD_CENTS, MIN_CONCENTRATION};
    use crate::analysis::spectral::{test_signals as sig, test_unpitched as unp};

    const SR: u32 = sig::TEST_RATE;

    #[test]
    fn a_tone_at_concert_pitch_reads_as_a440() {
        let est = estimate(&sig::tone(69, SR, 2.0, 0.0), SR).unwrap();
        assert!(est.cents.abs() < 4.0, "A440 read as {} cents", est.cents);
        assert!(!est.needs_correction);
        assert!(est.concentration > MIN_CONCENTRATION);
    }

    #[test]
    fn a_known_detune_comes_back_as_that_detune() {
        for detune in [-30.0f32, -8.0, 22.0] {
            let est = estimate(&sig::tone(69, SR, 2.0, detune), SR).unwrap();
            assert!(
                (est.cents - detune).abs() < 6.0,
                "{detune} cents read as {} cents",
                est.cents
            );
            assert_eq!(est.needs_correction, detune.abs() > CORRECTION_THRESHOLD_CENTS);
        }
    }

    #[test]
    fn unpitched_material_never_asks_to_be_resampled() {
        let est = estimate(&unp::percussion(SR, 4.0), SR).unwrap();
        assert!(est.concentration < MIN_CONCENTRATION);
        assert!(!est.needs_correction);
    }
}
