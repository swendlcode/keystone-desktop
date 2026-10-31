//! Turning placements into the eight `DetectedCue` rows, with honest
//! confidences.
//!
//! **Confidence is not "how neatly the rule fired".** Published F1 against real
//! DJ cue points is 0.22 for Mixed In Key 10 and 0.39 for the best system in the
//! literature; nobody, including the incumbent, agrees with a particular DJ.
//! Each role therefore has a ceiling taken from how reliable that role is on
//! real material — the first beat of a four-to-the-floor track is nearly always
//! right, a breakdown boundary often is not — and the rule's own margin only
//! moves the number inside that ceiling. The grid's own confidence multiplies
//! through, because a cue on a misphased grid is wrong however well the rule
//! read the audio.

use crate::analysis::cues::{context::Placement, N_SLOTS};
use crate::analysis::structure::grid::Grid;
use crate::models::{CueKind, DetectedCue};

/// Per-role confidence ceilings. These are deliberately modest: a DJ dragging a
/// marker that claimed 0.95 twice will stop believing the number, and the
/// markers are draggable precisely because they are starting points.
pub const CEILING_FIRST_BEAT: f64 = 0.85;
pub const CEILING_DROP: f64 = 0.60;
pub const CEILING_INTRO: f64 = 0.55;
pub const CEILING_OUTRO: f64 = 0.50;
pub const CEILING_BREAKDOWN: f64 = 0.45;
pub const CEILING_BUILD: f64 = 0.35;

/// What a slot reports when no rule fired and it fell back to a bar line. Still
/// a usable place to start — it is on a downbeat — and honestly labelled as not
/// a detection.
pub const FALLBACK_CONFIDENCE: f64 = 0.15;

/// The eight slots in order, with the role each one carries.
pub const SLOT_ROLES: [(CueKind, &str); N_SLOTS] = [
    (CueKind::Intro, "Intro"),
    (CueKind::FirstBeat, "First beat"),
    (CueKind::Breakdown, "Breakdown"),
    (CueKind::Breakdown, "Breakdown 2"),
    (CueKind::Build, "Build"),
    (CueKind::Drop, "Drop"),
    (CueKind::Drop, "Drop 2"),
    (CueKind::Outro, "Outro"),
];

pub fn ceiling(kind: CueKind) -> f64 {
    match kind {
        CueKind::FirstBeat => CEILING_FIRST_BEAT,
        CueKind::Drop => CEILING_DROP,
        CueKind::Intro => CEILING_INTRO,
        CueKind::Outro => CEILING_OUTRO,
        CueKind::Breakdown => CEILING_BREAKDOWN,
        CueKind::Build => CEILING_BUILD,
        _ => FALLBACK_CONFIDENCE,
    }
}

/// Grid trust, folded into every confidence. A cue is only as well placed as the
/// downbeat phase and the phrase period it was snapped to.
pub fn grid_trust(grid: &Grid) -> f64 {
    let downbeat = if grid.snap_to_downbeat { 1.0 } else { 0.7 };
    downbeat * grid.phrase_confidence.clamp(0.4, 1.0)
}

pub fn cue(slot: usize, placement: Placement, grid: &Grid) -> DetectedCue {
    let (kind, label) = SLOT_ROLES[slot.min(N_SLOTS - 1)];
    let position_ms = grid.snap_ms(grid.strong_beat_ms(placement.strong_beat));
    let aligned = grid.nearest_strong_beat(position_ms);

    let confidence = if placement.fallback {
        FALLBACK_CONFIDENCE * grid_trust(grid)
    } else {
        // Half the ceiling is earned simply by the rule firing at all; the
        // other half is the margin. A rule that barely cleared its threshold
        // still found something.
        let c = ceiling(kind);
        (c * 0.5 + c * 0.5 * placement.margin as f64) * grid_trust(grid)
    };

    DetectedCue {
        slot: slot as u8,
        kind,
        position_ms,
        end_ms: None,
        bar_index: Some(grid.strong_beat_to_bar(aligned)),
        phrase_level: Some(grid.phrase_level_at(position_ms)),
        label: Some(label.to_string()),
        confidence: confidence.clamp(0.0, 1.0),
    }
}

/// Fills the eight slots from the per-role placements.
///
/// Slot order is the spec's: CUE 1 intro, CUE 2 first beat, CUE 3/4 breakdowns,
/// CUE 5 build, CUE 6/7 drops, CUE 8 outro.
pub fn assemble(
    intro: Placement,
    first_beat: Placement,
    breakdowns: &[Placement],
    build: Option<Placement>,
    drops: &[Placement],
    outro: Placement,
    grid: &Grid,
) -> [Option<DetectedCue>; N_SLOTS] {
    let mut slots: [Option<DetectedCue>; N_SLOTS] = Default::default();
    slots[0] = Some(cue(0, intro, grid));
    slots[1] = Some(cue(1, first_beat, grid));
    slots[2] = breakdowns.first().map(|p| cue(2, *p, grid));
    slots[3] = breakdowns.get(1).map(|p| cue(3, *p, grid));
    slots[4] = build.map(|p| cue(4, p, grid));
    slots[5] = drops.first().map(|p| cue(5, *p, grid));
    slots[6] = drops.get(1).map(|p| cue(6, *p, grid));
    slots[7] = Some(cue(7, outro, grid));
    slots
}
