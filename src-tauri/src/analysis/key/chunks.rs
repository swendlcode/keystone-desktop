//! Per-chunk key inference, and the aggregate the track key is resolved from.
//!
//! Chunking is not an optimisation. Three things come out of it that a single
//! whole-track pass cannot give:
//!
//! 1. **A key change at the drop does not poison the track.** A whole-track
//!    chroma is one vector, so sixteen bars in the relative major are simply
//!    mixed into it; sixteen chunks are sixteen votes, and the ones that
//!    disagree lower the confidence instead of moving the answer.
//! 2. **Sections with nothing to say are weighted out.** A percussive drop
//!    contributes a flat chroma. Aggregated as a vector it smears the tonal
//!    sections; aggregated as a vote weighted by its own spread it contributes
//!    nothing, which is correct.
//! 3. **Per-region key for free**, which is what the deferred deck view reads
//!    and what `chunk_keys` stores.
//!
//! 15 s because that is where the model voter's accuracy stopped climbing
//! (3 s 30%, 5 s 50%, 10 s 67%, 15 s 100% on a single test track - treat the
//! figure as a hypothesis for the eval harness, but the window length is the
//! one both voters should share so their chunks line up).

use crate::analysis::key::fuse;
use crate::analysis::key::profiles::ProfileSet;
use crate::analysis::key::skey::{label_to_key, KeyProbabilities};
use crate::analysis::key::template;
use crate::analysis::spectral::chroma;
use crate::error::Result;
use crate::models::ChunkKey;

pub const CHUNK_SECONDS: f32 = 15.0;

/// The gate on a chunk's own spread that decides whether it votes, and with
/// what weight.
///
/// **Deliberately not the confidence gate.** These are the original
/// synthetic-derived numbers that `fuse::SPREAD_FLOOR` and `SPREAD_FULL` used
/// to carry, kept here under their own name because this gate chooses which
/// chunks enter the aggregate and therefore **decides the key**, while the
/// confidence gate only decides how loudly the answer is stated. The 2026-11-01
/// calibration refit the second and left the first exactly where it was, so
/// that run changed no estimate on any of the 1,063 tracks and the agreement
/// figures in `docs/ACCURACY.md` §3 still describe this engine.
///
/// That does not make it right, and it is probably the next thing to look at:
/// on the fit half no chunk cleared this floor on 63.7% of tracks, so the
/// whole-track PCP decided those and the chunk machinery contributed nothing
/// at all. Moving it is a detection change and needs its own measurement.
pub const CHUNK_WEIGHT_FLOOR: f32 = 0.08;
pub const CHUNK_WEIGHT_FULL: f32 = 0.14;

/// How much this chunk's vote counts, in [0, 1]. See [`CHUNK_WEIGHT_FLOOR`].
pub fn chunk_weight(spread: f32) -> f32 {
    if !spread.is_finite() || spread <= CHUNK_WEIGHT_FLOOR {
        return 0.0;
    }
    ((spread - CHUNK_WEIGHT_FLOOR) / (CHUNK_WEIGHT_FULL - CHUNK_WEIGHT_FLOOR)).clamp(0.0, 1.0)
}

/// Below this a chunk has too few chroma frames to mean anything — at hop
/// 16384 a 3 s window is eight frames.
pub const MIN_CHUNK_SECONDS: f32 = 3.0;

#[derive(Debug, Clone)]
pub struct ChunkVote {
    pub start_ms: f64,
    pub end_ms: f64,
    pub probabilities: KeyProbabilities,
    /// See [`template::TemplateVote::spread`]. Doubles as the chunk's weight.
    pub spread: f32,
}

#[derive(Debug, Clone)]
pub struct Aggregate {
    pub probabilities: KeyProbabilities,
    /// Weighted mean spread, i.e. how much the track as a whole had to say.
    pub spread: f32,
    /// Weighted fraction of chunks whose own winner is the aggregate winner.
    /// One on a single-key track, and it falls as soon as a section modulates.
    pub agreement: f32,
}

/// Splits `samples` into 15 s chunks and votes on each.
///
/// A trailing remainder shorter than [`MIN_CHUNK_SECONDS`] is folded into the
/// previous chunk rather than voted on separately: a 2 s tail of reverb is not a
/// region of the track and giving it an equal vote with a full window is how a
/// fade-out ends up outvoting a chorus.
pub fn vote(
    samples: &[f32],
    sample_rate: u32,
    residual_cents: f32,
    profiles: &ProfileSet,
) -> Result<Vec<ChunkVote>> {
    if samples.is_empty() || sample_rate == 0 {
        return Ok(Vec::new());
    }
    let rate = sample_rate as f64;
    let chunk = (sample_rate as f32 * CHUNK_SECONDS) as usize;
    let minimum = (sample_rate as f32 * MIN_CHUNK_SECONDS) as usize;
    if chunk == 0 || samples.len() < minimum {
        return Ok(Vec::new());
    }
    let mut bounds: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    while start < samples.len() {
        let end = (start + chunk).min(samples.len());
        bounds.push((start, end));
        start = end;
    }
    if bounds.len() > 1 {
        let (last_start, last_end) = bounds[bounds.len() - 1];
        if last_end - last_start < minimum {
            bounds.pop();
            let n = bounds.len() - 1;
            bounds[n].1 = last_end;
        }
    }

    let mut votes = Vec::with_capacity(bounds.len());
    for (lo, hi) in bounds {
        let pcp = chroma::pcp(&samples[lo..hi], sample_rate, residual_cents)?;
        let v = template::match_pcp(&pcp, profiles)?;
        votes.push(ChunkVote {
            start_ms: lo as f64 / rate * 1000.0,
            end_ms: hi as f64 / rate * 1000.0,
            probabilities: v.probabilities,
            spread: v.spread,
        });
    }
    Ok(votes)
}

/// Combines the chunk votes, each weighted by how much it had to say.
///
/// Returns `None` when no chunk cleared [`CHUNK_WEIGHT_FLOOR`] — percussion,
/// silence, an atonal ambient wash. The caller must treat that as "no answer"
/// rather than falling through to an unweighted mean, which would manufacture
/// confidence out of noise.
pub fn aggregate(votes: &[ChunkVote]) -> Option<Aggregate> {
    let mut p = [0.0f32; 24];
    let mut total_weight = 0.0f32;
    let mut weighted_spread = 0.0f32;
    for v in votes {
        let w = chunk_weight(v.spread);
        if w <= 0.0 {
            continue;
        }
        for (slot, q) in p.iter_mut().zip(v.probabilities.p.iter()) {
            *slot += w * *q;
        }
        weighted_spread += w * v.spread;
        total_weight += w;
    }
    if total_weight <= 0.0 {
        return None;
    }
    for slot in p.iter_mut() {
        *slot /= total_weight;
    }
    let probabilities = KeyProbabilities { p };
    let (winner, _) = probabilities.best();
    let mut agreeing = 0.0f32;
    for v in votes {
        let w = chunk_weight(v.spread);
        if w <= 0.0 {
            continue;
        }
        if v.probabilities.best().0 == winner {
            agreeing += w;
        }
    }
    Some(Aggregate {
        probabilities,
        spread: weighted_spread / total_weight,
        agreement: agreeing / total_weight,
    })
}

/// The per-chunk rows stored on the track. Not displayed in v1 — the deck view
/// and any future key-change timeline read them, and they cost nothing now.
pub fn to_chunk_keys(votes: &[ChunkVote]) -> Vec<ChunkKey> {
    votes
        .iter()
        .map(|v| {
            let (best, _) = v.probabilities.best();
            let (root, mode) = label_to_key(best);
            let evidence = fuse::evidence_from_spread(v.spread);
            ChunkKey {
                start_ms: v.start_ms,
                end_ms: v.end_ms,
                root,
                mode,
                // A chunk's own confidence, with agreement held at 1: a single
                // chunk cannot disagree with itself, and the track-level
                // agreement penalty belongs on the track, not on every row.
                confidence: fuse::confidence(&v.probabilities, evidence, 1.0) as f64,
            }
        })
        .collect()
}
