//! Tests for the fusion, confidence and ambiguity rules.
//!
//! Split out of `fuse.rs` to keep that module inside the 300-line cap; it is
//! `fuse`'s own test module, not a sibling.
use super::*;
use crate::analysis::camelot::Camelot;

fn peaked(label: usize, mass: f32) -> KeyProbabilities {
    let rest = (1.0 - mass) / 23.0;
    let mut p = [rest; 24];
    p[label] = mass;
    KeyProbabilities { p }
}

#[test]
fn the_relative_key_shares_its_camelot_number() {
    // The whole reason `8A/8B` is the ambiguity display.
    for root in 0u8..12 {
        for mode in [KeyMode::Major, KeyMode::Minor] {
            let (rr, rm) = relative_of(root, mode);
            let a = Camelot::from_key(root, mode);
            let b = Camelot::from_key(rr, rm);
            assert_eq!(a.number, b.number, "{root} {mode:?} and its relative differ");
            assert_ne!(a.letter, b.letter);
            // And it is an involution.
            assert_eq!(relative_of(rr, rm), (root, mode));
        }
    }
}

#[test]
fn fusion_without_the_model_voter_is_the_template_voter_unchanged() {
    let t = peaked(3, 0.6);
    let fused = fuse(None, &t, &DEFAULT_WEIGHTS);
    for i in 0..24 {
        assert!((fused.p[i] - t.p[i]).abs() < 1e-5, "label {i} moved");
    }
}

#[test]
fn fusion_moves_the_answer_when_the_voters_disagree() {
    let template = peaked(3, 0.7);
    let model = peaked(10, 0.7);
    let fused = fuse(Some(&model), &template, &DEFAULT_WEIGHTS);
    // 0.6 on the model against 0.4 on the template, so the model wins — and
    // the winner's mass is lower than either voter's, which is the point.
    assert_eq!(fused.best().0, 10);
    assert!(fused.best().1 < 0.7);
    let total: f32 = fused.p.iter().sum();
    assert!((total - 1.0).abs() < 1e-4);
}

#[test]
fn a_uniform_distribution_has_no_confidence_however_much_evidence_there_is() {
    let uniform = KeyProbabilities { p: [1.0 / 24.0; 24] };
    assert!(confidence(&uniform, 1.0, 1.0) < 0.05);
}

#[test]
fn no_evidence_means_no_confidence_however_peaked_the_distribution() {
    // A drum loop can still produce a peaked-looking distribution; without
    // evidence that the material is pitched, it means nothing.
    assert_eq!(confidence(&peaked(3, 0.95), 0.0, 1.0), 0.0);
    assert_eq!(evidence_from_spread(0.02), 0.0);
    assert_eq!(evidence_from_spread(f32::NAN), 0.0);
    assert_eq!(evidence_from_spread(0.5), 1.0);
}

#[test]
fn a_relative_split_is_flagged_even_when_top_one_looks_healthy() {
    // C major (label 3) and A minor (label 22) splitting the mass is the
    // single most common real error, and the badge must say so.
    let mut p = [0.002f32; 24];
    p[3] = 0.5;
    p[22] = 0.48;
    let dist = KeyProbabilities { p };
    assert!(relative_confidence(&dist, 0, KeyMode::Major) < MODE_AMBIGUITY_THRESHOLD);
    let result = to_result(Resolution {
        fused: &dist,
        evidence: 1.0,
        agreement: 1.0,
        tuning_cents: 0.0,
        chunk_keys: Vec::new(),
    })
    .unwrap();
    assert_eq!(result.camelot, "8B");
    assert!(result.mode_ambiguous, "a near-even relative split was not flagged");
}

#[test]
fn a_clean_win_is_not_flagged_and_lands_on_the_right_badge() {
    let result = to_result(Resolution {
        fused: &peaked(3, 0.9),
        evidence: 1.0,
        agreement: 1.0,
        tuning_cents: -7.5,
        chunk_keys: Vec::new(),
    })
    .unwrap();
    assert_eq!((result.root, result.mode), (0, KeyMode::Major));
    assert_eq!(result.camelot, "8B");
    assert!(!result.mode_ambiguous);
    assert!(result.confidence > 0.6, "confidence {}", result.confidence);
    assert_eq!(result.tuning_cents, -7.5);
}

#[test]
fn disagreeing_chunks_cost_confidence_without_changing_the_answer() {
    let dist = peaked(3, 0.9);
    let agreed = to_result(Resolution {
        fused: &dist,
        evidence: 1.0,
        agreement: 1.0,
        tuning_cents: 0.0,
        chunk_keys: Vec::new(),
    })
    .unwrap();
    let split = to_result(Resolution {
        fused: &dist,
        evidence: 1.0,
        agreement: 0.4,
        tuning_cents: 0.0,
        chunk_keys: Vec::new(),
    })
    .unwrap();
    assert_eq!(agreed.camelot, split.camelot);
    assert!(split.confidence < agreed.confidence);
}

