//! Step 5: coverage, spacing, and filling the slots a rule left empty.
//!
//! Eight markers clustered inside one section are worse than four spread across
//! the track, even when every one of them is a correct detection — a DJ loads a
//! track to find places to come in, and three of those places being the same
//! sixteen bars answers nothing. So the core is partitioned into eight spans and
//! the set is rebalanced against that partition before anything is returned.

use crate::analysis::cues::{CueCandidate, MIN_SPACING_BARS, N_SLOTS};
use crate::analysis::cues::assign::{cue, FALLBACK_CONFIDENCE};
use crate::analysis::cues::context::Placement;
use crate::analysis::structure::grid::Grid;
use crate::models::DetectedCue;

/// Index of the eighth of the core a position falls in.
pub fn span_of(ms: f64, core: (f64, f64)) -> usize {
    let width = (core.1 - core.0) / N_SLOTS as f64;
    if width <= 0.0 {
        return 0;
    }
    (((ms - core.0) / width).floor().max(0.0) as usize).min(N_SLOTS - 1)
}

pub fn min_spacing_ms(grid: &Grid) -> f64 {
    MIN_SPACING_BARS as f64 * grid.bar_ms
}

/// The spacing actually enforced on this track.
///
/// Eight markers 8 bars apart need 64 bars of core, and plenty of real tracks
/// are shorter than that — a 3-minute edit at 128 BPM has about 96 bars, an
/// intro-less radio cut far fewer. Returning fewer than eight markers is not an
/// option the spec leaves open, so a short core gets the widest spacing that
/// still fits eight rather than a violated promise or a missing CUE 7.
pub fn effective_spacing_ms(grid: &Grid, core: (f64, f64)) -> f64 {
    let available = ((core.1 - core.0) / N_SLOTS as f64).max(0.0);
    min_spacing_ms(grid).min(available)
}

/// Fills empty slots, then redistributes duplicates out of crowded spans.
pub fn enforce(
    slots: [Option<DetectedCue>; N_SLOTS],
    candidates: &[CueCandidate],
    grid: &Grid,
    core: (f64, f64),
) -> Vec<DetectedCue> {
    let mut slots = slots;
    backfill(&mut slots, candidates, grid, core);
    redistribute(&mut slots, candidates, grid, core);
    // Spacing runs last and may undo a little of the coverage pass. That order
    // is deliberate: coverage is a goal, the minimum spacing is a promise, and
    // two markers a bar apart are a worse answer than two spans sharing one.
    enforce_spacing(&mut slots, candidates, grid, core);
    slots.into_iter().flatten().collect()
}

/// Pulls apart any pair closer than [`effective_spacing_ms`], moving the less
/// confident of the two.
fn enforce_spacing(
    slots: &mut [Option<DetectedCue>; N_SLOTS],
    candidates: &[CueCandidate],
    grid: &Grid,
    core: (f64, f64),
) {
    let floor = effective_spacing_ms(grid, core);
    if floor <= 0.0 {
        return;
    }
    // Bounded rather than looped to convergence: each move can create a new
    // violation, and a track with no room left must terminate with the best set
    // it has rather than churn.
    for _ in 0..N_SLOTS * 2 {
        let Some(victim) = weakest_of_closest_pair(slots, floor) else {
            return;
        };
        let occupied: Vec<f64> = slots
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != victim)
            .filter_map(|(_, c)| c.as_ref().map(|c| c.position_ms))
            .collect();
        let Some(placement) = free_position(candidates, grid, core, &occupied, floor) else {
            return;
        };
        slots[victim] = Some(cue(victim, placement, grid));
    }
}

/// The lower-confidence member of the closest offending pair, or `None` when the
/// set already satisfies the floor.
fn weakest_of_closest_pair(
    slots: &[Option<DetectedCue>; N_SLOTS],
    floor: f64,
) -> Option<usize> {
    let mut worst: Option<(f64, usize)> = None;
    for a in 0..N_SLOTS {
        for b in (a + 1)..N_SLOTS {
            let (Some(x), Some(y)) = (&slots[a], &slots[b]) else {
                continue;
            };
            let gap = (x.position_ms - y.position_ms).abs();
            if gap >= floor {
                continue;
            }
            let weaker = if x.confidence <= y.confidence { a } else { b };
            if worst.map(|(g, _)| gap < g).unwrap_or(true) {
                worst = Some((gap, weaker));
            }
        }
    }
    worst.map(|(_, i)| i)
}

/// Somewhere inside the core at least `floor` from everything already placed:
/// the best remaining candidate if there is one, otherwise the downbeat with the
/// most room around it.
///
/// The second half matters. Without it the promise holds only on tracks that
/// happened to produce enough candidates, which is the same as not holding.
fn free_position(
    candidates: &[CueCandidate],
    grid: &Grid,
    core: (f64, f64),
    occupied: &[f64],
    floor: f64,
) -> Option<Placement> {
    let clear = |ms: f64| occupied.iter().all(|o| (ms - o).abs() >= floor);
    let in_core = |ms: f64| ms >= core.0 && ms <= core.1;

    let best = candidates
        .iter()
        .filter(|c| in_core(c.position_ms) && clear(c.position_ms))
        .max_by(|a, b| {
            a.salience
                .partial_cmp(&b.salience)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    if let Some(c) = best {
        return Some(Placement::fallback_at(c.strong_beat));
    }

    grid.downbeats_ms
        .iter()
        .filter(|d| in_core(**d) && clear(**d))
        .map(|d| (*d, min_gap(*d, occupied)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(ms, _)| Placement::fallback_at(grid.nearest_strong_beat(ms)))
}

fn min_gap(ms: f64, occupied: &[f64]) -> f64 {
    occupied
        .iter()
        .map(|o| (ms - o).abs())
        .fold(f64::INFINITY, f64::min)
}

/// A slot no rule could fill still has to carry a marker: the spec draws eight,
/// and a missing one reads as a bug rather than as an absence of evidence. It is
/// placed in the emptiest span and labelled with the fallback confidence.
fn backfill(
    slots: &mut [Option<DetectedCue>; N_SLOTS],
    candidates: &[CueCandidate],
    grid: &Grid,
    core: (f64, f64),
) {
    for slot in 0..N_SLOTS {
        if slots[slot].is_some() {
            continue;
        }
        let occupied = occupied_positions(slots);
        let target = emptiest_span(slots, core, slot);
        let placement = best_in_span(candidates, grid, core, target, &occupied)
            .unwrap_or_else(|| Placement::fallback_at(span_centre_strong_beat(grid, core, target)));
        slots[slot] = Some(cue(slot, placement, grid));
    }
}

/// Moves the least confident cue out of a span holding more than one, into a
/// span holding none. Bounded by the slot count so it cannot oscillate.
fn redistribute(
    slots: &mut [Option<DetectedCue>; N_SLOTS],
    candidates: &[CueCandidate],
    grid: &Grid,
    core: (f64, f64),
) {
    for _ in 0..N_SLOTS {
        let counts = span_counts(slots, core);
        let Some(empty) = counts.iter().position(|c| *c == 0) else {
            return;
        };
        let Some(crowded) = counts
            .iter()
            .enumerate()
            .filter(|(_, c)| **c > 1)
            .max_by_key(|(_, c)| **c)
            .map(|(i, _)| i)
        else {
            return;
        };

        let Some(victim) = weakest_in_span(slots, core, crowded) else {
            return;
        };
        let occupied = occupied_positions(slots)
            .into_iter()
            .filter(|ms| span_of(*ms, core) != crowded)
            .collect::<Vec<f64>>();
        let Some(placement) = best_in_span(candidates, grid, core, empty, &occupied) else {
            return;
        };
        // The role travels with the slot: a drop moved into an empty span is
        // still CUE 6, because the UI labels by slot and the export maps slot to
        // hot cue A-H.
        slots[victim] = Some(cue(victim, placement, grid));
    }
}

fn span_counts(slots: &[Option<DetectedCue>; N_SLOTS], core: (f64, f64)) -> [usize; N_SLOTS] {
    let mut counts = [0usize; N_SLOTS];
    for c in slots.iter().flatten() {
        counts[span_of(c.position_ms, core)] += 1;
    }
    counts
}

fn occupied_positions(slots: &[Option<DetectedCue>; N_SLOTS]) -> Vec<f64> {
    slots.iter().flatten().map(|c| c.position_ms).collect()
}

fn emptiest_span(
    slots: &[Option<DetectedCue>; N_SLOTS],
    core: (f64, f64),
    prefer_near: usize,
) -> usize {
    let counts = span_counts(slots, core);
    (0..N_SLOTS)
        .min_by_key(|i| (counts[*i], i.abs_diff(prefer_near)))
        .unwrap_or(prefer_near)
}

fn weakest_in_span(
    slots: &[Option<DetectedCue>; N_SLOTS],
    core: (f64, f64),
    span: usize,
) -> Option<usize> {
    slots
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.as_ref().map(|c| (i, c)))
        .filter(|(_, c)| span_of(c.position_ms, core) == span)
        .min_by(|a, b| {
            a.1.confidence
                .partial_cmp(&b.1.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

/// Highest-salience candidate inside a span that respects the 8-bar minimum
/// spacing against everything already placed.
fn best_in_span(
    candidates: &[CueCandidate],
    grid: &Grid,
    core: (f64, f64),
    span: usize,
    occupied: &[f64],
) -> Option<Placement> {
    let spacing = min_spacing_ms(grid);
    candidates
        .iter()
        .filter(|c| span_of(c.position_ms, core) == span)
        .filter(|c| {
            occupied
                .iter()
                .all(|ms| (c.position_ms - ms).abs() >= spacing)
        })
        .max_by(|a, b| {
            a.salience
                .partial_cmp(&b.salience)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|c| Placement::fallback_at(c.strong_beat))
}

fn span_centre_strong_beat(grid: &Grid, core: (f64, f64), span: usize) -> usize {
    let width = (core.1 - core.0) / N_SLOTS as f64;
    let ms = core.0 + width * (span as f64 + 0.5);
    grid.nearest_strong_beat(grid.snap_ms(ms))
}

/// The fallback confidence a backfilled marker reports, exposed so the UI copy
/// and the eval harness read the same number.
pub const BACKFILL_CONFIDENCE: f64 = FALLBACK_CONFIDENCE;
