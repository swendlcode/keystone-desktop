//! Stage-specific reductions over a magnitude spectrogram.
//!
//! The line between this and `analysis::frontend` is cost, not taste: the STFT
//! and the HPSS median filters are expensive and shared, so they live there and
//! are computed once per track. These are single passes over a matrix that
//! already exists, and both log-frequency projections below are S9's own
//! geometry (84 bins from C1, chroma over 7 octaves from A0) rather than
//! anything onsets or key would ask for.

use ndarray::{Array2, ArrayView2};

/// C1. The 84-bin log-frequency vector runs seven octaves up from here, which
/// is a semitone per bin.
pub const LOG_F_MIN_HZ: f32 = 32.703_196;
pub const LOG_BINS: usize = 84;

/// A0, the chroma vector's bottom.
pub const CHROMA_F_MIN_HZ: f32 = 27.5;
pub const CHROMA_OCTAVES: usize = 7;
pub const CHROMA_BINS: usize = 12;

/// Per-frame magnitude inside `[lo, hi)` Hz, as a sum.
///
/// A sum rather than an RMS because every use of it downstream is a *ratio*
/// against another band or against the whole frame, and a ratio of sums is the
/// quantity that stays put when the gain changes.
///
/// This is the right shape for a **flux** feature, which differences one band
/// against itself over time. It is the wrong shape for a band *ratio* against
/// the whole spectrum: see [`band_power_rows`].
pub fn band_rows(mag: ArrayView2<f32>, bin_hz: f32, lo_hz: f32, hi_hz: f32) -> Vec<f32> {
    let (lo, hi) = bin_range(mag.ncols(), bin_hz, lo_hz, hi_hz);
    mag.rows()
        .into_iter()
        .map(|row| {
            if hi <= lo {
                0.0
            } else {
                row.iter().skip(lo).take(hi - lo).sum()
            }
        })
        .collect()
}

/// Per-frame total magnitude.
pub fn total_rows(mag: ArrayView2<f32>) -> Vec<f32> {
    mag.rows().into_iter().map(|r| r.iter().sum()).collect()
}

/// Per-frame **power** inside `[lo, hi)` Hz: the sum of squared magnitudes.
///
/// The feature table calls `low_ratio` and `high_ratio` energy ratios, and the
/// distinction is not pedantry. A magnitude sum over 30-150 Hz spans about ten
/// FFT bins while the whole-spectrum denominator spans two thousand, so the
/// ratio ends up reporting how *wide* the band is rather than how loud it is: a
/// bass-heavy track measured that way comes out at 0.006. In power the bass
/// genuinely dominates, which is both what the spec asks for and what a listener
/// hears.
pub fn band_power_rows(mag: ArrayView2<f32>, bin_hz: f32, lo_hz: f32, hi_hz: f32) -> Vec<f32> {
    let (lo, hi) = bin_range(mag.ncols(), bin_hz, lo_hz, hi_hz);
    mag.rows()
        .into_iter()
        .map(|row| {
            if hi <= lo {
                0.0
            } else {
                row.iter().skip(lo).take(hi - lo).map(|m| m * m).sum()
            }
        })
        .collect()
}

/// Per-frame total power, the denominator of every band ratio.
pub fn total_power_rows(mag: ArrayView2<f32>) -> Vec<f32> {
    mag.rows()
        .into_iter()
        .map(|r| r.iter().map(|m| m * m).sum())
        .collect()
}

fn bin_range(n_bins: usize, bin_hz: f32, lo_hz: f32, hi_hz: f32) -> (usize, usize) {
    let lo = ((lo_hz / bin_hz).ceil().max(0.0) as usize).min(n_bins);
    let hi = if hi_hz.is_finite() {
        (((hi_hz / bin_hz).floor() as usize) + 1).min(n_bins)
    } else {
        n_bins
    };
    (lo, hi)
}

/// Half-wave-rectified first-order difference of a per-frame series. The signal
/// a flux feature is made of: energy appearing, never energy leaving.
pub fn hwr_diff(series: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(series.len());
    out.push(0.0);
    for w in series.windows(2) {
        out.push((w[1] - w[0]).max(0.0));
    }
    out
}

/// Projects the spectrogram onto 84 semitone bins from C1, each bin summing the
/// FFT bins inside a semitone-wide window around its centre.
pub fn log_frequency(mag: ArrayView2<f32>, bin_hz: f32) -> Array2<f32> {
    let edges = semitone_edges(LOG_F_MIN_HZ, LOG_BINS);
    project(mag, bin_hz, &edges, LOG_BINS)
}

/// 12-bin chroma: the same semitone geometry as [`log_frequency`], folded by
/// pitch class over seven octaves from A0.
///
/// Folded rather than summed from a single octave because a bass line and the
/// pad above it both have to reach the same pitch class, which is the whole
/// point of the feature in the SSM.
pub fn chroma(mag: ArrayView2<f32>, bin_hz: f32) -> Array2<f32> {
    let n_semitones = CHROMA_OCTAVES * CHROMA_BINS;
    let edges = semitone_edges(CHROMA_F_MIN_HZ, n_semitones);
    let semi = project(mag, bin_hz, &edges, n_semitones);

    let mut out = Array2::<f32>::zeros((semi.nrows(), CHROMA_BINS));
    for (t, row) in semi.rows().into_iter().enumerate() {
        for (i, v) in row.iter().enumerate() {
            // A0 is pitch class 9 in a C-rooted chroma, so the fold is offset.
            out[[t, (i + 9) % CHROMA_BINS]] += *v;
        }
    }
    out
}

/// `n + 1` semitone-band edges in Hz, starting a half semitone below `f_min`.
fn semitone_edges(f_min: f32, n: usize) -> Vec<f32> {
    let step = 2f32.powf(1.0 / 12.0);
    let mut f = f_min / step.sqrt();
    let mut edges = Vec::with_capacity(n + 1);
    for _ in 0..=n {
        edges.push(f);
        f *= step;
    }
    edges
}

fn project(mag: ArrayView2<f32>, bin_hz: f32, edges: &[f32], n_out: usize) -> Array2<f32> {
    let n_bins = mag.ncols();
    // Precomputed once: the same bin range serves every frame, and resolving it
    // per frame is what turns a 45 ms projection into a second.
    let ranges: Vec<(usize, usize)> = (0..n_out)
        .map(|i| {
            let lo = ((edges[i] / bin_hz).ceil().max(0.0) as usize).min(n_bins);
            let hi = (((edges[i + 1] / bin_hz).ceil()).max(0.0) as usize).min(n_bins);
            (lo, hi.max(lo))
        })
        .collect();

    let mut out = Array2::<f32>::zeros((mag.nrows(), n_out));
    for (t, row) in mag.rows().into_iter().enumerate() {
        for (i, (lo, hi)) in ranges.iter().enumerate() {
            out[[t, i]] = row.iter().skip(*lo).take(hi - lo).sum();
        }
    }
    out
}
