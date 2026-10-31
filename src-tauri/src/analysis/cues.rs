//! S11 — the eight cue points.
//!
//! **The reality check first, because it decides the copy.** On EDM-CUE (4,710
//! tracks, 21,461 cue points placed by four professional DJs), Mixed In Key 10
//! scores 0.22 F1 and the best model in the literature reaches 0.39. Exact
//! agreement with a particular DJ is not achievable by anyone, including the
//! incumbent. Zehren's own rules reached 86% precision / 49% recall against
//! MIK's 65% precision, and 95.8% of his generated candidates were usable by at
//! least one expert at 8.9 candidates per track.
//!
//! So design the feature around "eight musically defensible, phrase-aligned
//! starting points you will drag", not "the eight right answers" — the markers
//! are draggable for that reason. The useful corollary: MIK's cue points are far
//! weaker than its key detection, so this is the easier place to beat it.
//!
//! The pipeline is Zehren's rules from Automix (MIT, portable freely): peak-pick
//! per novelty curve, keep what is on the phrase period, require that something
//! musical follows, then assign the eight slots by role, enforce coverage, and
//! snap. The role assignment and the coverage pass are ours — no paper covers
//! them, and they are what stops eight correct detections from being useless.

pub mod assign;
pub mod context;
pub mod coverage;
pub mod filters;
pub mod roles;

#[cfg(test)]
mod tests;

use crate::analysis::beats::BeatAnalysis;
use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::{Boundary, StrongBeatFeatures, StructureFeature};
use crate::error::Result;
use crate::models::DetectedCue;

pub const N_SLOTS: usize = 8;

/// Peak-pick: a local maximum within a 4-bar window, above this fraction of
/// that curve's own maximum.
pub const PEAK_WINDOW_BARS: usize = 4;
pub const PEAK_AMPLITUDE_FRACTION: f32 = 0.3;

/// The salience filter: mean harmonic energy over the 4 bars **following** the
/// candidate must exceed this fraction of the track's maximum harmonic energy.
///
/// This encodes "the section after this point can stand on its own in a mix" and
/// is what takes precision from roughly 30% to 86%. **Do not skip it.**
pub const SALIENCE_FRACTION: f32 = 0.4;
pub const SALIENCE_WINDOW_BARS: usize = 4;

/// Below this phrase confidence the period filter is skipped: filtering onto a
/// phrase grid we do not believe throws away every real candidate at once, and a
/// wrong period was 45% of Zehren's false negatives.
pub const PHRASE_CONFIDENCE_FLOOR: f64 = 0.4;

/// Minimum spacing between retained cues.
pub const MIN_SPACING_BARS: usize = 8;

/// Role-specific thresholds, named rather than inlined so the eval harness can
/// sweep them without hunting through the body.
pub const INTRO_PERCUSSIVE_FRACTION: f32 = 0.25;
pub const INTRO_WINDOW_BARS: usize = 4;
pub const FIRST_BEAT_KICK_FRACTION: f32 = 0.5;
pub const FIRST_BEAT_WINDOW_BARS: usize = 4;
pub const BREAKDOWN_PERCUSSIVE_FRACTION: f32 = 0.4;
pub const BREAKDOWN_HARMONIC_FRACTION: f32 = 0.4;
pub const BREAKDOWN_WINDOW_BARS: usize = 8;
pub const BUILD_MIN_BARS: usize = 4;
/// A riser longer than this is a section, not a build into the next one.
pub const BUILD_MAX_BARS: usize = 16;
pub const DROP_JUMP_RATIO: f32 = 1.5;
pub const DROP_WINDOW_BARS: usize = 4;
pub const DROP_MIN_SEPARATION_BARS: usize = 32;
pub const OUTRO_MIN_BARS_FROM_END: usize = 32;
pub const OUTRO_WINDOW_BARS: usize = 8;

#[derive(Debug, Clone)]
pub struct CueCandidate {
    pub strong_beat: usize,
    pub position_ms: f64,
    pub bar_index: Option<i32>,
    pub salience: f32,
    pub source_feature: StructureFeature,
}

/// Union of local maxima across all seven novelty curves.
pub fn peak_pick(novelty: &[Vec<f32>], beats: &BeatAnalysis) -> Result<Vec<CueCandidate>> {
    filters::peak_pick(novelty, &Grid::from_beats(beats))
}

/// Keeps only candidates on the estimated 4-bar period and phase.
pub fn period_filter(candidates: Vec<CueCandidate>, beats: &BeatAnalysis) -> Vec<CueCandidate> {
    filters::period_filter(candidates, &Grid::from_beats(beats))
}

/// The filter that makes the feature work. See [`SALIENCE_FRACTION`].
pub fn salience_filter(
    candidates: Vec<CueCandidate>,
    features: &StrongBeatFeatures,
    beats: &BeatAnalysis,
) -> Vec<CueCandidate> {
    filters::salience_filter(candidates, features, &Grid::from_beats(beats))
}

/// Assigns the eight slots **by role**.
///
/// CUE 1 intro start, CUE 2 first beat, CUE 3/4 breakdowns, CUE 5 build,
/// CUE 6/7 drops, CUE 8 outro. Role assignment also gives each marker the label
/// the UI shows, which is worth more to a DJ than an unlabelled confidence rank.
pub fn assign_slots(
    candidates: &[CueCandidate],
    features: &StrongBeatFeatures,
    grid: &Grid,
) -> [Option<DetectedCue>; N_SLOTS] {
    let ctx = context::RoleContext::new(features, grid);
    // Drops are resolved first because the build rule has to know what it is
    // building into.
    let drops = roles::drops(&ctx, candidates);
    let build = roles::build(&ctx, candidates, &drops);
    assign::assemble(
        roles::intro(&ctx, candidates),
        roles::first_beat(&ctx),
        &roles::breakdowns(&ctx),
        build,
        &drops,
        roles::outro(&ctx, candidates),
        grid,
    )
}

/// Partition the core into 8 equal spans; if one span holds no cue while another
/// holds several, move the weakest-confidence duplicate to the best candidate in
/// the nearest empty span. Minimum spacing 8 bars.
pub fn enforce_coverage(
    slots: [Option<DetectedCue>; N_SLOTS],
    candidates: &[CueCandidate],
    grid: &Grid,
    core_ms: (f64, f64),
) -> Vec<DetectedCue> {
    coverage::enforce(slots, candidates, grid, core_ms)
}

/// Everything snaps to a downbeat, or to a strong beat when the downbeat phase
/// confidence is low. **Never to a raw novelty peak.**
pub fn snap(position_ms: f64, grid: &Grid) -> f64 {
    grid.snap_ms(position_ms)
}

/// S11 end to end.
pub fn detect(
    novelty: &[Vec<f32>],
    boundaries: &[Boundary],
    features: &StrongBeatFeatures,
    grid: &Grid,
    core_ms: (f64, f64),
) -> Result<Vec<DetectedCue>> {
    let mut candidates = filters::peak_pick(novelty, grid)?;
    merge_boundaries(&mut candidates, boundaries);
    let candidates = filters::period_filter(candidates, grid);
    let candidates = filters::salience_filter(candidates, features, grid);

    let slots = assign_slots(&candidates, features, grid);
    Ok(coverage::enforce(slots, &candidates, grid, core_ms))
}

/// Section edges join the candidate set.
///
/// They come from the same seven curves, but they are already bar-aligned and
/// carry a cross-curve agreement strength, so a section edge the 4-bar peak
/// window suppressed is still available for a slot to land on.
fn merge_boundaries(candidates: &mut Vec<CueCandidate>, boundaries: &[Boundary]) {
    for b in boundaries {
        match candidates
            .iter_mut()
            .find(|c| c.strong_beat.abs_diff(b.strong_beat) <= 1)
        {
            Some(existing) => existing.salience = existing.salience.max(b.strength),
            None => candidates.push(CueCandidate {
                strong_beat: b.strong_beat,
                position_ms: b.position_ms,
                bar_index: b.bar_index,
                salience: b.strength,
                source_feature: StructureFeature::LogMagnitude,
            }),
        }
    }
    candidates.sort_by_key(|c| c.strong_beat);
}
