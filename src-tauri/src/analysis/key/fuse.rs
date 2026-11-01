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
///
/// **Fitted 2026-11-01** on the fit half (532 tracks) of the Mixed In Key
/// corpus described in `docs/ACCURACY.md` §1. It was 0.6, which fired on 4.5%
/// of real tracks while the letter was still only 66% right up to 0.80: mode
/// agreement runs 62.5% below 0.60, 65.6% in [0.60, 0.70), 69.1% in [0.70,
/// 0.80) and 88.0% at or above 0.80. The gate goes where the letter becomes
/// trustworthy, not where the split becomes grotesque. Refitting means
/// re-running that band table, and a profile or `SOFTMAX_TEMPERATURE` change
/// invalidates it.
pub const MODE_AMBIGUITY_THRESHOLD: f32 = 0.80;

/// Spread (see `template::TemplateVote::spread`) at or below which the chroma
/// says nothing about key at all, and above which it says everything it is
/// going to. Between them the evidence term ramps linearly.
///
/// **Fitted 2026-11-01** on the fit half (532 tracks) of the corpus in
/// `docs/ACCURACY.md` §1, against the whole-track spread, which is what
/// `key::detect_detailed` passes in. They were 0.08 and 0.14, measured on
/// synthetic diatonic progressions, and real records reach neither: the fit
/// half runs min 0.0092, median 0.0370, p95 0.0773, max 0.1200, and full club
/// tracks min 0.0125, median 0.0456, max 0.0961. A 0.08 floor therefore zeroed
/// the evidence term — and with it the confidence — on 88% of a real library.
///
/// The floor is set by a constraint, not by the fit: it must stay above the
/// spread synthetic percussion reaches through this same pipeline, because
/// refusing a drum loop is a product promise and not a tuning target. Measured
/// at eleven durations from 8 s to 120 s that band is **0.0117-0.0132** and does
/// not shrink with more audio, so 0.014 is the smallest round number clear of it.
///
/// **The two bands overlap, so the floor is a price and not a separation.** The
/// thinnest real full track on the fit half sits at 0.0125, inside the
/// percussion band, so 0.014 hard-refuses 6.0% of real tracks (1.7% of full
/// ones) to keep the drum-loop promise — and a uniform chromatic wash, at
/// 0.0254, is the real distribution's 20th percentile and is not separated at
/// all. See [`AMBIGUOUS_CONFIDENCE_FLOOR`] for what that costs.
///
/// `SPREAD_FULL` is fitted: AUC against exact agreement is flat (0.685-0.688
/// pooled) from 0.030 to 0.040 and falls away above, so 0.040 is the top of
/// that plateau and its exact value is not load-bearing.
pub const SPREAD_FLOOR: f32 = 0.014;
pub const SPREAD_FULL: f32 = 0.040;

/// Below this the result is reported as ambiguous: no key badge, not a wrong
/// one. Percussive, atonal, live and rubato material is meant to land here.
///
/// **Fitted 2026-11-01** on the fit half (532 tracks) of the corpus in
/// `docs/ACCURACY.md` §1, against one stated criterion: a Camelot badge earns
/// its place when mixing on it works, which is the wheel's own rule — the same
/// code, one step either way with the same letter, or the relative. The floor
/// is the largest value at which the set it refuses is still **below even** on
/// that measure, so nothing useful is thrown away. The refused set is 46.8%
/// mixable at 0.08, 47.8% at 0.10, 49.0% at 0.11 and 50.5% at 0.12, where it
/// crosses. The curve is flat across all of that, so this is a choice made on a
/// criterion and not a peak found in the data.
///
/// It was 0.25, fitted on nothing. One consequence of moving it is known and
/// not hidden: a uniform chromatic wash reaches 0.20-0.26 through the refit
/// gates (20 s to 120 s) and is now shown a key, which the old floor refused by
/// accident rather than by design. Restoring that refusal through
/// `SPREAD_FLOOR` needs 0.026, costing 15.3% of full tracks their badge instead
/// of 1.7%. The fix belongs in the front end:
/// `template::match_chroma` standardises its similarities, so a near-flat
/// chroma still yields a peaked distribution and neither the separation nor the
/// top probability can tell it from a decision.
pub const AMBIGUOUS_CONFIDENCE_FLOOR: f32 = 0.12;

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
///
/// The **confidence** evidence term and nothing else: `chunks` has its own gate
/// on the same quantity, left on the synthetic constants because that one picks
/// which chunks vote and so belongs to detection.
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
