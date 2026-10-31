//! S2 -> S3 -> S4 end to end, against signals whose answer is known.
//!
//! Not only the easy case: progressions in all twelve keys, a track detuned by
//! a stated number of cents, a relative-ambiguous loop, a key change at the
//! drop, atonal material, pure percussion and silence. The last three matter
//! most - the only way to be right about a drum loop is to say nothing.

use super::{detect_detailed, KeyDetection, KeyInput};
use crate::analysis::camelot::Camelot;
use crate::analysis::spectral::{chroma, test_signals as sig, test_unpitched as unp, tuning};
use crate::models::KeyMode;

const SR: u32 = sig::TEST_RATE;

/// The whole key path for one buffer, exactly as S1..S4 specify it: tuning
/// first, the corrective resample when S2 asks for one, chroma, then the key
/// stage. No model voter, because there is no bundled graph yet.
///
/// This mirrors what the orchestrator has to do rather than shortcutting it, so
/// the resample sign and the residual bookkeeping are under test and not only
/// the DSP.
pub(super) fn analyze(samples: &[f32], sample_rate: u32) -> KeyDetection {
    let est = tuning::estimate(samples, sample_rate).unwrap();
    let (audio, residual) = if est.needs_correction {
        (tuning::correct(samples, sample_rate, est.cents).unwrap(), 0.0)
    } else {
        (samples.to_vec(), est.cents)
    };
    let pcp = chroma::pcp(&audio, sample_rate, residual).unwrap();
    let input = KeyInput {
        pcp: &pcp,
        mono: &audio,
        sample_rate,
        mono_22050: &[],
        tuning_cents: est.cents,
        residual_cents: residual,
    };
    detect_detailed(&input, None).unwrap()
}

pub(super) fn key_of(d: &KeyDetection) -> (u8, KeyMode) {
    (d.result.root, d.result.mode)
}
#[test]
fn chord_progressions_resolve_in_every_key() {
    let cases: [(&str, Vec<sig::Chord>, KeyMode); 3] = [
        ("I-V-vi-IV-I", sig::major_pop(), KeyMode::Major),
        ("I-IV-V7-I", sig::major_cadence(), KeyMode::Major),
        ("i-iv-i-bVII-bVI", sig::minor_iv(), KeyMode::Minor),
    ];
    for (name, chords, mode) in cases {
        for tonic in 0..12u8 {
            let samples = sig::progression(&chords, tonic as i32, SR, 8.0, 0.0);
            let d = analyze(&samples, SR);
            assert_eq!(
                key_of(&d),
                (tonic, mode),
                "{name} in {tonic} {mode:?} came back as {} (confidence {:.2})",
                d.result.camelot,
                d.result.confidence
            );
            assert!(!d.ambiguous, "{name} in {tonic} was called ambiguous");
            assert!(
                d.evidence > 0.5,
                "{name} in {tonic}: evidence only {:.2}",
                d.evidence
            );
        }
    }
}

#[test]
fn the_production_sample_rate_gives_the_same_answer() {
    // Everything else runs at 22,050 to keep a debug build quick; this pins
    // that the stage is rate-agnostic rather than tuned to that rate.
    let samples = sig::progression(&sig::major_pop(), 7, 44_100, 8.0, 0.0);
    let d = analyze(&samples, 44_100);
    assert_eq!(key_of(&d), (7, KeyMode::Major), "G major read as {}", d.result.camelot);
}

#[test]
fn a_detuned_track_resolves_to_the_same_key_and_reports_the_offset() {
    // A wrong tuning estimate shifts every result downstream, so this checks
    // both halves: the key survives, and the cents the UI shows are right.
    for detune in [-28.0f32, -16.0, 16.0, 28.0] {
        let samples = sig::progression(&sig::minor_iv(), 9, SR, 8.0, detune);
        let est = tuning::estimate(&samples, SR).unwrap();
        assert!(
            (est.cents - detune).abs() < 6.0,
            "{detune} cents reported as {:.1}",
            est.cents
        );
        let d = analyze(&samples, SR);
        assert_eq!(
            key_of(&d),
            (9, KeyMode::Minor),
            "A minor at {detune} cents came back as {}",
            d.result.camelot
        );
        assert!((d.result.tuning_cents - detune as f64).abs() < 6.0);
    }
}

#[test]
fn correcting_the_detune_first_gives_the_same_key_as_leaving_it() {
    // The two routes S2 offers - resample, or hand the offset to the chroma bin
    // grid - have to agree, or which one the orchestrator picks changes results.
    let detune = -26.0f32;
    let samples = sig::progression(&sig::major_cadence(), 2, SR, 8.0, detune);
    let est = tuning::estimate(&samples, SR).unwrap();
    assert!(est.needs_correction);
    let corrected = tuning::correct(&samples, SR, est.cents).unwrap();
    let pcp = chroma::pcp(&corrected, SR, 0.0).unwrap();
    let d = detect_detailed(
        &KeyInput {
            pcp: &pcp,
            mono: &corrected,
            sample_rate: SR,
            mono_22050: &[],
            tuning_cents: detune,
            residual_cents: 0.0,
        },
        None,
    )
    .unwrap();
    assert_eq!(key_of(&d), (2, KeyMode::Major), "came back as {}", d.result.camelot);
}

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
        d.result.confidence < super::fuse::AMBIGUOUS_CONFIDENCE_FLOOR as f64,
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
fn a_relative_ambiguous_loop_lands_in_the_pair_and_costs_confidence() {
    // `bVI - bIII - bVII - i` with every chord the same length has the relative
    // major's pitch content and nothing weighted as a tonic. There is no right
    // answer to give, so the requirements are that the answer stays inside the
    // relative pair and that it is visibly less certain than the same key
    // stated plainly.
    let ambiguous =
        analyze(&sig::progression(&sig::minor_relative_ambiguous(), 0, SR, 16.0, 0.0), SR);
    let clear = analyze(&sig::progression(&sig::minor_iv(), 0, SR, 16.0, 0.0), SR);

    let code = Camelot::parse(&ambiguous.result.camelot).unwrap();
    // 5A is C minor and 5B is Eb major: one Camelot number, two letters.
    assert_eq!(
        code.number, 5,
        "landed outside the relative pair: {}",
        ambiguous.result.camelot
    );
    assert_eq!(key_of(&clear), (0, KeyMode::Minor), "{}", clear.result.camelot);
    assert!(!clear.result.mode_ambiguous, "an unambiguous minor key was flagged");
    assert!(
        clear.relative_confidence > ambiguous.relative_confidence + 0.2,
        "the ambiguous loop was as certain of its letter as the clear one \
         ({:.2} vs {:.2})",
        ambiguous.relative_confidence,
        clear.relative_confidence
    );
    assert!(ambiguous.result.confidence < clear.result.confidence);
}

#[test]
fn a_track_sitting_between_relative_keys_shows_the_pair_rather_than_a_letter() {
    // Half in C minor, half in its relative major. The chunk votes split evenly
    // across one Camelot number, which is the case the `8A/8B` display exists
    // for - and the one where committing to a letter is a coin flip presented
    // as an answer.
    let mut mixed = sig::progression(&sig::minor_iv(), 0, SR, 15.0, 0.0);
    mixed.extend(sig::progression(&sig::major_pop(), 3, SR, 15.0, 0.0));
    let d = analyze(&mixed, SR);

    let code = Camelot::parse(&d.result.camelot).unwrap();
    assert_eq!(code.number, 5, "landed outside the relative pair: {}", d.result.camelot);
    assert!(
        d.result.mode_ambiguous,
        "committed to {} at relative confidence {:.2}",
        d.result.camelot, d.relative_confidence
    );
    assert!(d.relative_confidence < super::fuse::MODE_AMBIGUITY_THRESHOLD);
    // Both readings are present per chunk, which is what makes the flag honest
    // rather than a guess about the aggregate.
    let modes: Vec<KeyMode> = d.result.chunk_keys.iter().map(|c| c.mode).collect();
    assert!(modes.contains(&KeyMode::Minor) && modes.contains(&KeyMode::Major), "{modes:?}");
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
    assert!(d.evidence < 0.3, "evidence {:.2}", d.evidence);
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
    assert_eq!(cluster.result.confidence, 0.0);
    assert_eq!(drums.result.confidence, 0.0);
}

#[test]
fn a_key_change_at_the_drop_shows_up_per_chunk_and_costs_confidence() {
    let mut mixed = sig::progression(&sig::major_pop(), 0, SR, 15.0, 0.0);
    mixed.extend(sig::progression(&sig::major_pop(), 6, SR, 15.0, 0.0));
    let changed = analyze(&mixed, SR);
    let steady = analyze(&sig::progression(&sig::major_pop(), 0, SR, 30.0, 0.0), SR);

    assert_eq!(changed.result.chunk_keys.len(), 2, "expected one vote per 15 s");
    let roots: Vec<u8> = changed.result.chunk_keys.iter().map(|c| c.root).collect();
    assert_ne!(roots[0], roots[1], "the chunks did not see the change: {roots:?}");
    assert!(changed.chunk_agreement < 1.0);
    assert_eq!(steady.chunk_agreement, 1.0);
    assert!(
        changed.result.confidence < steady.result.confidence,
        "a modulating track was as confident as a steady one ({:.2} vs {:.2})",
        changed.result.confidence,
        steady.result.confidence
    );
    // And the per-chunk rows carry real times, so a later deck view can seek.
    assert_eq!(changed.result.chunk_keys[0].start_ms, 0.0);
    assert!(changed.result.chunk_keys[1].start_ms > 14_000.0);
}

#[test]
fn every_result_carries_a_code_the_wheel_can_parse() {
    for tonic in 0..12u8 {
        let samples = sig::progression(&sig::major_pop(), tonic as i32, SR, 8.0, 0.0);
        let d = analyze(&samples, SR);
        let parsed = Camelot::parse(&d.result.camelot).expect("unparseable camelot");
        assert_eq!(parsed.to_key(), (d.result.root, d.result.mode));
        assert!((0.0..=1.0).contains(&d.result.confidence));
        assert!(d.result.root < 12);
    }
}
