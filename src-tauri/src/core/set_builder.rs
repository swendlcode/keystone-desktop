//! What to play next, and a set built out of those answers.
//!
//! Three things decide a transition and they are not interchangeable:
//!
//! * **Key.** Camelot's wheel: the same segment, its relative across the
//!   ring, or one step either way. `Camelot::compatible` already owns that
//!   rule and is not restated here.
//! * **Tempo.** A drift, as a fraction rather than in BPM, because six per
//!   cent of 128 and six per cent of 174 are different numbers of beats and
//!   the same amount of pitch.
//! * **Energy.** A set goes up or holds; it does not fall three levels
//!   without being asked to.
//!
//! Key and tempo are **gates** and energy is a **preference**. A pair outside
//! the tempo drift is not a worse transition, it is a different one requiring
//! a trick this app knows nothing about, so it is not offered. An energy drop
//! is merely unusual, so it scores lower and still appears — a DJ taking the
//! room down on purpose is making a choice, not a mistake.
//!
//! Nothing here reads the database or writes anything.

use crate::analysis::camelot::Camelot;
use crate::models::Track;

/// Default tempo window either side, as a fraction. Six per cent is about
/// eight BPM at 128, which is roughly where a pitch fader stops being
/// invisible on vocals.
pub const DEFAULT_DRIFT: f64 = 0.06;

/// How a candidate scored, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub track_id: String,
    pub score: f64,
    pub key_relation: &'static str,
    pub bpm_change: f64,
    pub energy_change: i32,
}

/// Everything a caller can steer.
#[derive(Debug, Clone)]
pub struct Rules {
    pub drift: f64,
    /// Require a key relation from the wheel. Off lets an unanalysed or
    /// atonal track through, which a percussive tool or an intro may be.
    pub require_key: bool,
    /// Prefer energy that rises. False scores a hold highest instead, for the
    /// middle of a set that should sit still.
    pub climbing: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            drift: DEFAULT_DRIFT,
            require_key: true,
            climbing: true,
        }
    }
}

/// Scores every candidate against `from`, best first. A track that fails a
/// gate is absent rather than present with a low score.
pub fn rank(from: &Track, pool: &[Track], rules: &Rules) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = pool
        .iter()
        .filter(|t| t.id != from.id)
        .filter_map(|t| score(from, t, rules))
        .collect();
    // Ties broken by id so a set built twice from the same library is the
    // same set. Floating point scores tie more often than they look like
    // they would — two tracks at the same tempo and key score identically.
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.track_id.cmp(&b.track_id))
    });
    out
}

/// One transition, or `None` when a gate refuses it.
pub fn score(from: &Track, to: &Track, rules: &Rules) -> Option<Candidate> {
    let (a_bpm, b_bpm) = (tempo(from)?, tempo(to)?);
    let change = (b_bpm - a_bpm) / a_bpm;
    if change.abs() > rules.drift {
        return None;
    }

    let (relation, key_score) = match relation(from, to) {
        Some(found) => found,
        None if rules.require_key => return None,
        // Unknown either side: allowed, but it never outranks a real match.
        None => ("unknown", 0.3),
    };

    // Closeness in tempo, 1.0 at no change down to 0 at the edge of the
    // window. A set that holds its tempo is easier to mix than one that walks
    // to the edge of what is possible on every transition.
    let tempo_score = 1.0 - (change.abs() / rules.drift).min(1.0);
    let step = energy_change(from, to);
    Some(Candidate {
        track_id: to.id.clone(),
        score: 0.5 * key_score + 0.3 * tempo_score + 0.2 * energy_score(step, rules.climbing),
        key_relation: relation,
        bpm_change: round2(b_bpm - a_bpm),
        energy_change: step,
    })
}

/// Builds a chain, each link the best available from the one before.
///
/// Greedy on purpose. A set is played in order and a DJ choosing the next
/// record does not know what they will reach for after it, so a globally
/// optimal path would be optimising something nobody experiences — and a
/// chain that reshuffles its opening because of its ending is not a set
/// somebody can learn.
///
/// Stops when the target length is reached or nothing compatible is left, and
/// says which.
pub fn build(seed: &Track, pool: &[Track], minutes: f64, rules: &Rules) -> Chain {
    let target_ms = (minutes * 60_000.0).max(0.0);
    let mut chain = Chain {
        order: vec![seed.id.clone()],
        transitions: Vec::new(),
        total_ms: seed.duration_ms.unwrap_or(0),
        ran_out: false,
    };
    let mut used: std::collections::HashSet<String> = std::iter::once(seed.id.clone()).collect();
    let mut current = seed.clone();

    while chain.total_ms < target_ms as i64 {
        let remaining: Vec<Track> = pool
            .iter()
            .filter(|t| !used.contains(&t.id))
            .cloned()
            .collect();
        let Some(next) = rank(&current, &remaining, rules).into_iter().next() else {
            chain.ran_out = true;
            break;
        };
        let Some(track) = pool.iter().find(|t| t.id == next.track_id) else {
            chain.ran_out = true;
            break;
        };
        chain.total_ms += track.duration_ms.unwrap_or(0);
        used.insert(track.id.clone());
        chain.order.push(track.id.clone());
        chain.transitions.push(next);
        current = track.clone();
    }
    chain
}

#[derive(Debug, Clone)]
pub struct Chain {
    pub order: Vec<String>,
    /// One per transition, so `order[n + 1]` was chosen by `transitions[n]`.
    pub transitions: Vec<Candidate>,
    pub total_ms: i64,
    /// True when the pool ran dry before the target length.
    pub ran_out: bool,
}

fn tempo(track: &Track) -> Option<f64> {
    track
        .tempo_display
        .or(track.tempo_raw)
        .filter(|b| b.is_finite() && *b > 0.0)
}

/// The wheel relation between two tracks, and what it is worth.
fn relation(from: &Track, to: &Track) -> Option<(&'static str, f64)> {
    let a = Camelot::parse(from.camelot.as_deref()?)?;
    let b = Camelot::parse(to.camelot.as_deref()?)?;
    if a == b {
        return Some(("same key", 1.0));
    }
    let [_, relative, down, up] = a.compatible();
    if b == relative {
        return Some(("relative major or minor", 0.85));
    }
    if b == up {
        return Some(("one step up the wheel", 0.8));
    }
    if b == down {
        return Some(("one step down the wheel", 0.8));
    }
    None
}

fn energy_change(from: &Track, to: &Track) -> i32 {
    match (from.energy, to.energy) {
        (Some(a), Some(b)) => i32::from(b) - i32::from(a),
        _ => 0,
    }
}

/// A rise of one is the best thing a set can do; a hold is next. A fall is
/// allowed and ranked by how far it falls.
fn energy_score(step: i32, climbing: bool) -> f64 {
    let ideal = if climbing { 1 } else { 0 };
    let distance = (step - ideal).abs();
    (1.0 - 0.25 * f64::from(distance)).max(0.0)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
#[path = "set_builder_tests.rs"]
mod tests;
