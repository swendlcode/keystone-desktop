//! The refusal cases: percussion, silence and an atonal wash.
//!
//! Its own file rather than more cases in `signal_tests`, which is at the
//! 300-line module cap, and its own file for a second reason: these are the
//! tests the 2026-11-01 confidence calibration had to argue with. Refusing a
//! drum loop is a product promise and the calibration kept it; refusing a
//! uniform chromatic wash turned out to have been an accident of a gate set on
//! synthetic material, and each test below now says which of the two it is.

use super::signal_tests::analyze;
use super::fuse;
use crate::analysis::camelot::Camelot;
use crate::analysis::spectral::{test_signals as sig, test_unpitched as unp};
use super::{detect_detailed, KeyInput};
use crate::analysis::spectral::chroma;

const SR: u32 = sig::TEST_RATE;

#[test]
fn pure_percussion_comes_back_ambiguous_rather_than_guessed() {
    let samples = unp::percussion(SR, 20.0);
    let d = analyze(&samples, SR);
    assert!(
        d.ambiguous,
        "a drum loop was reported as {} at confidence {:.2}",
        d.result.camelot, d.result.confidence
    );
    assert!(d.evidence < 0.3, "percussion scored {:.2} evidence", d.evidence);
    assert!(
        d.result.confidence < fuse::AMBIGUOUS_CONFIDENCE_FLOOR as f64,
        "confidence {:.2}",
        d.result.confidence
    );
    assert!(d.result.mode_ambiguous, "an ambiguous result must not claim a letter");
}

#[test]
fn silence_comes_back_ambiguous_and_does_not_panic() {
    for seconds in [0.5f32, 20.0] {
        let d = analyze(&unp::silence(SR, seconds), SR);
        assert!(d.ambiguous);
        assert_eq!(d.result.confidence, 0.0);
        assert_eq!(d.result.alt_root, None);
        assert!(Camelot::parse(&d.result.camelot).is_some());
    }
    // An empty buffer is a decode that produced nothing, which must not be a
    // panic on the analysis worker.
    let pcp = chroma::pcp(&[], SR, 0.0).unwrap();
    let d = detect_detailed(
        &KeyInput {
            pcp: &pcp,
            mono: &[],
            sample_rate: SR,
            mono_22050: &[],
            tuning_cents: 0.0,
            residual_cents: 0.0,
        },
        None,
    )
    .unwrap();
    assert!(d.ambiguous);
}

#[test]
fn atonal_material_comes_back_ambiguous_too() {
    // Percussion is the "no pitch at all" case. A chromatic cluster is the
    // harder one: the chroma is full rather than empty, so a stage that only
    // checks for silence would read it as a key.
    let d = analyze(&unp::chromatic_cluster(SR, 10.0), SR);
    assert!(
        d.ambiguous,
        "a twelve-note cluster was reported as {} at confidence {:.2}",
        d.result.camelot, d.result.confidence
    );
    // What refuses it is the confidence floor, **not** the evidence gate.
    //
    // This assertion used to read `evidence < 0.3` and it was true only
    // because `SPREAD_FLOOR` sat at 0.08, where no real record reached it
    // either. The 2026-11-01 calibration measured the cluster's whole-track
    // spread at 0.0254 and the real corpus's at a median of 0.0370 with a 20th
    // percentile of 0.0254, so the two are not separable by spread and
    // pretending otherwise is what broke the product. The cluster is refused
    // here because its candidates do not separate, which is the honest reason.
    assert!(d.evidence > 0.0, "the gate is back to refusing real records too");
    assert!(
        (d.result.confidence as f32) < fuse::AMBIGUOUS_CONFIDENCE_FLOOR,
        "confidence {:.3}",
        d.result.confidence
    );
}

#[test]
fn confidence_ranks_material_by_how_much_it_supports_an_answer() {
    // The ordering is the product claim: a diatonic progression is answerable,
    // a relative-ambiguous loop less so, and unpitched material not at all.
    let clear = analyze(&sig::progression(&sig::major_pop(), 0, SR, 16.0, 0.0), SR);
    let ambiguous = analyze(&sig::progression(&sig::minor_relative_ambiguous(), 0, SR, 16.0, 0.0), SR);
    let cluster = analyze(&unp::chromatic_cluster(SR, 10.0), SR);
    let drums = analyze(&unp::percussion(SR, 16.0), SR);

    assert!(
        clear.result.confidence > 0.5,
        "a plain I-V-vi-IV only reached {:.2}",
        clear.result.confidence
    );
    assert!(
        clear.result.confidence > ambiguous.result.confidence,
        "{:.2} vs {:.2}",
        clear.result.confidence,
        ambiguous.result.confidence
    );
    assert!(ambiguous.result.confidence > cluster.result.confidence);
    // Drums are a hard zero: the spread gate is the only thing in the engine
    // that can tell percussion from music, and `SPREAD_FLOOR` is held above the
    // whole measured percussion band so that it does. The cluster is not a hard
    // zero, since the 2026-11-01 calibration found its spread sitting inside the
    // real corpus's own range; it is refused on separation instead, and the
    // ordering the test is really about is unchanged.
    assert_eq!(drums.result.confidence, 0.0);
    assert!(
        (cluster.result.confidence as f32) < fuse::AMBIGUOUS_CONFIDENCE_FLOOR,
        "a chromatic cluster reached confidence {:.3}",
        cluster.result.confidence
    );
}
