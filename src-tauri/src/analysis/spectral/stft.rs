//! The short-time Fourier front end for the key path, plus the two things built
//! directly on top of it: a streaming frame loop and sub-bin spectral peak
//! interpolation.
//!
//! Streaming rather than a materialised spectrogram, deliberately. A five
//! minute track framed at hop 512 is 25,800 frames of 1,025 bins — 106 MB — and
//! eight analysis workers would each hold one. [`Stft::process`] hands every
//! frame to a closure out of one reused buffer, so the peak cost is a single
//! frame however long the track is.
//!
//! This is a separate front end from `analysis::frontend`, which materialises
//! `Array2` spectrograms for the 100 fps onset / structure / energy path. The
//! two regimes genuinely differ: chroma samples a 93 ms window every 371 ms and
//! never overlaps them, and the peak interpolation and whitening here have no
//! consumer over there.

use std::f32::consts::PI;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

/// Hann window.
///
/// `periodic` selects the DFT-even form (`n` in the denominator) that a model
/// front end trained with `center=True` expects; the symmetric form (`n - 1`) is
/// what the chroma and tuning stages use. Mixing the two shifts every frame by
/// half a sample, which surfaces as a quiet bias in the tuning estimate rather
/// than as an error anybody would notice.
pub fn hann(n: usize, periodic: bool) -> Vec<f32> {
    if n <= 1 {
        return vec![1.0; n];
    }
    let denom = if periodic { n as f32 } else { (n - 1) as f32 };
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / denom).cos())
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub struct StftOpts {
    pub n_fft: usize,
    /// May legitimately exceed `n_fft`. The chroma stage samples frames every
    /// 371 ms, each 93 ms long, deliberately not overlapped — that is not a typo
    /// waiting to be tidied up.
    pub hop: usize,
    pub periodic: bool,
}

impl StftOpts {
    pub fn new(n_fft: usize, hop: usize) -> Self {
        Self { n_fft, hop, periodic: false }
    }
}

/// One planned real-input FFT with its reusable buffers. Build it once per
/// stage, not once per frame: planning is the expensive part.
pub struct Stft {
    r2c: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    magnitude: Vec<f32>,
    opts: StftOpts,
}

impl Stft {
    pub fn new(opts: StftOpts) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(opts.n_fft);
        let input = r2c.make_input_vec();
        let spectrum = r2c.make_output_vec();
        let magnitude = vec![0.0; spectrum.len()];
        let window = hann(opts.n_fft, opts.periodic);
        Self { r2c, window, input, spectrum, magnitude, opts }
    }

    /// `n_fft / 2 + 1`.
    pub fn n_bins(&self) -> usize {
        self.magnitude.len()
    }

    pub fn bin_hz(&self, sample_rate: u32) -> f32 {
        sample_rate as f32 / self.opts.n_fft as f32
    }

    /// How many frames `len` samples yield.
    ///
    /// A frame is kept while at least half of it holds real samples, and is
    /// zero-padded past the end. A frame that is mostly padding has a spectrum
    /// shaped more by the zeros than by the music; at hop 16384 it is also one
    /// eight-hundredth of the evidence, so it is not worth the distortion. An
    /// input shorter than half a window still yields one frame, because a 3 s
    /// chunk is a legitimate input and returning nothing would read as silence.
    pub fn n_frames(&self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        let half = (self.opts.n_fft / 2).min(len);
        1 + len.saturating_sub(half) / self.opts.hop
    }

    /// Runs `pre` over each raw frame before windowing — that is where the
    /// chroma stage's high-pass goes — then hands the magnitude spectrum to
    /// `on_frame` as `&mut`, since whitening rewrites it in place.
    pub fn process<P, F>(&mut self, samples: &[f32], mut pre: P, mut on_frame: F)
    where
        P: FnMut(&mut [f32]),
        F: FnMut(usize, &mut [f32]),
    {
        let frames = self.n_frames(samples.len());
        for i in 0..frames {
            let start = i * self.opts.hop;
            let avail = samples.len().saturating_sub(start).min(self.opts.n_fft);
            self.input[..avail].copy_from_slice(&samples[start..start + avail]);
            for v in self.input[avail..].iter_mut() {
                *v = 0.0;
            }
            pre(&mut self.input);
            for (v, w) in self.input.iter_mut().zip(self.window.iter()) {
                *v *= *w;
            }
            if self.r2c.process(&mut self.input, &mut self.spectrum).is_err() {
                continue;
            }
            for (m, c) in self.magnitude.iter_mut().zip(self.spectrum.iter()) {
                *m = c.norm();
            }
            on_frame(i, &mut self.magnitude);
        }
    }
}

/// One interpolated spectral peak.
#[derive(Debug, Clone, Copy)]
pub struct Peak {
    pub freq_hz: f32,
    pub magnitude: f32,
}

/// Peaks weaker than this fraction of the frame's loudest bin are noise for our
/// purposes and only blur the circular mean and the chroma.
pub const DEFAULT_PEAK_FLOOR_RATIO: f32 = 0.005;

/// Local maxima with three-point parabolic interpolation on the log magnitude.
///
/// Log magnitude, not linear: the main lobe of a Hann-windowed sinusoid is very
/// nearly a parabola in dB, and interpolating there puts the frequency error
/// under a hundredth of a bin. Without this the tuning estimate has no
/// sub-semitone resolution at all — at 4096 points and 44.1 kHz one bin is
/// 10.8 Hz, which at 440 Hz is 42 cents, larger than the whole range S2 is
/// trying to measure.
pub fn spectral_peaks(
    magnitude: &[f32],
    bin_hz: f32,
    lo_hz: f32,
    hi_hz: f32,
    floor_ratio: f32,
    out: &mut Vec<Peak>,
) {
    out.clear();
    if magnitude.len() < 3 || bin_hz <= 0.0 {
        return;
    }
    let loudest = magnitude.iter().copied().fold(0.0f32, f32::max);
    if loudest <= 0.0 {
        return;
    }
    let floor = loudest * floor_ratio;
    let lo = ((lo_hz / bin_hz).ceil().max(1.0)) as usize;
    let hi = ((hi_hz / bin_hz).floor().max(0.0) as usize).min(magnitude.len() - 2);
    if lo > hi {
        return;
    }
    for k in lo..=hi {
        let m = magnitude[k];
        if m > floor && m >= magnitude[k - 1] && m > magnitude[k + 1] {
            if let Some(p) = interpolate(magnitude, k, bin_hz) {
                out.push(p);
            }
        }
    }
}

fn interpolate(magnitude: &[f32], k: usize, bin_hz: f32) -> Option<Peak> {
    const EPS: f32 = 1e-20;
    let a = magnitude[k - 1].max(EPS).ln();
    let b = magnitude[k].max(EPS).ln();
    let c = magnitude[k + 1].max(EPS).ln();
    let denom = a - 2.0 * b + c;
    if !denom.is_finite() || denom.abs() < 1e-9 {
        // A flat top carries no sub-bin information; report the bin centre
        // rather than dropping evidence the chroma stage can still use.
        return Some(Peak { freq_hz: k as f32 * bin_hz, magnitude: magnitude[k] });
    }
    let delta = 0.5 * (a - c) / denom;
    if !delta.is_finite() || delta.abs() > 1.0 {
        return None;
    }
    let log_peak = b - 0.25 * (a - c) * delta;
    if !log_peak.is_finite() {
        return None;
    }
    Some(Peak { freq_hz: (k as f32 + delta) * bin_hz, magnitude: log_peak.exp() })
}
