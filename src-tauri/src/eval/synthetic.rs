//! The harness run over the synthetic corpus.
//!
//! What this answers, and nothing more: does the key path recover a key it was
//! given, does it recover it again when the material is detuned by a known
//! number of cents, and does it refuse material that has no key. Those are
//! engineering questions with arithmetic answers. **Whether the engine is
//! accurate on real records is a different question and this file cannot answer
//! it** — see [`super::report`], which prints that in the report rather than
//! leaving it to whoever reads the number.

use std::time::Instant;

use crate::error::Result;
use crate::models::KeyMode;

use super::corpus::{self, Spec, Truth};
use super::key_path;
use super::mirex::{self, KeyScore, Pair};

/// One scored subset of the corpus.
pub struct SetScore {
    pub label: String,
    pub score: KeyScore,
    /// Keyed items the engine flagged ambiguous. Not an error in itself — the
    /// corpus contains material that *should* be flagged — but a count that
    /// climbs means the ambiguity flag has started hiding answers the engine
    /// had, and the flag is the product's differentiator rather than an excuse.
    pub flagged_ambiguous: Vec<String>,
    /// Every item that came back as something other than its reference, with
    /// what it came back as. The breakdown is the point of the harness; a
    /// category count with no examples cannot be debugged.
    pub misses: Vec<Miss>,
}

pub struct Miss {
    pub name: String,
    pub reference: String,
    pub estimate: String,
    pub category: mirex::Category,
    pub confidence: f64,
}

/// How well S2 recovered a known detune.
pub struct TuningRecovery {
    pub detune_cents: f32,
    pub n: usize,
    pub max_abs_error_cents: f32,
    pub mean_abs_error_cents: f32,
    /// How many items S2 decided to resample. A detune this far out that it
    /// declines to correct is a miss even when the key survives.
    pub corrected: usize,
}

/// The items with no key at all.
pub struct RefusalCheck {
    pub n: usize,
    pub refused: usize,
    /// Anything that was given a key anyway, with the key and the confidence it
    /// claimed. One entry here is worse than a point of weighted score.
    pub answered_anyway: Vec<(String, String, f64)>,
}

pub struct SyntheticReport {
    pub in_tune: SetScore,
    /// One per value in [`corpus::DETUNE_CENTS`].
    pub per_detune: Vec<SetScore>,
    pub detuned_all: SetScore,
    /// The detuned corpus with S2 skipped, which is the only way to say what
    /// the tuning stage is worth rather than assuming it.
    pub detuned_without_tuning: SetScore,
    pub tuning: Vec<TuningRecovery>,
    pub refusals: RefusalCheck,
    pub elapsed_ms: u64,
}

fn name_of(key: (u8, KeyMode)) -> String {
    use crate::analysis::camelot::Camelot;
    format!("{} ({})", Camelot::from_key(key.0, key.1).label(false), key.1.as_str())
}

/// Scores one list of specs through the key path.
fn score_set(label: &str, specs: &[Spec], tuning_stage: bool) -> Result<SetScore> {
    let mut pairs: Vec<Pair> = Vec::with_capacity(specs.len());
    let mut flagged_ambiguous = Vec::new();
    let mut misses = Vec::new();

    for spec in specs {
        let Some(reference) = spec.reference_key() else { continue };
        let item = spec.render();
        let run = if tuning_stage {
            key_path::run(&item.samples, spec.sample_rate, None)?
        } else {
            key_path::run_without_tuning(&item.samples, spec.sample_rate, None)?
        };
        let d = &run.detection;
        let estimate = (d.result.root, d.result.mode);
        pairs.push((reference, estimate));
        if d.ambiguous {
            flagged_ambiguous.push(spec.name.clone());
        }
        let category = mirex::classify(estimate, reference);
        if category != mirex::Category::Correct {
            misses.push(Miss {
                name: spec.name.clone(),
                reference: name_of(reference),
                estimate: name_of(estimate),
                category,
                confidence: d.result.confidence,
            });
        }
    }

    Ok(SetScore {
        label: label.to_string(),
        score: mirex::score(&pairs),
        flagged_ambiguous,
        misses,
    })
}

/// S2's recovery of one known detune.
fn tuning_recovery(detune_cents: f32, specs: &[Spec]) -> Result<TuningRecovery> {
    use crate::analysis::spectral::tuning;
    let mut max_abs = 0.0f32;
    let mut sum_abs = 0.0f32;
    let mut corrected = 0usize;
    let mut n = 0usize;
    for spec in specs {
        if spec.reference_key().is_none() {
            continue;
        }
        let item = spec.render();
        let est = tuning::estimate(&item.samples, spec.sample_rate)?;
        let err = (est.cents - detune_cents).abs();
        max_abs = max_abs.max(err);
        sum_abs += err;
        if est.needs_correction {
            corrected += 1;
        }
        n += 1;
    }
    Ok(TuningRecovery {
        detune_cents,
        n,
        max_abs_error_cents: max_abs,
        mean_abs_error_cents: if n == 0 { 0.0 } else { sum_abs / n as f32 },
        corrected,
    })
}

fn refusal_check(specs: &[Spec]) -> Result<RefusalCheck> {
    let mut check = RefusalCheck { n: 0, refused: 0, answered_anyway: Vec::new() };
    for spec in specs {
        if spec.truth != Truth::NoKey {
            continue;
        }
        check.n += 1;
        let item = spec.render();
        let run = key_path::run(&item.samples, spec.sample_rate, None)?;
        if run.detection.ambiguous {
            check.refused += 1;
        } else {
            check.answered_anyway.push((
                spec.name.clone(),
                run.detection.result.camelot.clone(),
                run.detection.result.confidence,
            ));
        }
    }
    Ok(check)
}

/// The whole synthetic run.
pub fn run() -> Result<SyntheticReport> {
    let started = Instant::now();
    let in_tune_specs = corpus::keyed(0.0);
    let in_tune = score_set("in tune", &in_tune_specs, true)?;

    let mut per_detune = Vec::new();
    let mut tuning = Vec::new();
    for cents in corpus::DETUNE_CENTS {
        let specs = corpus::keyed(cents);
        per_detune.push(score_set(&format!("detuned {cents:+.0} cents"), &specs, true)?);
        tuning.push(tuning_recovery(cents, &specs)?);
    }

    let all_detuned = corpus::detuned();
    let detuned_all = score_set("detuned, both offsets", &all_detuned, true)?;
    let detuned_without_tuning =
        score_set("detuned, S2 skipped", &all_detuned, false)?;

    let refusals = refusal_check(&corpus::unpitched())?;

    Ok(SyntheticReport {
        in_tune,
        per_detune,
        detuned_all,
        detuned_without_tuning,
        tuning,
        refusals,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}
