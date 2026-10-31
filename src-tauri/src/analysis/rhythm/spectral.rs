//! The rhythm path's own front end: STFT projected straight into log-spaced
//! bands, one frame at a time.
//!
//! `analysis::frontend` is the shared front end and this overlaps it. The
//! overlap is deliberate while the engine is being built: the rhythm path never
//! wants a magnitude spectrogram, only 64 band magnitudes per frame, and fusing
//! the band projection into the STFT loop holds one frame instead of
//! frames x 1025 -- 15 MB rather than 250 MB on a ten-minute track, times eight
//! workers. Nothing outside this file sees a spectrogram, so moving the rhythm
//! path onto the shared module later is a change to this file alone.

use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::sync::Arc;

/// One triangular band, stored as a bin offset plus its weights.
///
/// Sparse, never a dense matmul: the same spectrogram measured 45 ms through
/// this shape against 1.07 s through a dense multiply. A 24x penalty for a
/// two-line difference, so there is no version of this worth writing densely.
pub struct Band {
    pub start_bin: usize,
    pub weights: Vec<f32>,
    pub centre_hz: f32,
}

impl Band {
    fn project(&self, frame: &[f32]) -> f32 {
        let mut acc = 0.0f32;
        for (i, w) in self.weights.iter().enumerate() {
            acc += frame[self.start_bin + i] * w;
        }
        acc
    }
}

/// Frames x bands, row-major. A flat `Vec` rather than an `ndarray` because the
/// only access pattern here is "this row and the one before it".
pub struct BandMatrix {
    pub data: Vec<f32>,
    pub n_frames: usize,
    pub n_bands: usize,
}

impl BandMatrix {
    pub fn row(&self, t: usize) -> &[f32] {
        &self.data[t * self.n_bands..(t + 1) * self.n_bands]
    }
}

/// Log-spaced triangular bands between `f_min` and `f_max`.
///
/// Each band is normalised to unit weight sum, so a band magnitude is a
/// weighted average rather than a sum. Without that the wide high bands would
/// dominate the across-band flux sum purely because they cover more bins, and
/// the hat curve would drown the kick curve it is meant to be compared against.
pub fn log_bands(
    n_bins: usize,
    sample_rate: u32,
    n_fft: usize,
    n_bands: usize,
    f_min: f32,
    f_max: f32,
) -> Vec<Band> {
    let bin_hz = sample_rate as f32 / n_fft as f32;
    let lo = f_min.max(bin_hz);
    let hi = f_max.min(sample_rate as f32 * 0.5).max(lo * 2.0);
    let ratio = (hi / lo).ln() / (n_bands + 1) as f32;

    let edges: Vec<f32> = (0..n_bands + 2)
        .map(|i| lo * (ratio * i as f32).exp())
        .collect();

    let mut bands = Vec::with_capacity(n_bands);
    for i in 0..n_bands {
        let (left, centre, right) = (edges[i], edges[i + 1], edges[i + 2]);
        let first = (left / bin_hz).ceil().max(0.0) as usize;
        let last = ((right / bin_hz).floor() as usize).min(n_bins.saturating_sub(1));
        // A band narrower than one bin still has to exist, or the centre
        // frequency it contributes to the band-split masks goes missing.
        let last = last.max(first);
        let mut weights: Vec<f32> = (first..=last)
            .map(|b| {
                let f = b as f32 * bin_hz;
                if f <= centre {
                    ((f - left) / (centre - left).max(1e-6)).clamp(0.0, 1.0)
                } else {
                    ((right - f) / (right - centre).max(1e-6)).clamp(0.0, 1.0)
                }
            })
            .collect();
        let sum: f32 = weights.iter().sum();
        if sum > 0.0 {
            weights.iter_mut().for_each(|w| *w /= sum);
        } else {
            weights = vec![1.0];
        }
        bands.push(Band {
            start_bin: first,
            weights,
            centre_hz: centre,
        });
    }
    bands
}

/// STFT magnitudes projected into `bands` and compressed as
/// `log(1 + gamma * x)`, one row per frame.
///
/// Frames are not centred: frame `t` starts at sample `t * hop`. Onset times
/// therefore lead the audio by half a window, which the beat grid absorbs as a
/// constant offset -- but mixing centred and uncentred frames between stages
/// would shift one curve against another, so the convention is stated here and
/// not varied.
pub fn band_magnitudes(
    samples: &[f32],
    n_fft: usize,
    hop: usize,
    bands: &[Band],
    gamma: f32,
) -> BandMatrix {
    let n_bands = bands.len();
    if samples.len() < n_fft || n_bands == 0 || hop == 0 {
        return BandMatrix { data: Vec::new(), n_frames: 0, n_bands };
    }
    let n_frames = (samples.len() - n_fft) / hop + 1;
    let win = hann(n_fft);
    let fft = planner(n_fft);
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let mut mags = vec![0.0f32; spectrum.len()];
    let mut data = vec![0.0f32; n_frames * n_bands];

    for t in 0..n_frames {
        let off = t * hop;
        for i in 0..n_fft {
            input[i] = samples[off + i] * win[i];
        }
        // process_with_scratch rather than process: the scratch buffer is
        // allocated once instead of per frame, and there are ~36,000 frames in
        // a six-minute track.
        if fft
            .process_with_scratch(&mut input, &mut spectrum, &mut scratch)
            .is_err()
        {
            break;
        }
        magnitudes(&spectrum, &mut mags);
        let row = &mut data[t * n_bands..(t + 1) * n_bands];
        for (b, band) in bands.iter().enumerate() {
            row[b] = (1.0 + gamma * band.project(&mags)).ln();
        }
    }

    BandMatrix { data, n_frames, n_bands }
}

fn magnitudes(spectrum: &[Complex<f32>], out: &mut [f32]) {
    for (m, c) in out.iter_mut().zip(spectrum) {
        *m = c.norm();
    }
}

fn planner(n_fft: usize) -> Arc<dyn RealToComplex<f32>> {
    RealFftPlanner::<f32>::new().plan_fft_forward(n_fft)
}

/// Symmetric Hann, which is what every stage in the rhythm path uses. The
/// periodic variant belongs to model front ends trained with `center=True`, and
/// mixing the two shifts every frame by half a sample.
pub fn hann(n: usize) -> Vec<f32> {
    if n <= 1 {
        return vec![1.0; n];
    }
    (0..n)
        .map(|i| {
            let x = std::f32::consts::PI * i as f32 / (n - 1) as f32;
            x.sin() * x.sin()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_are_ordered_and_unit_weighted() {
        let bands = log_bands(1025, 44_100, 2048, 64, 30.0, 22_050.0);
        assert_eq!(bands.len(), 64);
        for pair in bands.windows(2) {
            assert!(pair[1].centre_hz > pair[0].centre_hz);
        }
        for b in &bands {
            let sum: f32 = b.weights.iter().sum();
            assert!((sum - 1.0).abs() < 1e-4, "band sum {sum}");
        }
    }

    #[test]
    fn a_sine_lands_in_the_band_that_covers_it() {
        let sr = 44_100u32;
        let samples: Vec<f32> = (0..sr as usize)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin())
            .collect();
        let bands = log_bands(1025, sr, 2048, 64, 30.0, 22_050.0);
        let m = band_magnitudes(&samples, 2048, 441, &bands, 1000.0);
        assert!(m.n_frames > 90);
        let row = m.row(50);
        let peak = row
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        let centre = bands[peak].centre_hz;
        assert!((centre - 1000.0).abs() < 150.0, "peak band at {centre} Hz");
    }

    #[test]
    fn a_frame_shorter_than_the_window_is_not_a_panic() {
        let bands = log_bands(1025, 44_100, 2048, 64, 30.0, 22_050.0);
        let m = band_magnitudes(&[0.0; 100], 2048, 441, &bands, 1000.0);
        assert_eq!(m.n_frames, 0);
    }
}
