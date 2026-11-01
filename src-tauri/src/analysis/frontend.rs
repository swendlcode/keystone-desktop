//! Shared DSP front end: STFT, filterbanks, HPSS and band splits. Everything
//! onsets, structure and energy have in common lives here so none of them
//! recomputes a spectrogram the others already have.
//!
//! Layout:
//!
//! - [`stft`] — the frame loop that materialises a frames x bins magnitude
//!   matrix, with the centring and scaling conventions stated once.
//! - [`banks`] — the sparse triangular filterbanks and the log compression
//!   applied straight after a band projection.
//! - [`hpss`] — harmonic/percussive separation by median filtering.
//! - [`curves`] — the per-frame reductions: band energy, flux, local
//!   normalisation.
//! - [`spectrum`] — per-frame spectral operations: high-pass, whitening,
//!   centroid, sub-bin peak position.
//!
//! Everything above this layer inherits this layer's errors, and a scaling
//! mistake here surfaces as a key-detection accuracy loss with no visible
//! cause. That is why the tests pin signals whose spectrum can be stated
//! exactly — one sine in one bin, silence, and Parseval — rather than only
//! checking that the shapes line up.

mod banks;
mod curves;
mod hpss;
mod spectrum;
mod stft;

#[cfg(test)]
mod tests;

use ndarray::Array2;

pub use self::banks::{log_compress, log_filterbank, mel_filterbank};
pub use self::curves::{band_energy, hwr_flux, local_normalize};
pub use self::hpss::hpss;
pub use self::spectrum::{high_pass, parabolic_peak, spectral_centroid, spectral_whitening};
pub use self::stft::stft_magnitude;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    /// Symmetric. What the chroma and onset stages use.
    Hann,
    /// Periodic, which is what a model front end trained with `center=True`
    /// reflect padding expects. Mixing the two shifts every frame.
    HannPeriodic,
}

#[derive(Debug, Clone)]
pub struct SpectrogramOpts {
    pub n_fft: usize,
    pub hop: usize,
    pub window: WindowKind,
    /// Reflect-pad so frame 0 is centred on sample 0.
    pub center: bool,
    /// Divide magnitudes by `sqrt(n_fft)`. Required by the Beat This! front
    /// end; wrong for everything else.
    pub normalize_by_sqrt_n: bool,
}

/// Magnitude spectrogram, frames x bins.
pub struct Spectrogram {
    pub data: Array2<f32>,
    pub n_fft: usize,
    pub hop: usize,
    pub sample_rate: u32,
}

impl Spectrogram {
    pub fn n_frames(&self) -> usize {
        self.data.nrows()
    }

    pub fn n_bins(&self) -> usize {
        self.data.ncols()
    }

    pub fn fps(&self) -> f32 {
        self.sample_rate as f32 / self.hop as f32
    }

    pub fn bin_hz(&self) -> f32 {
        self.sample_rate as f32 / self.n_fft as f32
    }
}

/// Hann, by the raised-cosine definition.
///
/// The two forms differ only in the denominator, and the difference is half a
/// sample of shift plus a slightly different coherent gain — `sum(w)` is
/// `n/2` exactly for the periodic form and `(n-1)/2` for the symmetric one. A
/// stage that windows with one and scales as if it had used the other is wrong
/// by that ratio in every magnitude it produces, which is why the choice is a
/// parameter the caller states rather than a default buried in the frame loop.
pub fn window(kind: WindowKind, n: usize) -> Vec<f32> {
    if n <= 1 {
        return vec![1.0; n];
    }
    let denom = match kind {
        WindowKind::Hann => (n - 1) as f32,
        WindowKind::HannPeriodic => n as f32,
    };
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / denom).cos())
        .collect()
}

/// One band of a triangular filterbank, stored sparsely.
pub struct FilterBand {
    pub start_bin: usize,
    pub weights: Vec<f32>,
}

/// A sparse triangular filterbank.
///
/// Sparse, never a dense matmul: the same spectrogram measured 45 ms through
/// this shape against 1.07 s through a dense multiply. A 24x penalty for a
/// two-line difference, so there is no version of this worth writing densely.
pub struct TriangularFilterbank {
    pub bands: Vec<FilterBand>,
}

impl TriangularFilterbank {
    /// Projects one magnitude frame into `out`, which must be `bands.len()` long.
    ///
    /// A band is truncated rather than skipped when the frame is shorter than
    /// the bank was built for: a caller that hands over a cropped frame gets
    /// the part of the band the frame covers, not a panic halfway through a
    /// track.
    pub fn apply(&self, frame: &[f32], out: &mut [f32]) {
        debug_assert_eq!(out.len(), self.bands.len());
        for (band, slot) in self.bands.iter().zip(out.iter_mut()) {
            *slot = 0.0;
            if band.start_bin >= frame.len() {
                continue;
            }
            let bins = &frame[band.start_bin..];
            let mut acc = 0.0f32;
            for (m, w) in bins.iter().zip(band.weights.iter()) {
                acc += *m * *w;
            }
            *slot = acc;
        }
    }

    pub fn n_bands(&self) -> usize {
        self.bands.len()
    }
}
