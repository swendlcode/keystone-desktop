//! The strong-beat grid as a lookup object, shared by S9, S10 and S11.
//!
//! [`BeatAnalysis`] stores the grid as three parallel time series and derives
//! downbeats on demand. S9 asks for a bar index per boundary, S10 slices the
//! core into bars, and S11 snaps eight cues plus every candidate it considers —
//! so deriving downbeats per call would allocate a `Vec` of several hundred
//! floats inside those loops. Building it once turns every one of those lookups
//! into a binary search.

use crate::analysis::beats::BeatAnalysis;

/// Below this downbeat-phase confidence, cues and boundaries snap to a strong
/// beat instead of a downbeat.
///
/// Zehren et al. could not reliably separate the downbeat from beat 3 in EDM
/// even with a neural tracker. When our own phase estimate is unsure, a strong
/// beat is half a bar away from the truth at worst; a confidently wrong downbeat
/// is a whole bar out and leaks a systematic offset into all eight cues.
pub const DOWNBEAT_CONFIDENCE_FLOOR: f64 = 0.5;

/// Strong beats are beats 1 and 3, so two per bar in 4/4.
pub const BEATS_PER_STRONG_BEAT: usize = 2;

#[derive(Debug, Clone)]
pub struct Grid {
    pub beats_ms: Vec<f64>,
    pub strong_beats_ms: Vec<f64>,
    pub downbeats_ms: Vec<f64>,
    pub beats_per_bar: usize,
    /// Strong beats per bar: 2 in 4/4.
    pub strong_per_bar: usize,
    /// Mean bar duration, for span-to-bar arithmetic where an exact beat index
    /// is not available (the head and tail of the track).
    pub bar_ms: f64,
    /// False when the downbeat phase is not trusted; see
    /// [`DOWNBEAT_CONFIDENCE_FLOOR`].
    pub snap_to_downbeat: bool,
    /// Phrase length in bars: 4, promoted to 8/16/32 by S8.
    pub phrase_bars: usize,
    /// Phrase phase, in **strong beats**, because S8 chooses it from the eight
    /// candidate strong-beat offsets rather than from bar lines.
    pub phrase_phase: usize,
    pub phrase_confidence: f64,
}

impl Grid {
    pub fn from_beats(b: &BeatAnalysis) -> Self {
        let beats_per_bar = b.beats_per_bar.max(1) as usize;
        let strong_per_bar = (beats_per_bar / BEATS_PER_STRONG_BEAT).max(1);
        let phase = (b.downbeat_phase as usize) % beats_per_bar;

        let downbeats_ms: Vec<f64> = b
            .beats_ms
            .iter()
            .enumerate()
            .filter(|(i, _)| i % beats_per_bar == phase)
            .map(|(_, ms)| *ms)
            .collect();

        let bar_ms = mean_step(&downbeats_ms)
            .or_else(|| mean_step(&b.beats_ms).map(|s| s * beats_per_bar as f64))
            .unwrap_or(0.0);

        Self {
            beats_ms: b.beats_ms.clone(),
            strong_beats_ms: b.strong_beats_ms.clone(),
            downbeats_ms,
            beats_per_bar,
            strong_per_bar,
            bar_ms,
            snap_to_downbeat: b.downbeat_confidence >= DOWNBEAT_CONFIDENCE_FLOOR,
            phrase_bars: b.phrase_bars.max(1) as usize,
            phrase_phase: b.phrase_phase as usize,
            phrase_confidence: b.phrase_confidence,
        }
    }

    pub fn n_strong_beats(&self) -> usize {
        self.strong_beats_ms.len()
    }

    pub fn strong_beat_ms(&self, i: usize) -> f64 {
        match self.strong_beats_ms.get(i) {
            Some(ms) => *ms,
            // Past the last tracked beat the grid is extrapolated rather than
            // clamped: a window that runs off the end must keep advancing or a
            // "following 4 bars" test silently compares a bar against itself.
            None => self.extrapolate(i),
        }
    }

    fn extrapolate(&self, i: usize) -> f64 {
        match (self.strong_beats_ms.last(), self.strong_beats_ms.len()) {
            (Some(last), n) if n > 0 => {
                let step = self.bar_ms / self.strong_per_bar as f64;
                last + step * (i + 1 - n) as f64
            }
            _ => 0.0,
        }
    }

    pub fn bars_to_strong_beats(&self, bars: usize) -> usize {
        (bars * self.strong_per_bar).max(1)
    }

    /// Bar number of a strong beat, counting from the first downbeat. Negative
    /// before it, which happens on tracks whose first detected downbeat is not
    /// the first strong beat.
    pub fn strong_beat_to_bar(&self, i: usize) -> i32 {
        let first = self.first_downbeat_strong_beat();
        let offset = i as i64 - first as i64;
        offset.div_euclid(self.strong_per_bar as i64) as i32
    }

    /// Index into `strong_beats_ms` of the strong beat carrying the first
    /// downbeat.
    pub fn first_downbeat_strong_beat(&self) -> usize {
        match self.downbeats_ms.first() {
            Some(ms) => self.nearest_strong_beat(*ms),
            None => 0,
        }
    }

    pub fn nearest_strong_beat(&self, ms: f64) -> usize {
        nearest(&self.strong_beats_ms, ms)
    }

    /// The snap rule: nearest downbeat, or nearest strong beat when the
    /// downbeat phase is not trusted. **Never a raw novelty peak.**
    pub fn snap_ms(&self, ms: f64) -> f64 {
        let series = if self.snap_to_downbeat && !self.downbeats_ms.is_empty() {
            &self.downbeats_ms
        } else {
            &self.strong_beats_ms
        };
        match series.get(nearest(series, ms)) {
            Some(v) => *v,
            None => ms,
        }
    }

    /// Strong beats in one phrase.
    pub fn phrase_strong_beats(&self) -> usize {
        (self.phrase_bars * self.strong_per_bar).max(1)
    }

    /// Whether a strong beat sits on the estimated phrase period and phase.
    pub fn on_phrase(&self, strong_beat: usize) -> bool {
        let period = self.phrase_strong_beats();
        let phase = self.phrase_phase % period;
        strong_beat % period == phase
    }

    /// Phrase level of a position: 32, 16, 8 or 4, whichever is the largest
    /// phrase whose boundary this bar lands on.
    pub fn phrase_level_at(&self, ms: f64) -> u8 {
        let bar = self.strong_beat_to_bar(self.nearest_strong_beat(ms));
        if bar < 0 {
            return 4;
        }
        for level in [32u8, 16, 8] {
            if bar % level as i32 == 0 {
                return level;
            }
        }
        4
    }
}

/// Index of the value in a sorted series closest to `ms`.
pub fn nearest(series: &[f64], ms: f64) -> usize {
    if series.is_empty() {
        return 0;
    }
    let hi = series.partition_point(|v| *v < ms);
    if hi == 0 {
        return 0;
    }
    if hi >= series.len() {
        return series.len() - 1;
    }
    if (series[hi] - ms).abs() < (ms - series[hi - 1]).abs() {
        hi
    } else {
        hi - 1
    }
}

fn mean_step(series: &[f64]) -> Option<f64> {
    if series.len() < 2 {
        return None;
    }
    Some((series[series.len() - 1] - series[0]) / (series.len() - 1) as f64)
}
