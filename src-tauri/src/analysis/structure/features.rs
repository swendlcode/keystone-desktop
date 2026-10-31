//! The seven strong-beat-synchronous features.
//!
//! Zehren used a CNN drum transcriber for the first three. Band-limited flux is
//! the permissively licensed substitute and it gives up some precision on
//! material where the kick and the bass line overlap — say so in the changelog
//! rather than claiming parity.

use ndarray::Array2;

use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::spectra::Reductions;
use crate::analysis::structure::{Aggregation, StrongBeatFeatures, StructureFeature, N_FEATURES};
use crate::error::{KeystoneError, Result};

/// Local-mean window for the transient emphasis on the snare band, in frames.
/// At hop 512 / 44.1 kHz this is ~46 ms: long enough to subtract a sustained
/// pad, short enough to leave a snare hit standing.
const TRANSIENT_MEAN_FRAMES: usize = 4;

pub fn build(reductions: &Reductions, grid: &Grid) -> Result<StrongBeatFeatures> {
    let n = grid.n_strong_beats();
    if n < 2 {
        return Err(KeystoneError::Analysis(
            "structure needs at least two strong beats".into(),
        ));
    }

    let windows = strong_beat_windows(reductions, grid, n);
    let mut scalars: Vec<Vec<f32>> = vec![Vec::new(); N_FEATURES];

    for (slot, feature) in StructureFeature::ALL.iter().enumerate() {
        let series = match feature {
            StructureFeature::KickFlux => Some(flux_series(&reductions.kick, 1)),
            StructureFeature::SnareFlux => {
                Some(flux_series(&reductions.snare, TRANSIENT_MEAN_FRAMES))
            }
            StructureFeature::HatFlux => Some(flux_series(&reductions.hat, 1)),
            StructureFeature::HarmonicEnergy => Some(reductions.harmonic.clone()),
            StructureFeature::PercussiveEnergy => Some(reductions.percussive.clone()),
            _ => None,
        };
        if let Some(series) = series {
            scalars[slot] = aggregate(&series, &windows, feature.aggregation());
        }
    }

    Ok(StrongBeatFeatures {
        n_strong_beats: n,
        scalars,
        log_magnitude: mean_rows(&reductions.log_mag, &windows),
        chroma: mean_rows(&reductions.chroma, &windows),
    })
}

/// Frame range per strong beat. The last window is extrapolated from the grid
/// rather than clipped at the final beat, so the last bar is as wide as the
/// others and does not read as a dip in every continuous feature.
fn strong_beat_windows(
    reductions: &Reductions,
    grid: &Grid,
    n: usize,
) -> Vec<std::ops::Range<usize>> {
    let n_frames = reductions.n_frames();
    (0..n)
        .map(|i| {
            let start = reductions.ms_to_frame(grid.strong_beat_ms(i)).min(n_frames);
            let end = reductions
                .ms_to_frame(grid.strong_beat_ms(i + 1))
                .min(n_frames)
                .max(start + 1)
                .min(n_frames.max(start + 1));
            start..end
        })
        .collect()
}

/// Half-wave-rectified rise of a band series, with `mean_frames > 1` first
/// subtracting a running mean so a sustained band does not register as onsets.
fn flux_series(series: &[f32], mean_frames: usize) -> Vec<f32> {
    let base: Vec<f32> = if mean_frames > 1 {
        series
            .iter()
            .enumerate()
            .map(|(t, v)| {
                let lo = t.saturating_sub(mean_frames);
                let win = &series[lo..=t];
                let mean = win.iter().sum::<f32>() / win.len() as f32;
                (*v - mean).max(0.0)
            })
            .collect()
    } else {
        series.to_vec()
    };
    crate::analysis::structure::bands::hwr_diff(&base)
}

fn aggregate(
    series: &[f32],
    windows: &[std::ops::Range<usize>],
    how: Aggregation,
) -> Vec<f32> {
    windows
        .iter()
        .map(|w| {
            let slice = series.get(w.clone()).unwrap_or(&[]);
            if slice.is_empty() {
                return 0.0;
            }
            match how {
                Aggregation::OnsetSum => slice.iter().sum(),
                Aggregation::Rms => {
                    (slice.iter().map(|v| v * v).sum::<f32>() / slice.len() as f32).sqrt()
                }
            }
        })
        .collect()
}

fn mean_rows(matrix: &Array2<f32>, windows: &[std::ops::Range<usize>]) -> Array2<f32> {
    let cols = matrix.ncols();
    let mut out = Array2::<f32>::zeros((windows.len(), cols));
    for (i, w) in windows.iter().enumerate() {
        let lo = w.start.min(matrix.nrows());
        let hi = w.end.min(matrix.nrows());
        if hi <= lo {
            continue;
        }
        for t in lo..hi {
            for c in 0..cols {
                out[[i, c]] += matrix[[t, c]];
            }
        }
        let n = (hi - lo) as f32;
        for c in 0..cols {
            out[[i, c]] /= n;
        }
    }
    out
}
