//! Steps 1-3: peak-pick, period filter, salience filter.
//!
//! Step 3 is the one that matters. Requiring four bars of harmonic energy
//! *after* the candidate encodes "the section this opens can stand on its own in
//! a mix", and it is what took Zehren's precision from roughly 30% to 86%. It
//! looks like a detail and it is most of the feature.

use crate::analysis::cues::{
    CueCandidate, PEAK_AMPLITUDE_FRACTION, PEAK_WINDOW_BARS, SALIENCE_FRACTION,
    SALIENCE_WINDOW_BARS,
};
use crate::analysis::structure::boundaries::peak_pick_curve;
use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::{StrongBeatFeatures, StructureFeature};
use crate::error::Result;

/// Two curves peaking a strong beat apart are the same moment heard twice.
const MERGE_STRONG_BEATS: usize = 1;

/// Union of local maxima across all seven novelty curves.
pub fn peak_pick(novelty: &[Vec<f32>], grid: &Grid) -> Result<Vec<CueCandidate>> {
    let window = grid.bars_to_strong_beats(PEAK_WINDOW_BARS);
    let mut out: Vec<CueCandidate> = Vec::new();

    for (slot, curve) in novelty.iter().enumerate() {
        let feature = StructureFeature::ALL
            .get(slot)
            .copied()
            .unwrap_or(StructureFeature::KickFlux);
        for (idx, amp) in peak_pick_curve(curve, window, PEAK_AMPLITUDE_FRACTION) {
            match out
                .iter_mut()
                .find(|c| c.strong_beat.abs_diff(idx) <= MERGE_STRONG_BEATS)
            {
                // Agreement across curves is the only evidence available at this
                // stage that a peak is a section edge rather than a loud bar, so
                // it accumulates instead of being maxed.
                Some(existing) => existing.salience += amp,
                None => out.push(CueCandidate {
                    strong_beat: idx,
                    position_ms: grid.strong_beat_ms(idx),
                    bar_index: Some(grid.strong_beat_to_bar(idx)),
                    salience: amp,
                    source_feature: feature,
                }),
            }
        }
    }

    out.sort_by_key(|c| c.strong_beat);
    Ok(out)
}

/// Keeps only candidates on the estimated 4-bar period and phase.
///
/// A wrong phrase period accounted for 45% of Zehren's false negatives, so this
/// filter is also the one most likely to be the reason a track comes back with
/// too few candidates. When the phrase estimate is itself unconfident, it is
/// skipped rather than applied to a grid we do not believe.
pub fn period_filter(candidates: Vec<CueCandidate>, grid: &Grid) -> Vec<CueCandidate> {
    if grid.phrase_confidence < crate::analysis::cues::PHRASE_CONFIDENCE_FLOOR {
        return candidates;
    }
    candidates
        .into_iter()
        .filter(|c| grid.on_phrase(c.strong_beat))
        .collect()
}

/// Requires four bars of following harmonic energy above
/// [`SALIENCE_FRACTION`] of the track's maximum, and folds that ratio into the
/// candidate's salience so later "weakest candidate" comparisons weigh both
/// novelty agreement and whether anything musical follows.
pub fn salience_filter(
    candidates: Vec<CueCandidate>,
    features: &StrongBeatFeatures,
    grid: &Grid,
) -> Vec<CueCandidate> {
    let harmonic = features.scalar(StructureFeature::HarmonicEnergy);
    let max = harmonic.iter().cloned().fold(0.0f32, f32::max);
    if max <= f32::EPSILON {
        return candidates;
    }
    let floor = SALIENCE_FRACTION * max;
    let window = grid.bars_to_strong_beats(SALIENCE_WINDOW_BARS);

    candidates
        .into_iter()
        .filter_map(|mut c| {
            let mean = window_mean(harmonic, c.strong_beat, window);
            if mean < floor {
                return None;
            }
            c.salience *= mean / max;
            Some(c)
        })
        .collect()
}

/// Mean of `series` over `len` entries starting at `from`. A window that runs
/// off the end averages what is there rather than padding with zeros, so the
/// last candidate in a track is not penalised for being last.
pub fn window_mean(series: &[f32], from: usize, len: usize) -> f32 {
    let hi = (from + len).min(series.len());
    let slice = series.get(from..hi).unwrap_or(&[]);
    if slice.is_empty() {
        return 0.0;
    }
    slice.iter().sum::<f32>() / slice.len() as f32
}

/// Mean of the `len` entries immediately before `before`.
pub fn window_mean_before(series: &[f32], before: usize, len: usize) -> f32 {
    let lo = before.saturating_sub(len);
    let slice = series.get(lo..before.min(series.len())).unwrap_or(&[]);
    if slice.is_empty() {
        return 0.0;
    }
    slice.iter().sum::<f32>() / slice.len() as f32
}
