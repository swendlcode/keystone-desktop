//! The duration floor: how much audio an answer needs before it is an answer.
//!
//! Its own file rather than another case in `signal_tests`, which is already at
//! the 300-line module cap. It shares that file's path runner, because the claim
//! being tested is about S2 -> S3 -> S4 end to end and not about the arithmetic
//! in `fuse` alone.

use super::signal_tests::{analyze, key_of};
use crate::analysis::spectral::test_signals as sig;
use crate::models::KeyMode;

const SR: u32 = sig::TEST_RATE;

#[test]
fn too_little_audio_is_refused_rather_than_answered() {
    // S3 samples a 93 ms window every 371 ms, so a fifth of a second is one
    // frame: one moment of one chord. It can be as peaked as a whole track, and
    // before the duration floor existed 0.2 s of a I-V-vi-IV came back at
    // confidence 0.73-0.83 — a confident answer from four frames, in the one
    // part of the product whose promise is that it does not do that.
    for seconds in [0.2f32, 0.5, 1.0] {
        let d = analyze(&sig::progression(&sig::major_pop(), 0, SR, seconds, 0.0), SR);
        assert!(
            d.ambiguous,
            "{seconds} s was answered as {} at confidence {:.2}",
            d.result.camelot, d.result.confidence
        );
        assert!(
            (d.result.confidence as f32) < super::fuse::AMBIGUOUS_CONFIDENCE_FLOOR,
            "{seconds} s reached confidence {:.2}",
            d.result.confidence
        );
        // No runner-up either: offering an alternative implies a second-best
        // reading existed, and over one frame it did not.
        assert_eq!(d.result.alt_root, None, "{seconds} s offered an alternative key");
    }

    // And the floor must not reach material a listener could name. Eight
    // seconds of the same progression is a key, and it still resolves.
    let eight = analyze(&sig::progression(&sig::major_pop(), 0, SR, 8.0, 0.0), SR);
    assert_eq!(key_of(&eight), (0, KeyMode::Major), "{}", eight.result.camelot);
    assert!(!eight.ambiguous, "eight seconds of a plain progression was refused");

    // The ramp is monotone, and more audio never supports less.
    let support = super::fuse::duration_support;
    assert_eq!(support(0.0), 0.0);
    assert_eq!(support(1.0), 0.0);
    assert!(support(2.0) < support(4.0));
    assert_eq!(support(5.0), 1.0);
    assert_eq!(support(600.0), 1.0);
    assert_eq!(support(f32::NAN), 0.0);
}
