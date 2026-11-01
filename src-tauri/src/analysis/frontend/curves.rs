//! Per-frame reductions over a matrix that already exists: band energy, flux,
//! and the local normalisation that makes a curve comparable across a track.

use ndarray::Array2;

use super::Spectrogram;

/// Per-frame energy inside a frequency band.
///
/// A sum of **magnitudes**, not of squares. The distinction decides what the
/// number measures — a magnitude sum tracks how much of the band is present, a
/// power sum tracks its level — and the callers that need power square the
/// bins themselves so the two never silently swap places. The upper edge is
/// exclusive and may be infinite, which is how the hat band ("above 6 kHz") is
/// expressed.
pub fn band_energy(spec: &Spectrogram, lo_hz: f32, hi_hz: f32) -> Vec<f32> {
    let bin_hz = spec.bin_hz();
    let n_bins = spec.data.ncols();
    if bin_hz <= 0.0 || n_bins == 0 {
        return vec![0.0; spec.data.nrows()];
    }
    let first = (lo_hz / bin_hz).ceil().max(0.0) as usize;
    let last = if hi_hz.is_finite() {
        (hi_hz / bin_hz).ceil().max(0.0) as usize
    } else {
        n_bins
    };
    let (first, last) = (first.min(n_bins), last.min(n_bins));

    spec.data
        .rows()
        .into_iter()
        .map(|row| {
            if first >= last {
                return 0.0;
            }
            row.iter().skip(first).take(last - first).sum()
        })
        .collect()
}

/// Half-wave-rectified first-order difference, summed across bands.
///
/// Frame 0 is zero rather than absent: every curve in the engine is indexed by
/// frame and compared against the others frame for frame, so dropping the first
/// sample would shift one curve by 10 ms against the rest.
pub fn hwr_flux(bands: &Array2<f32>) -> Vec<f32> {
    let mut out = vec![0.0f32; bands.nrows()];
    for t in 1..bands.nrows() {
        let (prev, cur) = (bands.row(t - 1), bands.row(t));
        out[t] = cur
            .iter()
            .zip(prev.iter())
            .map(|(c, p)| (*c - *p).max(0.0))
            .sum();
    }
    out
}

/// Local-mean subtraction over a window, half-wave rectify, then divide by the
/// global standard deviation. The 1.5 s window is what makes the curve
/// comparable across a track whose loudness changes.
///
/// Without the local mean a quiet intro contributes nothing to the tempogram
/// and a drop contributes everything, which is how a long ambient opening ends
/// up setting the tempo for the whole track. The accumulation is in f64: the
/// prefix sum runs over 30,000 frames and the subtraction that follows is of
/// two nearly equal numbers, which is where an f32 prefix sum loses its
/// significant digits.
pub fn local_normalize(curve: &mut Vec<f32>, window_frames: usize) {
    let n = curve.len();
    if n == 0 {
        return;
    }
    let mut prefix = vec![0.0f64; n + 1];
    for i in 0..n {
        prefix[i + 1] = prefix[i] + curve[i] as f64;
    }
    let half = window_frames.max(1) / 2;
    for i in 0..n {
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(n);
        let mean = (prefix[hi] - prefix[lo]) / (hi - lo) as f64;
        curve[i] = (curve[i] as f64 - mean).max(0.0) as f32;
    }

    let mean = mean_of(curve);
    let var = curve
        .iter()
        .map(|v| {
            let d = *v as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n as f64;
    let sd = var.sqrt();
    // A silent or perfectly steady curve rectifies to all zeros, and dividing
    // it by its own zero spread would hand the peak picker NaNs instead of the
    // flat curve that is the honest answer.
    if sd > 1e-9 {
        curve.iter_mut().for_each(|v| *v = (*v as f64 / sd) as f32);
    }
}

fn mean_of(curve: &[f32]) -> f64 {
    curve.iter().map(|v| *v as f64).sum::<f64>() / curve.len() as f64
}
