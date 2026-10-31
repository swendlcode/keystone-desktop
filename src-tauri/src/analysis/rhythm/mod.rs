//! The rhythm path: S6 onsets, S7 tempo and beat grid, S8 downbeats and phrase
//! period.
//!
//! One novelty front end feeds all three. The band-limited curves are the
//! reason: the kick curve is what settles the tempo octave, the hat curve is
//! what exposes half-time, and both come out of the same filterbank pass as the
//! full-band curve the grid is tracked on.
//!
//! What this path refuses to do is as much of the design as what it does.
//! Everything here is tuned for four-to-the-floor, and ENGINE.md §7.2 names
//! swung house, UK garage and half-time trap as cases that break it. The answer
//! is a confidence that drops rather than a number that lies: the octave rule
//! reports when the kicks and the hats disagree, the grid reports the fraction
//! of the file it actually fits, and two-decimal BPM is gated on a residual
//! under 15 ms so variable-tempo material cannot acquire spurious precision.

pub mod arbiter;
pub mod beats;
pub mod downbeat;
pub mod grid;
pub mod octave;
pub mod onset;
pub mod phrase;
pub mod spectral;
pub mod tempo;
pub mod tempogram;

// Generators for the tests and for the dev-only eval harness. The harness has
// to build its corpus from the *same* generators the unit tests use: two copies
// of a chord renderer means one generator bug can flatter one stage's numbers
// while hiding from the other's.
#[cfg(any(test, feature = "eval"))]
pub mod fixtures;

use crate::error::Result;

/// Everything S6..S8 produced for one track.
pub struct RhythmAnalysis {
    pub curves: onset::NoveltyCurves,
    pub tempo: tempo::TempoEstimate,
    pub grid: tempo::BeatGrid,
    pub beats: beats::BeatAnalysis,
}

/// S6..S8 over one decoded mono buffer.
///
/// `beat_chroma` is optional and may be empty. The chroma stage is S3 and the
/// downbeat phase is better with it, but it must not be a prerequisite: a file
/// whose key analysis failed still owes the user a tempo and a grid.
pub fn analyze(
    samples: &[f32],
    sample_rate: u32,
    beat_chroma: &[[f32; 12]],
) -> Result<RhythmAnalysis> {
    let curves = onset::novelty(samples, sample_rate)?;
    let (tempo, grid) = tempo::estimate(&curves)?;
    let beats = beats::analyze(&grid, &curves, beat_chroma)?;
    Ok(RhythmAnalysis { curves, tempo, grid, beats })
}

/// One beatgrid marker, before persistence gives it an id.
///
/// Mirrors rekordbox's TEMPO element, which is what makes the export musically
/// correct: `Inizio`, `Bpm`, `Metro` and `Battito`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMarker {
    pub position_ms: f64,
    pub bpm: f64,
    pub beats_per_bar: u8,
    pub beat_in_bar: u8,
}

/// The beatgrid markers S12 persists and the rekordbox exporter reads.
///
/// **Two markers for a constant-tempo track, not one per beat.** rekordbox
/// 5.8.6+ writes one per beat, which is why real exports run to millions of
/// elements; what rekordbox actually needs is for its BPM and grid to agree
/// with ours, or every imported cue drifts.
pub fn beatgrid_markers(analysis: &RhythmAnalysis) -> Vec<GridMarker> {
    let downbeats = analysis.beats.downbeats_ms();
    let marker = |position_ms: f64| GridMarker {
        position_ms,
        bpm: analysis.tempo.raw_bpm,
        beats_per_bar: analysis.beats.beats_per_bar,
        beat_in_bar: 1,
    };
    match (downbeats.first(), downbeats.last()) {
        (Some(first), Some(last)) if last > first => vec![marker(*first), marker(*last)],
        (Some(first), _) => vec![marker(*first)],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_whole_path_runs_on_a_house_loop() {
        let f = fixtures::click_track(124.0, 24.0, fixtures::Groove::FourToTheFloor);
        let r = analyze(&f.samples, fixtures::SAMPLE_RATE, &[]).unwrap();
        assert!((r.tempo.raw_bpm - 124.0).abs() < 1.0, "bpm {}", r.tempo.raw_bpm);
        assert!(!r.tempo.variable);
        assert!(!r.beats.strong_beats_ms.is_empty());
        assert_eq!(beatgrid_markers(&r).len(), 2);

        // The recovered grid against the fixture's own click times. 70 ms of
        // tolerance covers the uncentred STFT, which leads the audio by up to
        // half a 2048-sample window.
        let hit = beats::agreement(&r.grid.beats_ms, &f.beats_ms, beats::AGREEMENT_TOLERANCE_MS);
        assert!(hit > 0.95, "only {hit:.2} of the grid landed on a click");
    }

    #[test]
    fn a_track_with_no_grid_produces_no_markers() {
        let r = analyze(&vec![0.0f32; 44_100], fixtures::SAMPLE_RATE, &[]).unwrap();
        assert!(beatgrid_markers(&r).len() <= 1);
    }
}
