//! Step 4: the six role rules.
//!
//! No paper covers slot assignment, and it is where the spec's eight fixed slots
//! bite. Taking the top eight candidates by salience clusters them all in one
//! section and makes the feature useless even when every single detection is
//! correct — so each slot is won by a rule about what that part of a track does,
//! not by a ranking.

use crate::analysis::cues::*;
use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::regions::trends_up;

use super::context::{Placement, RoleContext};
use super::filters::{window_mean, window_mean_before};

/// CUE 6/7 — drops. Combined kick + percussive energy over the following 4 bars
/// jumps at least 1.5x relative to the preceding 4, on a 16-bar boundary; the
/// two highest jumps, forced at least 32 bars apart.
///
/// Computed first because the build rule needs somewhere to build to.
pub fn drops(ctx: &RoleContext, candidates: &[CueCandidate]) -> Vec<Placement> {
    let w = ctx.bars(DROP_WINDOW_BARS);
    let mut jumps: Vec<(usize, f32)> = candidates
        .iter()
        .filter(|c| on_level(ctx.grid, c.strong_beat, 16))
        .filter_map(|c| {
            let before = window_mean_before(&ctx.combined, c.strong_beat, w);
            let after = window_mean(&ctx.combined, c.strong_beat, w);
            if before <= f32::EPSILON || after < DROP_JUMP_RATIO * before {
                return None;
            }
            Some((c.strong_beat, after / before))
        })
        .collect();

    jumps.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let apart = ctx.bars(DROP_MIN_SEPARATION_BARS);
    let mut out: Vec<Placement> = Vec::new();
    for (sb, ratio) in jumps {
        if out.iter().any(|p| p.strong_beat.abs_diff(sb) < apart) {
            continue;
        }
        out.push(Placement::found(sb, margin(ratio, DROP_JUMP_RATIO)));
        if out.len() == 2 {
            break;
        }
    }
    out.sort_by_key(|p| p.strong_beat);
    out
}

/// CUE 5 — build. The start of a monotone rise in high-band energy and onset
/// density lasting at least 4 bars and ending at a drop.
///
/// "Ending within 2 bars of a drop candidate" is satisfied by construction: the
/// rise is measured over `[candidate, drop]`, so the window the rule tests is
/// the build itself rather than a free-floating riser that happens to be near
/// one.
pub fn build(
    ctx: &RoleContext,
    candidates: &[CueCandidate],
    drops: &[Placement],
) -> Option<Placement> {
    let min = ctx.bars(BUILD_MIN_BARS);
    let max = ctx.bars(BUILD_MAX_BARS);
    let mut best: Option<(usize, f32)> = None;

    for drop in drops {
        for c in candidates {
            let len = drop.strong_beat.saturating_sub(c.strong_beat);
            if len < min || len > max {
                continue;
            }
            let hat = slice(ctx.hat, c.strong_beat, len);
            let perc = slice(ctx.percussive, c.strong_beat, len);
            if !(trends_up(&hat) && trends_up(&perc)) {
                continue;
            }
            // How much the high band actually gained across the riser. A rise
            // that is monotone but flat is a mastering artefact, not a build.
            let lift = ratio(
                window_mean(ctx.hat, drop.strong_beat.saturating_sub(min), min),
                window_mean(ctx.hat, c.strong_beat, min),
            );
            if best.map(|(_, b)| lift > b).unwrap_or(true) {
                best = Some((c.strong_beat, lift));
            }
        }
    }
    best.map(|(sb, lift)| Placement::found(sb, margin(lift, 1.0)))
}

/// CUE 1 — intro start. The first candidate at or after the first downbeat whose
/// preceding 4 bars are below 0.25x max percussive energy and whose following 4
/// bars are above it; otherwise the first downbeat.
pub fn intro(ctx: &RoleContext, candidates: &[CueCandidate]) -> Placement {
    let w = ctx.bars(INTRO_WINDOW_BARS);
    let floor = INTRO_PERCUSSIVE_FRACTION * ctx.perc_max;
    let first = ctx.grid.first_downbeat_strong_beat();

    candidates
        .iter()
        .filter(|c| c.strong_beat >= first)
        .find_map(|c| {
            let before = window_mean_before(ctx.percussive, c.strong_beat, w);
            let after = window_mean(ctx.percussive, c.strong_beat, w);
            (before < floor && after > floor)
                .then(|| Placement::found(c.strong_beat, margin(after, floor.max(f32::EPSILON))))
        })
        .unwrap_or_else(|| Placement::fallback(first))
}

/// CUE 2 — first beat. The first downbeat where kick-band energy crosses 0.5x
/// its track median over a 4-bar window. On four-to-the-floor this is the most
/// reliable cue in the set, which is why its confidence ceiling is the highest.
pub fn first_beat(ctx: &RoleContext) -> Placement {
    let w = ctx.bars(FIRST_BEAT_WINDOW_BARS);
    let floor = FIRST_BEAT_KICK_FRACTION * ctx.kick_median;
    let step = ctx.grid.strong_per_bar.max(1);

    (0..ctx.n)
        .step_by(step)
        .find_map(|sb| {
            let mean = window_mean(ctx.kick, sb, w);
            (mean > floor).then(|| Placement::found(sb, margin(mean, floor.max(f32::EPSILON))))
        })
        .unwrap_or_else(|| Placement::fallback(ctx.grid.first_downbeat_strong_beat()))
}

/// CUE 3/4 — breakdowns. Percussive energy over the following 8 bars below 0.4x
/// its track median *while* harmonic energy stays above 0.4x its own: the beat
/// stopping while the music does not. The two longest such regions' starts.
pub fn breakdowns(ctx: &RoleContext) -> Vec<Placement> {
    let w = ctx.bars(BREAKDOWN_WINDOW_BARS);
    let perc_floor = BREAKDOWN_PERCUSSIVE_FRACTION * ctx.perc_median;
    let harm_floor = BREAKDOWN_HARMONIC_FRACTION * ctx.harm_median;
    let step = ctx.grid.strong_per_bar.max(1);

    let inside: Vec<bool> = (0..ctx.n)
        .map(|sb| {
            window_mean(ctx.percussive, sb, w) < perc_floor
                && window_mean(ctx.harmonic, sb, w) > harm_floor
        })
        .collect();

    let mut runs = runs_of_true(&inside);
    runs.sort_by_key(|(lo, hi)| std::cmp::Reverse(hi - lo));
    runs.truncate(2);
    runs.sort_by_key(|(lo, _)| *lo);
    runs.into_iter()
        .map(|(lo, hi)| {
            let aligned = lo - lo % step;
            Placement::found(aligned, margin((hi - lo + 1) as f32, w as f32))
        })
        .collect()
}

/// CUE 8 — outro. The last candidate at least 32 bars before the end where
/// percussive energy begins a monotone decline; otherwise the last 16-bar
/// boundary.
pub fn outro(ctx: &RoleContext, candidates: &[CueCandidate]) -> Placement {
    let w = ctx.bars(OUTRO_WINDOW_BARS);
    let limit = ctx.n.saturating_sub(ctx.bars(OUTRO_MIN_BARS_FROM_END));

    let found = candidates
        .iter()
        .rev()
        .filter(|c| c.strong_beat <= limit)
        .find_map(|c| {
            let after = slice(ctx.percussive, c.strong_beat, w);
            let declining = !after.is_empty() && !trends_up(&after);
            let drop = ratio(
                window_mean_before(ctx.percussive, c.strong_beat, w),
                window_mean(ctx.percussive, c.strong_beat, w),
            );
            (declining && drop > 1.0).then(|| Placement::found(c.strong_beat, margin(drop, 1.0)))
        });

    found.unwrap_or_else(|| Placement::fallback(last_level_boundary(ctx, 16)))
}

fn last_level_boundary(ctx: &RoleContext, level: i32) -> usize {
    (0..ctx.n)
        .rev()
        .find(|sb| on_level(ctx.grid, *sb, level))
        .unwrap_or(0)
}

/// Whether a strong beat lands on an `level`-bar line.
pub fn on_level(grid: &Grid, strong_beat: usize, level: i32) -> bool {
    let bar = grid.strong_beat_to_bar(strong_beat);
    bar >= 0 && bar % level == 0
}

fn runs_of_true(flags: &[bool]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, on) in flags.iter().enumerate() {
        match (on, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, flags.len() - 1));
    }
    out
}

fn slice(series: &[f32], from: usize, len: usize) -> Vec<f32> {
    let hi = (from + len).min(series.len());
    series.get(from..hi).unwrap_or(&[]).to_vec()
}

fn ratio(a: f32, b: f32) -> f32 {
    if b <= f32::EPSILON {
        0.0
    } else {
        a / b
    }
}

/// How far past its threshold a rule fired, squashed into [0, 1].
///
/// Saturating rather than linear so a 20x jump and a 4x jump are both simply
/// "clearly a drop": the difference between them says nothing about whether a DJ
/// would start there.
fn margin(observed: f32, threshold: f32) -> f32 {
    if threshold <= f32::EPSILON {
        return 0.0;
    }
    let excess = (observed / threshold - 1.0).max(0.0);
    1.0 - (-excess).exp()
}
