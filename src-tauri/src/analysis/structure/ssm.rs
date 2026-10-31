//! Self-similarity and the Gaussian checkerboard novelty curve.
//!
//! Measured at 1.2 ms per feature at 384 strong beats, under 10 ms for all
//! seven. The stage is free, which is why there are seven features and not two.

use ndarray::{Array2, ArrayView2};

use crate::analysis::structure::PadMode;

/// Pairwise **standardised**-Euclidean distance.
///
/// Standardised, because the 84-bin vector's low bins carry an order of
/// magnitude more energy than its top ones, and plain Euclidean would let the
/// bass decide every boundary in the track.
pub fn distance_matrix(feature: ArrayView2<f32>) -> Array2<f32> {
    let n = feature.nrows();
    let d = feature.ncols();
    let mut out = Array2::<f32>::zeros((n, n));
    if n == 0 || d == 0 {
        return out;
    }

    let inv_sd = inverse_sds(feature);
    for i in 0..n {
        for j in (i + 1)..n {
            let mut acc = 0.0f32;
            for c in 0..d {
                let diff = (feature[[i, c]] - feature[[j, c]]) * inv_sd[c];
                acc += diff * diff;
            }
            let dist = acc.sqrt();
            out[[i, j]] = dist;
            out[[j, i]] = dist;
        }
    }
    out
}

/// `1 / sd` per dimension, with zero-variance dimensions neutralised rather
/// than producing infinities — a silent band is common in real tracks.
fn inverse_sds(feature: ArrayView2<f32>) -> Vec<f32> {
    let n = feature.nrows() as f32;
    (0..feature.ncols())
        .map(|c| {
            let col = feature.column(c);
            let mean = col.iter().sum::<f32>() / n;
            let var = col.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / n;
            let sd = var.sqrt();
            if sd > f32::EPSILON {
                1.0 / sd
            } else {
                0.0
            }
        })
        .collect()
}

/// Convolves the matrix diagonal with a Gaussian checkerboard kernel.
///
/// `kernel` is the kernel's **total** width in strong beats: 16 spans 8 bars and
/// so compares 4 bars against the 4 bars that follow, which is the phrase length
/// the whole cue pipeline is built around.
///
/// `distance` is a distance matrix, as [`distance_matrix`] returns; Foote's
/// kernel wants similarity, so it is inverted here rather than at every call
/// site.
pub fn checkerboard_novelty(distance: ArrayView2<f32>, kernel: usize, pad: PadMode) -> Vec<f32> {
    let n = distance.nrows();
    let half = (kernel / 2).max(1);
    if n == 0 {
        return Vec::new();
    }

    let max = distance.iter().cloned().fold(0.0f32, f32::max);
    let similarity = |i: isize, j: isize| -> f32 {
        if i < 0 || j < 0 || i >= n as isize || j >= n as isize {
            // Zero padding is the single detail that gives the intro a novelty
            // value at all, and so the only reason CUE 1 is detectable. Outside
            // the matrix there is no similarity, which is exactly the claim we
            // want to make about the silence before bar 1.
            return 0.0;
        }
        if max <= f32::EPSILON {
            return 1.0;
        }
        1.0 - distance[[i as usize, j as usize]] / max
    };

    let k = gaussian_checkerboard(half);
    let mut out = vec![0.0f32; n];
    let (lo, hi) = match pad {
        PadMode::Valid => (half.min(n), n.saturating_sub(half)),
        PadMode::Zero => (0, n),
    };

    for centre in lo..hi {
        let mut acc = 0.0f32;
        for (a, di) in (-(half as isize)..(half as isize)).enumerate() {
            for (b, dj) in (-(half as isize)..(half as isize)).enumerate() {
                acc += k[a][b] * similarity(centre as isize + di, centre as isize + dj);
            }
        }
        // Negative novelty means "more similar across the boundary than along
        // it", which is not a boundary.
        out[centre] = acc.max(0.0);
    }
    out
}

/// `2*half` square, `+1` on the two diagonal quadrants and `-1` on the
/// off-diagonal ones, tapered by a Gaussian so beats near the centre dominate.
fn gaussian_checkerboard(half: usize) -> Vec<Vec<f32>> {
    let size = 2 * half;
    let sigma = half as f32 / 2.0;
    let mut k = vec![vec![0.0f32; size]; size];
    let mut norm = 0.0f32;
    for (a, row) in k.iter_mut().enumerate() {
        for (b, cell) in row.iter_mut().enumerate() {
            let i = a as f32 - half as f32 + 0.5;
            let j = b as f32 - half as f32 + 0.5;
            let g = (-(i * i + j * j) / (2.0 * sigma * sigma)).exp();
            let sign = if (i < 0.0) == (j < 0.0) { 1.0 } else { -1.0 };
            *cell = sign * g;
            norm += g;
        }
    }
    if norm > f32::EPSILON {
        for row in k.iter_mut() {
            for cell in row.iter_mut() {
                *cell /= norm;
            }
        }
    }
    k
}
