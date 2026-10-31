//! Peak picking over the seven novelty curves, and the union that survives.

use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::Boundary;
use crate::error::Result;

/// Peak window, in bars. A local maximum has to be the largest thing inside a
/// phrase, otherwise every bar line in a four-to-the-floor track is a boundary.
pub const PEAK_WINDOW_BARS: usize = 4;

/// A peak must clear this fraction of its own curve's maximum. Per curve, not
/// across curves: the hat flux of a track with no hats would otherwise
/// contribute nothing at all, and the point of seven curves is that each one
/// speaks about the thing it measures.
pub const PEAK_AMPLITUDE_FRACTION: f32 = 0.3;

/// Two curves peaking one strong beat apart are the same boundary heard twice.
pub const MERGE_STRONG_BEATS: usize = 1;

/// Only boundaries on an 8-bar line or longer become section edges. 4-bar
/// boundaries are kept as cue candidates but a 4-bar "section" is a bar group,
/// not a section.
pub const SECTION_MIN_PHRASE_LEVEL: u8 = 8;

/// Local maxima of one curve, as `(index, amplitude)`.
///
/// Shared with S11, which peak-picks the identical curves with the identical
/// rule — the cue candidates and the section boundaries have to be drawn from
/// one set of peaks or a cue lands a strong beat away from the section it is
/// supposed to open.
pub fn peak_pick_curve(curve: &[f32], window: usize, fraction: f32) -> Vec<(usize, f32)> {
    let max = curve.iter().cloned().fold(0.0f32, f32::max);
    if max <= f32::EPSILON {
        return Vec::new();
    }
    let floor = fraction * max;
    let w = window.max(1);

    (0..curve.len())
        .filter(|i| {
            let v = curve[*i];
            if v < floor {
                return false;
            }
            let lo = i.saturating_sub(w);
            let hi = (i + w + 1).min(curve.len());
            // `>=` to the left and `>` to the right breaks a plateau tie
            // towards its first frame, which is the musically earlier edge.
            curve[lo..*i].iter().all(|o| v >= *o) && curve[i + 1..hi].iter().all(|o| v > *o)
        })
        .map(|i| (i, curve[i] / max))
        .collect()
}

/// Union of every curve's peaks, merged, aligned to bar lines and levelled.
pub fn detect(novelty: &[Vec<f32>], grid: &Grid) -> Result<Vec<Boundary>> {
    let window = grid.bars_to_strong_beats(PEAK_WINDOW_BARS);
    let mut merged: Vec<(usize, f32)> = Vec::new();

    for curve in novelty {
        for (idx, amp) in peak_pick_curve(curve, window, PEAK_AMPLITUDE_FRACTION) {
            match merged
                .iter_mut()
                .find(|(i, _)| i.abs_diff(idx) <= MERGE_STRONG_BEATS)
            {
                // Strength accumulates across curves on purpose: a boundary
                // that the kick, the chroma and the harmonic energy all agree
                // on is a stronger section edge than one only the hats saw.
                Some(slot) => slot.1 += amp,
                None => merged.push((idx, amp)),
            }
        }
    }

    merged.sort_by_key(|(i, _)| *i);
    let mut out: Vec<Boundary> = Vec::with_capacity(merged.len());
    for (idx, strength) in merged {
        let position_ms = grid.snap_ms(grid.strong_beat_ms(idx));
        let aligned = grid.nearest_strong_beat(position_ms);
        let boundary = Boundary {
            strong_beat: aligned,
            position_ms,
            bar_index: Some(grid.strong_beat_to_bar(aligned)),
            phrase_level: grid.phrase_level_at(position_ms),
            strength,
        };
        // Snapping can collapse two neighbours onto one downbeat; keep the
        // stronger of the two rather than emitting a zero-length section.
        match out.last_mut() {
            Some(prev) if prev.strong_beat == boundary.strong_beat => {
                if boundary.strength > prev.strength {
                    *prev = boundary;
                }
            }
            _ => out.push(boundary),
        }
    }
    Ok(out)
}

/// The subset that can bound a section.
pub fn section_edges(boundaries: &[Boundary]) -> Vec<&Boundary> {
    boundaries
        .iter()
        .filter(|b| b.phrase_level >= SECTION_MIN_PHRASE_LEVEL)
        .collect()
}
