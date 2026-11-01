//! S4 — key, by two voters fused.
//!
//! The two voters are kept deliberately unalike: a CNN over a variable-Q
//! transform and a cosine match over a whitened HPCP fail on different tracks,
//! and uncorrelated error profiles are the whole reason the ensemble is the
//! cheapest real accuracy gain available to us.
//!
//! Only the template voter is live. The model voter needs a bundled graph that
//! is not in the tree yet, so [`skey`] is still unimplemented and
//! [`detect`] takes it as an `Option`. That is not a placeholder arrangement —
//! the eval harness has to be able to score each voter alone in order to decide
//! whether the research track is funded at all (ENGINE.md 7.1), so the
//! template-only path is a supported configuration, not a degraded one.
//!
//! What this stage resolves, in order: per-chunk votes, the aggregate they
//! imply, fusion with the model voter when there is one, then one honest
//! confidence and the ambiguity flag.

pub mod chunks;
pub mod fuse;
pub mod profiles;
pub mod skey;
pub mod template;

#[cfg(test)]
mod camelot_tests;
#[cfg(test)]
mod duration_tests;
#[cfg(test)]
mod refusal_tests;
#[cfg(test)]
mod signal_tests;

use crate::analysis::key::profiles::ProfileId;
use crate::analysis::key::skey::KeyProbabilities;
use crate::analysis::spectral::chroma::Pcp;
use crate::error::Result;
use crate::models::KeyResult;

/// Everything the key stage reads. Takes borrowed audio because the caller owns
/// the one decoded buffer and must not clone 200 MB to pass it in.
pub struct KeyInput<'a> {
    /// The whole-track PCP from S3. Used as the fallback for inputs shorter
    /// than one chunk; the chunk votes are what normally decide.
    pub pcp: &'a Pcp,
    /// Tuned mono at the analysis rate. The template voter's per-chunk pass
    /// needs the audio, not only the aggregate vector.
    pub mono: &'a [f32],
    pub sample_rate: u32,
    /// Tuned mono at 22,050 Hz. The model's rate is fixed by its graph.
    pub mono_22050: &'a [f32],
    /// S2's measured offset for the *file*. Reported to the UI, which shows it,
    /// and never used to correct anything here.
    pub tuning_cents: f32,
    /// The offset still present in `mono` and `mono_22050`: S2's estimate when
    /// the caller left them alone, **zero** when it already resampled them.
    ///
    /// Two fields rather than one because they answer different questions, and
    /// collapsing them breaks one of the two. Correcting against a residual of
    /// zero on an uncorrected buffer leaves the detune in the chroma; reporting
    /// the residual instead of the measurement tells a user with a vinyl rip
    /// that their track is at A440.
    pub residual_cents: f32,
}

/// Everything the stage worked out, not only what the track row stores.
///
/// The extra fields are not debug noise. `ambiguous` is the answer the UI needs
/// in order to show no badge rather than a wrong one; `evidence`,
/// `chunk_agreement` and the two voters' own distributions are what the eval
/// harness scores and what the `Improve Tracks` collection is built from.
pub struct KeyDetection {
    pub result: KeyResult,
    /// The material did not support a confident answer. Percussive, atonal,
    /// live and rubato tracks are meant to land here.
    pub ambiguous: bool,
    pub evidence: f32,
    /// The raw gate input: the template voter's score spread, weighted-meaned
    /// over the chunks that carried a vote. Carried out of the stage because
    /// the confidence gates are fitted on its distribution, and a constant
    /// fitted on a number nobody can dump is a constant nobody can refit.
    pub spread: f32,
    /// How much of an answer this much audio can support, in [0, 1]. The other
    /// factor in `evidence`, separated from the spread term so a refusal caused
    /// by a 4 s file is distinguishable from one caused by flat chroma.
    pub duration_support: f32,
    /// No chunk carried a vote, so the whole-track PCP decided. Part of
    /// detection rather than of the confidence model, which is why the chunk
    /// gate it depends on is held fixed.
    pub whole_track_fallback: bool,
    /// The whole-track PCP's own spread, computed whether or not it decided.
    ///
    /// Carried separately because `spread` above is **gated**: when any chunk
    /// clears the chunk-weight floor it is a weighted mean over those chunks
    /// and so cannot fall below the floor, and when none does it is this
    /// number instead. That makes the gated figure bimodal by construction and
    /// useless as the input to a fitted curve. This one is measured on every
    /// track by the same code path.
    pub whole_track_spread: f32,
    /// Every chunk's own spread, ungated and in time order.
    pub chunk_spreads: Vec<f32>,
    pub chunk_agreement: f32,
    pub relative_confidence: f32,
    pub parallel_confidence: f32,
    pub best_profile: ProfileId,
    pub template: KeyProbabilities,
    pub skey: Option<KeyProbabilities>,
    pub fused: KeyProbabilities,
}

/// Runs both voters, fuses them, maps to Camelot and sets the ambiguity flag.
pub fn detect(input: &KeyInput<'_>, skey: Option<&skey::SkeySession>) -> Result<KeyResult> {
    Ok(detect_detailed(input, skey)?.result)
}

/// What the template voter made of the whole track.
struct TrackVote {
    distribution: KeyProbabilities,
    spread: f32,
    agreement: f32,
    chunk_keys: Vec<crate::models::ChunkKey>,
    best_profile: ProfileId,
    whole_track_fallback: bool,
    whole_track_spread: f32,
    chunk_spreads: Vec<f32>,
}

/// Resolves the template voter's per-chunk votes into one track-level answer.
///
/// The chunk aggregate, not the whole-track PCP, is the track's answer when
/// there is one. Both cover the same audio, but the aggregate weights each 15 s
/// window by how much it had to say, so a percussive drop contributes nothing
/// instead of smearing the vector every tonal section contributed to. The
/// whole-track PCP is the fallback for inputs too short to chunk, and for
/// material where no chunk cleared the evidence floor.
fn template_vote(input: &KeyInput<'_>, profiles: &profiles::ProfileSet) -> Result<TrackVote> {
    let whole = template::match_pcp(input.pcp, profiles)?;
    let votes = chunks::vote(input.mono, input.sample_rate, input.residual_cents, profiles)?;
    let (distribution, spread, agreement, fallback) = match chunks::aggregate(&votes) {
        Some(a) => (a.probabilities, a.spread, a.agreement, false),
        None => (whole.probabilities, whole.spread, 1.0, true),
    };
    Ok(TrackVote {
        distribution,
        spread,
        agreement,
        chunk_keys: chunks::to_chunk_keys(&votes),
        best_profile: whole.best_profile,
        whole_track_fallback: fallback,
        whole_track_spread: whole.spread,
        chunk_spreads: votes.iter().map(|v| v.spread).collect(),
    })
}

/// As [`detect`], keeping the working.
pub fn detect_detailed(
    input: &KeyInput<'_>,
    skey: Option<&skey::SkeySession>,
) -> Result<KeyDetection> {
    let profiles = profiles::shipping_set();
    let vote = template_vote(input, &profiles)?;

    let model = match skey {
        Some(session) => Some(session.infer(input.mono_22050)?.0),
        None => None,
    };
    let fused = fuse::fuse(model.as_ref(), &vote.distribution, &fuse::DEFAULT_WEIGHTS);

    // Evidence comes from the template voter's score spread, scaled by how much
    // audio there was to spread over. When the model voter lands it brings its
    // own notion of how pitched the input was, and this term has to be refitted
    // over both rather than left reading one.
    //
    // The **whole-track** spread, not the chunk aggregate's. Both measure the
    // same thing on the same audio, but the aggregate's is a weighted mean over
    // the chunks that cleared `chunks::CHUNK_WEIGHT_FLOOR` and so cannot fall
    // below that floor, while a track where none cleared it reports the
    // whole-track figure instead. That makes the aggregate bimodal by
    // construction — an artefact of a detection gate showing up inside the
    // confidence — and on the 2026-11-01 fit half it is the difference between
    // a confidence that orders real tracks by whether they are right (AUC 0.69)
    // and one that does not (0.51).
    let seconds = if input.sample_rate == 0 {
        0.0
    } else {
        input.mono.len() as f32 / input.sample_rate as f32
    };
    let support = fuse::duration_support(seconds);
    let evidence = fuse::evidence_from_spread(vote.whole_track_spread) * support;
    let result = fuse::to_result(fuse::Resolution {
        fused: &fused,
        evidence,
        agreement: vote.agreement,
        tuning_cents: input.tuning_cents,
        chunk_keys: vote.chunk_keys,
    })?;

    let ambiguous =
        evidence <= 0.0 || (result.confidence as f32) < fuse::AMBIGUOUS_CONFIDENCE_FLOOR;
    Ok(KeyDetection {
        relative_confidence: fuse::relative_confidence(&fused, result.root, result.mode),
        parallel_confidence: fuse::parallel_confidence(&fused, result.root, result.mode),
        result,
        ambiguous,
        evidence,
        spread: vote.spread,
        duration_support: support,
        whole_track_fallback: vote.whole_track_fallback,
        whole_track_spread: vote.whole_track_spread,
        chunk_spreads: vote.chunk_spreads,
        chunk_agreement: vote.agreement,
        best_profile: vote.best_profile,
        template: vote.distribution,
        skey: model,
        fused,
    })
}
