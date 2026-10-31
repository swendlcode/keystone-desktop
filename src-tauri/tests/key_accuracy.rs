//! ENGINE.md 7.1's harness, run over the synthetic corpus.
//!
//! Dev-only: the whole file is behind the `eval` feature, so
//! `cargo test --features eval` runs it and a shipping build does not contain
//! it. Run it with `-- --nocapture` to read the report; the assertions below are
//! deliberately the smaller half of this file's value.
//!
//! **Every number printed here is from a synthetic corpus and does not transfer
//! to real audio.** The report says so itself, at the top, because a figure
//! copied out of a test log without its caveat is exactly the failure this
//! product is an argument against.

#![cfg(feature = "eval")]

use std::sync::OnceLock;

use keystone_lib::eval::mirex::{self, FifthConvention};
use keystone_lib::eval::synthetic::SyntheticReport;
use keystone_lib::eval::{corpus, report, synthetic};
use keystone_lib::models::KeyMode::{Major, Minor};

/// The in-tune corpus is 24 keys: two major progressions over twelve major
/// keys and one minor progression over twelve minor ones.
const KEYED_ITEMS: usize = 36;

/// One run, shared by every test in the file. The corpus is 112 renders through
/// the whole key path and the tests below ask different questions of the same
/// answers, so running it per test would quadruple the wall clock for no extra
/// coverage. `OnceLock` rather than a `lazy_static`: the harness returns a
/// `Result` and a poisoned mutex would hide the real failure.
fn harness() -> &'static SyntheticReport {
    static REPORT: OnceLock<SyntheticReport> = OnceLock::new();
    REPORT.get_or_init(|| synthetic::run().expect("harness run"))
}

#[test]
fn the_key_path_scores_itself_on_the_synthetic_corpus() {
    let r = harness();
    // Printed whatever happens, because a failing assertion is exactly when the
    // breakdown is worth reading.
    println!("{}", report::synthetic(&r));

    assert_eq!(r.in_tune.score.n, KEYED_ITEMS, "the corpus lost items");
    assert_eq!(
        r.detuned_all.score.n,
        KEYED_ITEMS * corpus::DETUNE_CENTS.len(),
        "the detuned corpus lost items"
    );

    // Exact match first. The weighted total is the headline in the literature
    // and it is the figure that can be gamed, so it is asserted *after* the one
    // that cannot.
    let s = &r.in_tune.score;
    assert!(
        s.exact >= 97.0,
        "in-tune exact match {:.2}% — a rendered progression with a doubled tonic is \
         the easiest input this engine will ever see",
        s.exact
    );
    assert!(s.weighted >= s.exact, "weighted {:.2} below exact {:.2}", s.weighted, s.exact);
    assert!(
        s.mode_accuracy >= 97.0,
        "mode accuracy {:.2}% — mode is Camelot's A/B letter and the thing a DJ sees",
        s.mode_accuracy
    );
    assert!(
        s.key_signature_accuracy >= s.exact,
        "key-signature accuracy cannot be below exact match"
    );

    // The corpus is deliberately modally balanced: 24 of 36 items major, 12
    // minor. GiantSteps is 84.8% minor, where a minor-biased model posts a good
    // weighted score while being useless, so a balanced corpus is the control.
    assert!(
        (s.reference_minor - 1.0 / 3.0).abs() < 0.01,
        "reference minor fraction {:.3}",
        s.reference_minor
    );
    assert!(
        (s.estimate_minor - s.reference_minor).abs() < 0.06,
        "mode bias: {:.3} of estimates minor against {:.3} of references",
        s.estimate_minor,
        s.reference_minor
    );
}

#[test]
fn material_with_no_key_comes_back_refused_rather_than_guessed() {
    let r = harness();
    assert!(r.refusals.n >= 4, "the corpus must contain material with no key");
    assert!(
        r.refusals.answered_anyway.is_empty(),
        "unpitched material was given a key: {:?}",
        r.refusals.answered_anyway
    );
    assert_eq!(r.refusals.refused, r.refusals.n);
}

#[test]
fn a_known_detune_is_recovered_to_within_a_few_cents() {
    let r = harness();
    assert_eq!(r.tuning.len(), corpus::DETUNE_CENTS.len());
    for t in &r.tuning {
        println!(
            "{:+.0} cents: mean |error| {:.2} c, max {:.2} c, resampled {}/{}",
            t.detune_cents, t.mean_abs_error_cents, t.max_abs_error_cents, t.corrected, t.n
        );
        assert!(
            t.max_abs_error_cents < 6.0,
            "{:+.0} cents was recovered with an error of up to {:.2} cents",
            t.detune_cents,
            t.max_abs_error_cents
        );
        assert_eq!(
            t.corrected, t.n,
            "{:+.0} cents is far outside the 10-cent skip threshold and every item \
             should have been resampled",
            t.detune_cents
        );
    }
}

#[test]
fn key_accuracy_on_the_detuned_corpus_is_restored_to_the_in_tune_level() {
    // This is the claim S2 exists for: a vinyl rip, a pitched edit or a
    // turntable at -35 cents must not cost the user their key. The model's own
    // front end has one bin per semitone and was measured collapsing to the
    // wrong tonic *and* mode at 50 cents, so "restored" is the requirement
    // rather than "degraded gracefully".
    let r = harness();
    let in_tune = r.in_tune.score.exact;
    for set in &r.per_detune {
        println!("{}", report::set_score(set));
        assert!(
            set.score.exact >= in_tune - 3.0,
            "{}: exact match {:.2}% against {:.2}% in tune",
            set.label,
            set.score.exact,
            in_tune
        );
        assert!(
            set.score.mode_accuracy >= r.in_tune.score.mode_accuracy - 3.0,
            "{}: mode accuracy {:.2}% against {:.2}% in tune",
            set.label,
            set.score.mode_accuracy,
            r.in_tune.score.mode_accuracy
        );
    }
}

#[test]
fn the_aggregate_arithmetic_is_the_arithmetic_in_the_paper() {
    // The scorer's own hand-worked cases live next to it as unit tests. What is
    // checked here is the contract this file depends on: the weighted total is a
    // percentage, the five categories partition the corpus, and the fifth
    // convention travels with the number.
    let pairs = vec![
        ((0, Major), (0, Major)),
        ((0, Major), (7, Major)),
        ((0, Major), (9, Minor)),
        ((0, Major), (0, Minor)),
        ((0, Major), (2, Major)),
    ];
    let s = mirex::score(&pairs);
    assert_eq!(s.correct + s.fifth + s.relative + s.parallel + s.other, s.n);
    // (1.0 + 0.5 + 0.3 + 0.2 + 0.0) / 5 = 0.4
    assert!((s.weighted - 40.0).abs() < 1e-9, "weighted {:.4}", s.weighted);
    assert!((s.exact - 20.0).abs() < 1e-9, "exact {:.4}", s.exact);
    assert_eq!(s.convention, FifthConvention::Dominant);
    println!("{}", report::key_score("hand-worked five", &s));
}
