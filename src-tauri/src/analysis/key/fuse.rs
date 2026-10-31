//! Probability fusion, confidence, the runner-up, and the ambiguity flag.
//!
//! This file is where the product promise lives. v1 does not expect to beat
//! Mixed In Key's published accuracy; what it does instead is refuse to show a
//! confident wrong badge. Everything below exists to make "we are not sure"
//! a first-class answer rather than a low number nobody looks at.

use crate::analysis::camelot::Camelot;
use crate::analysis::key::skey::{key_to_label, label_to_key, KeyProbabilities};
use crate::error::Result;
use crate::models::{ChunkKey, KeyMode, KeyResult};

#[derive(Debug, Clone, Copy)]
pub struct FusionWeights {
    pub skey: f32,
    pub template: f32,
}

/// Starting point only. The real weights are fitted on the eval set, and the
/// harness exists before the detector precisely so this number is measured
/// rather than argued about.
pub const DEFAULT_WEIGHTS: FusionWeights = FusionWeights { skey: 0.6, template: 0.4 };

/// Below this, the UI shows the relative pair (`8A/8B`) instead of committing
/// to a letter.
///
/// This is the differentiator that does not depend on winning an accuracy
/// contest. Most residual error in every published system is **mode**, which is
/// exactly Camelot's A/B letter and the thing a DJ actually sees. A DJ would
/// rather see acknowledged ambiguity than a confident wrong letter, and the
/// incumbent does not offer that.
pub const MODE_AMBIGUITY_THRESHOLD: f32 = 0.6;

/// Spread (see `template::TemplateVote::spread`) at or below which the chroma
/// says nothing about key at all, and above which it says everything it is
/// going to. Between them the evidence term ramps linearly.
///
/// Measured through this pipeline on synthetic material: percussion and a
/// chromatic cluster sit at 0.02-0.05, diatonic progressions at 0.13-0.15. The
/// floor sits clear of the noise band rather than halfway between, because the
/// cost of calling a drum loop F minor is much higher than the cost of calling
/// a sparse intro ambiguous. Provisional until calibrated on a real corpus.
pub const SPREAD_FLOOR: f32 = 0.08;
pub const SPREAD_FULL: f32 = 0.14;

/// Below this the result is reported as ambiguous: no key badge, not a wrong
/// one. Percussive, atonal, live and rubato material is meant to land here.
pub const AMBIGUOUS_CONFIDENCE_FLOOR: f32 = 0.25;

/// The floor the chunk-agreement multiplier cannot go below. A track that
/// modulates is not unanalysable, so disagreement scales confidence down rather
/// than to nothing.
const AGREEMENT_FLOOR: f32 = 0.6;

/// Sums the two distributions under the weights, then renormalises — so a
/// missing voter is a no-op rather than a scaled-down distribution.
///
/// `skey` is optional so the template voter alone is still a complete engine,
/// which is what the eval harness needs in order to score the two voters
/// separately and decide whether the research track is funded at all.
pub fn fuse(
    skey: Option<&KeyProbabilities>,
    template: &KeyProbabilities,
    weights: &FusionWeights,
) -> KeyProbabilities {
    let mut p = [0.0f32; 24];
    let mut total = 0.0f32;
    for i in 0..24 {
        let mut v = weights.template.max(0.0) * template.p[i];
        if let Some(s) = skey {
            v += weights.skey.max(0.0) * s.p[i];
        }
        p[i] = v.max(0.0);
        total += p[i];
    }
    if total > 0.0 {
        for v in p.iter_mut() {
            *v /= total;
        }
    } else {
        p = [1.0 / 24.0; 24];
    }
    KeyProbabilities { p }
}

/// Below this much audio the chroma is a snapshot rather than a measurement,
/// and above the second figure it is as much of one as it is going to be.
///
/// S3 samples a 93 ms window every 371 ms, so a fifth of a second is **one
/// frame**: one moment of one chord, which can be as peaked as a whole track
/// and says nothing about what the track is in. Measured through this pipeline,
/// 0.2 s of a I-V-vi-IV progression came back at confidence 0.73-0.83 — a
/// confident answer from four chroma frames, in the one part of the product
/// whose promise is that it does not do that. S4 already states a 3 s minimum
/// for the model voter; the ramp spans it and reaches full support at 5 s, by
/// which point the vector covers a chord cycle rather than a chord.
pub const SUPPORT_ZERO_SECONDS: f32 = 1.0;
pub const SUPPORT_FULL_SECONDS: f32 = 5.0;

/// How much of an answer this much audio can support, in [0, 1].
///
/// A multiplier on the evidence term rather than a separate branch, which is
/// what makes a too-short input fall out of the pipeline the same way a drum
/// loop does: no evidence, so no runner-up is offered and the result is flagged
/// ambiguous instead of being given a badge nobody should trust.
pub fn duration_support(seconds: f32) -> f32 {
    if !seconds.is_finite() || seconds <= SUPPORT_ZERO_SECONDS {
        return 0.0;
    }
    ((seconds - SUPPORT_ZERO_SECONDS) / (SUPPORT_FULL_SECONDS - SUPPORT_ZERO_SECONDS))
        .clamp(0.0, 1.0)
}

/// Maps the spread of the candidate scores onto [0, 1].
pub fn evidence_from_spread(spread: f32) -> f32 {
    if !spread.is_finite() || spread <= SPREAD_FLOOR {
        return 0.0;
    }
    ((spread - SPREAD_FLOOR) / (SPREAD_FULL - SPREAD_FLOOR)).clamp(0.0, 1.0)
}

/// The relative key: the one sharing this key's Camelot number.
///
/// C major and A minor are 8B and 8A. That is what the spec means by showing
/// `8A/8B`, and it is also where a chroma-based detector's error actually is —
/// the two keys have identical pitch content and differ only in which degree is
/// weighted as the tonic.
pub fn relative_of(root: u8, mode: KeyMode) -> (u8, KeyMode) {
    match mode {
        KeyMode::Major => ((root + 9) % 12, KeyMode::Minor),
        KeyMode::Minor => ((root + 3) % 12, KeyMode::Major),
    }
}

/// How much of the probability on this key and its relative sits on this key.
///
/// The number the `8A/8B` display is gated on. Deliberately not the top-1
/// probability: top-1 can be comfortable while the relative pair splits it
/// almost evenly, and that is exactly the case a DJ must be warned about.
pub fn relative_confidence(p: &KeyProbabilities, root: u8, mode: KeyMode) -> f32 {
    let (rel_root, rel_mode) = relative_of(root, mode);
    pair_share(p, (root, mode), (rel_root, rel_mode))
}

/// The same question for the parallel key (C major against C minor). Not what
/// drives the badge — a parallel confusion does not render as one Camelot
/// number — but it is voter-disagreement evidence for the `Improve Tracks`
/// collection and the eval harness's parallel-error category.
pub fn parallel_confidence(p: &KeyProbabilities, root: u8, mode: KeyMode) -> f32 {
    let other = match mode {
        KeyMode::Major => KeyMode::Minor,
        KeyMode::Minor => KeyMode::Major,
    };
    pair_share(p, (root, mode), (root, other))
}

fn pair_share(p: &KeyProbabilities, a: (u8, KeyMode), b: (u8, KeyMode)) -> f32 {
    let pa = p.p[key_to_label(a.0, a.1)];
    let pb = p.p[key_to_label(b.0, b.1)];
    let total = pa + pb;
    if total <= 0.0 {
        return 0.0;
    }
    pa / total
}

/// A real confidence: three independent things have to hold.
///
/// `evidence` — the material is pitched enough to have a key at all.
/// `sharpness` — one candidate stands clear of the next, so it is not a
/// relative or fifth coin flip. `agreement` — the chunks agree, so the track
/// has one key rather than several averaged into a plausible-looking third.
///
/// Multiplicative on purpose. Any one of the three failing has to be able to
/// sink the result, because each of them failing on its own is a distinct way
/// of being wrong, and a weighted sum would let two strong terms carry a
/// percussive track to a confident answer.
pub fn confidence(p: &KeyProbabilities, evidence: f32, agreement: f32) -> f32 {
    let (_, top) = p.best();
    let (_, second) = p.runner_up();
    let agree = AGREEMENT_FLOOR + (1.0 - AGREEMENT_FLOOR) * agreement.clamp(0.0, 1.0);
    (evidence.clamp(0.0, 1.0) * separation(top, second).sqrt() * agree).clamp(0.0, 1.0)
}

/// How much better the winner is than the best alternative, scale-free.
///
/// `(p1 - p2) / (p1 + p2)` rather than the bare difference, because the absolute
/// mass on the winner depends on how peaked the softmax happens to be and not on
/// how decisive the match was: an unambiguous cadence lands around 0.54 simply
/// because a dominant-seventh progression also fits the dominant's own key a
/// little, and reading that as half-confidence would put a plainly diatonic
/// track behind the ambiguity threshold.
///
/// Square-rooted in [`confidence`] because the curve should be concave. The
/// difference between "tied with the relative" and "clearly the best" is what a
/// DJ needs to see; the difference between "clearly" and "overwhelmingly" is not
/// worth a third of the scale.
fn separation(top: f32, second: f32) -> f32 {
    let total = top + second;
    if !(total > 0.0) {
        return 0.0;
    }
    ((top - second) / total).clamp(0.0, 1.0)
}

/// Everything `to_result` needs, named rather than passed as six positionals.
pub struct Resolution<'a> {
    pub fused: &'a KeyProbabilities,
    pub evidence: f32,
    pub agreement: f32,
    pub tuning_cents: f32,
    pub chunk_keys: Vec<ChunkKey>,
}

/// Builds the stored result: top key, Camelot label, confidence, runner-up, and
/// the ambiguity flag. Voter disagreement here is also what drives the spec's
/// `Improve Tracks` collection and the `file can be improved` message.
pub fn to_result(r: Resolution<'_>) -> Result<KeyResult> {
    let (best, _) = r.fused.best();
    let (root, mode) = label_to_key(best);
    let conf = confidence(r.fused, r.evidence, r.agreement);
    // No runner-up when there was no evidence: offering an alternative implies
    // a second-best reading existed, and for silence or a drum loop it did not.
    let (alt_root, alt_mode) = if r.evidence > 0.0 {
        let (alt, _) = r.fused.runner_up();
        let (ar, am) = label_to_key(alt);
        (Some(ar), Some(am))
    } else {
        (None, None)
    };
    let mode_ambiguous = relative_confidence(r.fused, root, mode) < MODE_AMBIGUITY_THRESHOLD
        || conf < AMBIGUOUS_CONFIDENCE_FLOOR;
    Ok(KeyResult {
        root,
        mode,
        camelot: Camelot::from_key(root, mode).label(false),
        confidence: conf as f64,
        alt_root,
        alt_mode,
        mode_ambiguous,
        tuning_cents: r.tuning_cents as f64,
        chunk_keys: r.chunk_keys,
    })
}

#[cfg(test)]
#[path = "fuse_tests.rs"]
mod tests;
