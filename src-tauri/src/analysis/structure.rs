//! S9 — strong-beat features, self-similarity, checkerboard novelty, regions.
//!
//! Measured at 1.2 ms per feature at 384 strong beats, under 10 ms for all
//! seven. This stage is free, so do not economise on features.
//!
//! Everything here is indexed in **strong beats**, not bars. Zehren et al.
//! report they could not reliably separate the downbeat from beat 3 in EDM even
//! with a neural tracker; a 2-beat grid is reliable where a 4-beat grid is not,
//! and designing on the reliable one is what stops a systematic half-bar offset
//! leaking into every section edge and every cue built on top of it.

pub mod bands;
pub mod boundaries;
pub mod features;
pub mod grid;
pub mod regions;
pub mod spectra;
pub mod ssm;

// Generators for the tests and for the dev-only eval harness. The harness has
// to build its corpus from the *same* generators the unit tests use: two copies
// of a chord renderer means one generator bug can flatter one stage's numbers
// while hiding from the other's.
#[cfg(any(test, feature = "eval"))]
pub mod fixture;
#[cfg(test)]
mod tests;

use ndarray::{Array2, ArrayView2};

use crate::analysis::beats::BeatAnalysis;
use crate::error::Result;
use crate::models::DetectedRegion;

use self::grid::Grid;
use self::spectra::{Reductions, Spectra};

pub const N_FEATURES: usize = 7;
/// A Gaussian checkerboard kernel sized to compare 4 bars against 4 bars: an
/// 8-bar kernel is 32 beats, which is 16 strong beats.
pub const KERNEL_STRONG_BEATS: usize = 16;
/// HPSS median filter extents at hop 512.
pub const HPSS_TIME_MEDIAN: usize = 17;
pub const HPSS_FREQ_MEDIAN: usize = 17;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureFeature {
    /// 30-150 Hz flux.
    KickFlux,
    /// 150-800 Hz flux, transient-emphasised.
    SnareFlux,
    /// Above 6 kHz flux.
    HatFlux,
    /// Harmonic component energy after HPSS.
    HarmonicEnergy,
    /// Percussive component energy after HPSS.
    PercussiveEnergy,
    /// 84-bin log-frequency magnitude, f_min 32.7 Hz, hop 512 at 44.1 kHz.
    LogMagnitude,
    /// 12-bin chroma, 7 octaves, f_min 27.5 Hz.
    Chroma,
}

impl StructureFeature {
    pub const ALL: [StructureFeature; N_FEATURES] = [
        StructureFeature::KickFlux,
        StructureFeature::SnareFlux,
        StructureFeature::HatFlux,
        StructureFeature::HarmonicEnergy,
        StructureFeature::PercussiveEnergy,
        StructureFeature::LogMagnitude,
        StructureFeature::Chroma,
    ];

    /// Position in [`StructureFeature::ALL`], which is also the index into
    /// `StrongBeatFeatures::scalars` and into the novelty curve list.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }

    /// Sparse and transient features aggregate by **sum of onsets** per strong
    /// beat; continuous ones by **RMS**. Using one rule for both flattens the
    /// transients that make a boundary visible.
    pub fn aggregation(self) -> Aggregation {
        match self {
            StructureFeature::KickFlux
            | StructureFeature::SnareFlux
            | StructureFeature::HatFlux => Aggregation::OnsetSum,
            _ => Aggregation::Rms,
        }
    }

    /// **Zero** padding for the two loudness features, **valid** for the rest.
    ///
    /// This is the single detail that gives the intro a novelty value at all,
    /// and so the only reason "intro start" is detectable. Without it CUE 1 has
    /// nothing to find.
    pub fn pad_mode(self) -> PadMode {
        match self {
            StructureFeature::HarmonicEnergy | StructureFeature::PercussiveEnergy => PadMode::Zero,
            _ => PadMode::Valid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregation {
    OnsetSum,
    Rms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadMode {
    Valid,
    Zero,
}

/// Strong-beat-synchronous features. Scalar features are one row; the two
/// vector features keep their full width because their SSM is what detects a
/// harmonic change the scalar curves cannot see.
pub struct StrongBeatFeatures {
    pub n_strong_beats: usize,
    /// Indexed by `StructureFeature::ALL` order; the vector features store
    /// their matrix in `matrices` and leave their scalar row empty.
    pub scalars: Vec<Vec<f32>>,
    pub log_magnitude: Array2<f32>,
    pub chroma: Array2<f32>,
}

impl StrongBeatFeatures {
    /// Empty for the two vector features, which have no scalar row.
    pub fn scalar(&self, feature: StructureFeature) -> &[f32] {
        self.scalars
            .get(feature.index())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// One feature as a time x dims matrix, which is what the SSM takes.
    pub fn matrix(&self, feature: StructureFeature) -> Array2<f32> {
        match feature {
            StructureFeature::LogMagnitude => self.log_magnitude.clone(),
            StructureFeature::Chroma => self.chroma.clone(),
            other => {
                let row = self.scalar(other);
                Array2::from_shape_vec((row.len(), 1), row.to_vec())
                    .unwrap_or_else(|_| Array2::zeros((0, 1)))
            }
        }
    }
}

/// What S9 hands to S10 and S11.
pub struct Structure {
    pub grid: Grid,
    pub reductions: Reductions,
    pub features: StrongBeatFeatures,
    /// One checkerboard novelty curve per feature, in `StructureFeature::ALL`
    /// order and indexed in strong beats.
    pub novelty: Vec<Vec<f32>>,
    pub boundaries: Vec<Boundary>,
    pub regions: Vec<DetectedRegion>,
}

#[derive(Debug, Clone)]
pub struct Boundary {
    pub strong_beat: usize,
    pub position_ms: f64,
    pub bar_index: Option<i32>,
    /// 4 | 8 | 16 | 32.
    pub phrase_level: u8,
    pub strength: f32,
}

/// S9 end to end over one already-computed spectral pass.
pub fn analyze(spectra: &Spectra, beats: &BeatAnalysis, duration_ms: i64) -> Result<Structure> {
    let grid = Grid::from_beats(beats);
    let reductions = Reductions::new(spectra);
    let feats = features::build(&reductions, &grid)?;
    let curves = novelty_curves(&feats);
    let bounds = boundaries::detect(&curves, &grid)?;
    let regs = regions::build(&bounds, &feats, &grid, duration_ms)?;

    Ok(Structure {
        grid,
        reductions,
        features: feats,
        novelty: curves,
        boundaries: bounds,
        regions: regs,
    })
}

pub fn strong_beat_features(
    samples: &[f32],
    sample_rate: u32,
    beats: &BeatAnalysis,
) -> Result<StrongBeatFeatures> {
    let spectra = Spectra::compute(samples, sample_rate)?;
    features::build(&Reductions::new(&spectra), &Grid::from_beats(beats))
}

/// Self-similarity with **standardised-Euclidean** distance. Plain Euclidean
/// lets whichever dimension happens to have the largest variance dominate.
pub fn ssm(feature: ArrayView2<f32>) -> Array2<f32> {
    ssm::distance_matrix(feature)
}

/// Convolves the SSM diagonal with a Gaussian checkerboard kernel.
pub fn checkerboard_novelty(matrix: ArrayView2<f32>, kernel: usize, pad: PadMode) -> Vec<f32> {
    ssm::checkerboard_novelty(matrix, kernel, pad)
}

/// One novelty curve per feature, each with that feature's own padding rule.
pub fn novelty_curves(features: &StrongBeatFeatures) -> Vec<Vec<f32>> {
    StructureFeature::ALL
        .iter()
        .map(|f| {
            let matrix = features.matrix(*f);
            let distance = ssm(matrix.view());
            checkerboard_novelty(distance.view(), KERNEL_STRONG_BEATS, f.pad_mode())
        })
        .collect()
}

/// Retained boundaries across every feature's novelty curve.
pub fn boundaries(novelty: &[Vec<f32>], beats: &BeatAnalysis) -> Result<Vec<Boundary>> {
    boundaries::detect(novelty, &Grid::from_beats(beats))
}

/// Sections are consecutive retained boundaries at the 8- or 16-bar level.
/// Per-section energy is filled by S10 with the identical frozen constants, so
/// the numbers are comparable with the track energy.
pub fn regions(
    bounds: &[Boundary],
    features: &StrongBeatFeatures,
    beats: &BeatAnalysis,
    duration_ms: i64,
) -> Result<Vec<DetectedRegion>> {
    regions::build(bounds, features, &Grid::from_beats(beats), duration_ms)
}
