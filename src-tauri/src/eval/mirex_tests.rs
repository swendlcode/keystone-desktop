//! Hand-worked examples for the scorer.
//!
//! A scorer that is wrong makes every later number meaningless, and the failure
//! is silent: the harness still prints a plausible figure. So every category is
//! pinned against a relation worked out by hand from the circle of fifths rather
//! than from the implementation, the wrap-around cases are included because
//! `rem_euclid` is where this kind of code actually breaks, and the aggregate
//! arithmetic is checked against a total computed on paper.

use super::mirex::{
    classify, classify_with, score, score_with, Category, FifthConvention, PUBLISH_GATE_WEIGHTED,
};
use crate::models::KeyMode;
use crate::models::KeyMode::{Major, Minor};

const C_MAJOR: (u8, KeyMode) = (0, Major);
const A_MINOR: (u8, KeyMode) = (9, Minor);

#[test]
fn every_category_against_a_relation_worked_out_by_hand() {
    // Reference C major. Its dominant is G, its subdominant F, its relative
    // A minor, its parallel C minor.
    for (est, expected, why) in [
        ((0, Major), Category::Correct, "C major is C major"),
        ((7, Major), Category::Fifth, "G major is the dominant of C major"),
        ((5, Major), Category::Other, "F major is the subdominant, not the dominant"),
        ((9, Minor), Category::Relative, "A minor is the relative minor of C major"),
        ((0, Minor), Category::Parallel, "C minor is the parallel minor of C major"),
        ((2, Major), Category::Other, "D major is a tone away"),
        ((3, Major), Category::Other, "Eb major shares no relation with C major"),
        ((3, Minor), Category::Other, "Eb minor is not C major's relative"),
    ] {
        assert_eq!(classify(est, C_MAJOR), expected, "{why}");
    }

    // Reference A minor. Dominant E minor, subdominant D minor, relative
    // C major, parallel A major.
    for (est, expected, why) in [
        ((9, Minor), Category::Correct, "A minor is A minor"),
        ((4, Minor), Category::Fifth, "E minor is the dominant of A minor"),
        ((2, Minor), Category::Other, "D minor is the subdominant of A minor"),
        ((0, Major), Category::Relative, "C major is the relative major of A minor"),
        ((9, Major), Category::Parallel, "A major is the parallel major of A minor"),
        ((1, Major), Category::Other, "C# major shares no relation with A minor"),
    ] {
        assert_eq!(classify(est, A_MINOR), expected, "{why}");
    }
}

#[test]
fn the_relations_that_wrap_round_the_circle_are_still_relations() {
    // B major's dominant is F# major: 11 + 7 = 18 = 6 mod 12. An implementation
    // that subtracts without wrapping scores this as `other`.
    assert_eq!(classify((6, Major), (11, Major)), Category::Fifth);
    // Eb minor's relative major is Gb (= F#) major: 3 + 3 = 6.
    assert_eq!(classify((6, Major), (3, Minor)), Category::Relative);
    // B minor's relative major is D major, and going the other way D major's
    // relative minor is B minor. Both directions have to land on `relative`, or
    // the reference and the estimate are not being distinguished.
    assert_eq!(classify((2, Major), (11, Minor)), Category::Relative);
    assert_eq!(classify((11, Minor), (2, Major)), Category::Relative);
    // And a root outside 0-11 is folded rather than mis-scored.
    assert_eq!(classify((12, Major), (0, Major)), Category::Correct);
}

#[test]
fn relative_is_directional_and_is_not_symmetric_about_the_mode() {
    // The pair C major / A minor is one key signature. The pair C major /
    // Eb minor is not, and a test that only checks `interval == 3 || == 9`
    // without reading the reference's mode passes both.
    assert_eq!(classify((9, Minor), (0, Major)), Category::Relative);
    assert_eq!(classify((3, Minor), (0, Major)), Category::Other);
    assert_eq!(classify((3, Major), (0, Minor)), Category::Relative);
    assert_eq!(classify((9, Major), (0, Minor)), Category::Other);
}

#[test]
fn the_fifth_convention_is_a_parameter_and_both_answers_are_reported() {
    // F major against C major is a fifth below. MIREX as published and
    // mir_eval credit the dominant only, so this is `other` by default.
    assert_eq!(classify_with((5, Major), C_MAJOR, FifthConvention::Dominant), Category::Other);
    assert_eq!(classify_with((5, Major), C_MAJOR, FifthConvention::Both), Category::Fifth);

    let s = score(&[(C_MAJOR, (5, Major))]);
    assert_eq!(s.other, 1);
    assert_eq!(s.subdominant, 1, "a subdominant estimate has to be countable inside `other`");
    assert!((s.weighted - 0.0).abs() < 1e-9);
    assert!(
        (s.weighted_other_convention - 50.0).abs() < 1e-9,
        "crediting the subdominant would make this 50.0, and that has to be visible"
    );
}

#[test]
fn the_weighted_total_matches_one_computed_on_paper() {
    // Six exact, one dominant, one relative, one parallel, one unrelated.
    // weight = 6*1.0 + 0.5 + 0.3 + 0.2 = 7.0 over ten tracks = 70.0 weighted,
    // 60.0 exact. The mode matches on the six, the fifth and the unrelated one,
    // so mode accuracy is 80.0; the signature is right on the six and the
    // relative, so 70.0.
    let mut pairs = vec![(C_MAJOR, C_MAJOR); 6];
    pairs.push((C_MAJOR, (7, Major)));
    pairs.push((C_MAJOR, (9, Minor)));
    pairs.push((C_MAJOR, (0, Minor)));
    pairs.push((C_MAJOR, (2, Major)));

    let s = score(&pairs);
    assert_eq!(s.n, 10);
    assert_eq!((s.correct, s.fifth, s.relative, s.parallel, s.other), (6, 1, 1, 1, 1));
    assert!((s.weighted - 70.0).abs() < 1e-9, "weighted {}", s.weighted);
    assert!((s.exact - 60.0).abs() < 1e-9, "exact {}", s.exact);
    assert!((s.mode_accuracy - 80.0).abs() < 1e-9, "mode {}", s.mode_accuracy);
    assert!(
        (s.key_signature_accuracy - 70.0).abs() < 1e-9,
        "key signature {}",
        s.key_signature_accuracy
    );
    assert!((s.rate(s.correct) - 60.0).abs() < 1e-9);
    assert!(!s.clears_publish_gate(), "70.0 is below the {PUBLISH_GATE_WEIGHTED} gate");
}

#[test]
fn a_mode_biased_estimator_posts_a_good_weighted_score_and_the_breakdown_exposes_it() {
    // This is the `edmm` failure, in miniature: an estimator that always
    // answers minor on a corpus that is mostly minor. The weighted total looks
    // respectable and the mode numbers are where it shows up, which is the
    // whole reason the harness is forbidden from reporting one figure.
    let mut pairs: Vec<_> = (0..8).map(|i| ((i as u8, Minor), (i as u8, Minor))).collect();
    pairs.push(((0, Major), (0, Minor)));
    pairs.push(((5, Major), (5, Minor)));

    let s = score(&pairs);
    assert!(s.weighted > 80.0, "weighted {}", s.weighted);
    assert_eq!(s.parallel, 2, "every major reference should be a parallel error");
    assert!((s.mode_accuracy - 80.0).abs() < 1e-9);
    assert!((s.estimate_minor - 1.0).abs() < 1e-9, "every estimate was minor");
    assert!((s.reference_minor - 0.8).abs() < 1e-9);
    assert!(
        s.estimate_minor > s.reference_minor,
        "the mode bias has to be readable off the two fractions"
    );
}

#[test]
fn an_empty_corpus_scores_zero_rather_than_dividing_by_it() {
    let s = score(&[]);
    assert_eq!(s.n, 0);
    assert!(s.weighted.is_finite() && s.weighted == 0.0);
    assert!(s.exact.is_finite() && s.exact == 0.0);
    assert!(s.mode_accuracy.is_finite());
    assert_eq!(s.rate(0), 0.0);
    assert!(!s.clears_publish_gate());
}

#[test]
fn the_categories_carry_the_ismir_weights_and_nothing_else() {
    assert_eq!(Category::Correct.weight(), 1.0);
    assert_eq!(Category::Fifth.weight(), 0.5);
    assert_eq!(Category::Relative.weight(), 0.3);
    assert_eq!(Category::Parallel.weight(), 0.2);
    assert_eq!(Category::Other.weight(), 0.0);
    // A perfect corpus is 100.0, not 1.0: the figure is a percentage, which is
    // how the bar (75.70) is stated.
    let s = score_with(&[(C_MAJOR, C_MAJOR)], FifthConvention::Both);
    assert!((s.weighted - 100.0).abs() < 1e-9);
}
