//! The evidence one track offers the role rules, gathered once.
//!
//! The six rules in [`super::roles`] each ask the same handful of questions —
//! how much percussion is in these four bars, how does it compare with the
//! track's own median, is this strong beat on a 16-bar line — so the series and
//! the summary statistics they share are resolved here rather than recomputed
//! per rule.

use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::regions::median;
use crate::analysis::structure::{StrongBeatFeatures, StructureFeature};

#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub strong_beat: usize,
    /// How decisively the rule fired, before the role prior is applied.
    pub margin: f32,
    /// True when the rule found nothing and the slot fell back to a bar line.
    /// A fallback is still a usable starting point — it is on a downbeat — but
    /// it is not a detection and must not be reported as one.
    pub fallback: bool,
}

impl Placement {
    pub(super) fn found(strong_beat: usize, margin: f32) -> Self {
        Self {
            strong_beat,
            margin: margin.clamp(0.0, 1.0),
            fallback: false,
        }
    }

    /// A slot filled without a rule firing: on a bar line, honestly flagged.
    pub fn fallback_at(strong_beat: usize) -> Self {
        Self::fallback(strong_beat)
    }

    pub(super) fn fallback(strong_beat: usize) -> Self {
        Self {
            strong_beat,
            margin: 0.0,
            fallback: true,
        }
    }
}

pub struct RoleContext<'a> {
    pub grid: &'a Grid,
    pub kick: &'a [f32],
    pub hat: &'a [f32],
    pub harmonic: &'a [f32],
    pub percussive: &'a [f32],
    /// Kick plus percussive energy, the quantity a drop moves.
    pub combined: Vec<f32>,
    pub perc_median: f32,
    pub perc_max: f32,
    pub kick_median: f32,
    pub harm_median: f32,
    pub n: usize,
}

impl<'a> RoleContext<'a> {
    pub fn new(features: &'a StrongBeatFeatures, grid: &'a Grid) -> Self {
        let kick = features.scalar(StructureFeature::KickFlux);
        let percussive = features.scalar(StructureFeature::PercussiveEnergy);
        let combined = kick
            .iter()
            .zip(percussive.iter())
            .map(|(k, p)| k + p)
            .collect();
        Self {
            grid,
            kick,
            hat: features.scalar(StructureFeature::HatFlux),
            harmonic: features.scalar(StructureFeature::HarmonicEnergy),
            percussive,
            combined,
            perc_median: median(percussive),
            perc_max: percussive.iter().cloned().fold(0.0f32, f32::max),
            kick_median: median(kick),
            harm_median: median(features.scalar(StructureFeature::HarmonicEnergy)),
            n: features.n_strong_beats,
        }
    }

    pub fn bars(&self, bars: usize) -> usize {
        self.grid.bars_to_strong_beats(bars)
    }
}
