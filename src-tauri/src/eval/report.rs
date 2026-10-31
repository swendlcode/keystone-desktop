//! Formatting, which is load-bearing rather than cosmetic.
//!
//! The product's whole argument against the incumbent is that "around 76%" was
//! quoted without naming the measure. So the harness cannot print a headline
//! number: every figure here arrives with its five-category breakdown, the mode
//! and key-signature figures beside it, the corpus it came from, and what the
//! corpus does not cover. [`CAVEAT`] is printed with every report and is the
//! part that matters most, because a synthetic score is the easiest number in
//! this project to quote by accident.

use std::fmt::Write as _;

use super::mirex::{FifthConvention, KeyScore, MIK_EXACT, MIK_WEIGHTED, PUBLISH_GATE_WEIGHTED};
use super::synthetic::{SetScore, SyntheticReport};

/// Printed with every report. Not a disclaimer — a statement of what has and
/// has not been measured.
pub const CAVEAT: &str = "\
These numbers come from a SYNTHETIC corpus generated in process: rendered chord
progressions with known tonics, the same material detuned by a known number of
cents, and percussive / atonal / silent material that has no key at all.

A synthetic corpus is NOT GiantSteps and these numbers DO NOT TRANSFER to real
audio. Synthetic chords have no production, no percussion over the harmony, no
sampled or re-pitched material, no modulation beyond what the fixture states and
no annotator disagreement. The corpus is easier than real records by
construction, so a score here is an upper bound and not an estimate.

Nothing here may be published as an accuracy claim. What has to be measured
before any claim is made:
  1. GiantSteps+ EDM Key (Zenodo 1095691, 600 tracks, CC BY-SA 4.0) scored by
     this same scorer, stating that the 600-track revision is not byte-identical
     to the 604 the published figures used.
  2. The S-KEY ONNX voter, which is absent from this tree, so every key figure
     below is the TEMPLATE VOTER ALONE. The ensemble is the cheapest real gain
     available and it is unmeasured here.
  3. A modally balanced corpus, because a weighted score on an 84.8%-minor
     dataset flatters a minor-biased model.
  4. The awkward-material set from ENGINE.md 7.2: swung, half-time, live,
     acoustic and ambient, where the 24-key taxonomy itself stops fitting.";

/// One scored set, in full. Never a single figure.
pub fn key_score(label: &str, s: &KeyScore) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{label}  (n = {})", s.n);
    if s.n == 0 {
        let _ = writeln!(out, "  nothing scored");
        return out;
    }
    let _ = writeln!(
        out,
        "  MIREX weighted  {:6.2}   (exact match {:6.2}%)",
        s.weighted, s.exact
    );
    let _ = writeln!(
        out,
        "    correct   {:4}  {:6.2}%   x1.0\n    \
             fifth     {:4}  {:6.2}%   x0.5\n    \
             relative  {:4}  {:6.2}%   x0.3\n    \
             parallel  {:4}  {:6.2}%   x0.2\n    \
             other     {:4}  {:6.2}%   x0.0",
        s.correct,
        s.rate(s.correct),
        s.fifth,
        s.rate(s.fifth),
        s.relative,
        s.rate(s.relative),
        s.parallel,
        s.rate(s.parallel),
        s.other,
        s.rate(s.other),
    );
    let _ = writeln!(
        out,
        "  mode accuracy {:6.2}%   key-signature accuracy {:6.2}%",
        s.mode_accuracy, s.key_signature_accuracy
    );
    let _ = writeln!(
        out,
        "  reference minor {:.3}   estimate minor {:.3}",
        s.reference_minor, s.estimate_minor
    );
    let convention = match s.convention {
        FifthConvention::Dominant => "dominant only (MIREX as published, mir_eval)",
        FifthConvention::Both => "dominant and subdominant",
    };
    let _ = writeln!(
        out,
        "  fifths credited: {convention}; {} subdominant estimate(s) inside `other`, \
         weighted under the other convention {:.2}",
        s.subdominant, s.weighted_other_convention
    );
    out
}

pub fn set_score(set: &SetScore) -> String {
    let mut out = key_score(&set.label, &set.score);
    if !set.flagged_ambiguous.is_empty() {
        let _ = writeln!(
            out,
            "  flagged ambiguous: {} of {} — {}",
            set.flagged_ambiguous.len(),
            set.score.n,
            set.flagged_ambiguous.join(", ")
        );
    }
    for m in &set.misses {
        let _ = writeln!(
            out,
            "  miss  {:<28}  {} -> {}  [{}]  confidence {:.2}",
            m.name,
            m.reference,
            m.estimate,
            m.category.as_str(),
            m.confidence
        );
    }
    out
}

/// The whole synthetic report, caveat first so it cannot be skipped by someone
/// reading only the top of the output.
pub fn synthetic(r: &SyntheticReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "\n=== Keystone key accuracy harness: SYNTHETIC corpus ===\n");
    let _ = writeln!(out, "{CAVEAT}\n");
    let _ = writeln!(out, "--- key, tuning stage on ---\n");
    let _ = write!(out, "{}", set_score(&r.in_tune));
    for set in &r.per_detune {
        let _ = write!(out, "{}", set_score(set));
    }
    let _ = write!(out, "{}", set_score(&r.detuned_all));
    let _ = writeln!(out, "--- the same detuned corpus with S2 skipped ---\n");
    let _ = write!(out, "{}", set_score(&r.detuned_without_tuning));
    let _ = writeln!(
        out,
        "  tuning stage is worth {:+.2} weighted / {:+.2} exact on this corpus",
        r.detuned_all.score.weighted - r.detuned_without_tuning.score.weighted,
        r.detuned_all.score.exact - r.detuned_without_tuning.score.exact,
    );
    let _ = writeln!(
        out,
        "  (Faraldo and Sha'ath both measured NO aggregate gain from tuning handling on\n   \
         digitally-produced EDM at A440. A gain here is a gain on deliberately detuned\n   \
         material, which is the tail this stage exists for, not a library-wide gain.)\n"
    );

    let _ = writeln!(out, "--- S2 tuning recovery ---\n");
    for t in &r.tuning {
        let _ = writeln!(
            out,
            "  {:+.0} cents:  mean |error| {:.2} c, max {:.2} c, resampled {} of {}",
            t.detune_cents, t.mean_abs_error_cents, t.max_abs_error_cents, t.corrected, t.n
        );
    }

    let _ = writeln!(out, "\n--- material with no key: the only right answer is to refuse ---\n");
    let _ = writeln!(out, "  refused {} of {}", r.refusals.refused, r.refusals.n);
    for (name, camelot, confidence) in &r.refusals.answered_anyway {
        let _ = writeln!(out, "  ANSWERED ANYWAY  {name} -> {camelot} at {confidence:.2}");
    }

    let _ = writeln!(
        out,
        "\nThe bar on GiantSteps is {MIK_WEIGHTED:.2} weighted / {MIK_EXACT:.2}% exact \
         (Mixed In Key 8.3)."
    );
    let _ = writeln!(
        out,
        "The publish gate is {PUBLISH_GATE_WEIGHTED:.1} weighted ON GIANTSTEPS+. \
         It is NOT met or tested by anything above."
    );
    let _ = writeln!(out, "\nharness elapsed {} ms", r.elapsed_ms);
    out
}
