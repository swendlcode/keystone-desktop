//! Spectral whitening, and the high-pass that runs before it.
//!
//! Both are bgate's additions to Faraldo's HPCP settings and both exist for the
//! same reason: a key template match should be decided by *which* pitch classes
//! are present, not by how a particular synth or master happened to distribute
//! energy across the spectrum. Without whitening, a track with a loud sub and a
//! dull top end matches a different profile from the same track mastered
//! brighter.

use std::f32::consts::PI;

/// RBJ-cookbook biquad high-pass, Butterworth Q, applied in place over one
/// frame.
///
/// Per frame rather than over the whole track, for two reasons. The chroma
/// stage's hop is larger than its window so frames never overlap and there is
/// no continuity to preserve; and filtering a decoded track means a second copy
/// of it, which is 100 MB on a ten-minute file and eight analysis workers hold
/// eight. The filter's start transient lands in the part of the frame the Hann
/// window has already multiplied by zero.
///
/// A time-domain filter rather than simply zeroing the low FFT bins: the point
/// is to stop a kick 40 dB above everything else leaking through the window's
/// sidelobes into the bass bins that carry the tonic, and zeroing bins
/// afterwards removes the kick's own bins but not its leakage.
pub fn high_pass_frame(frame: &mut [f32], sample_rate: u32, cutoff_hz: f32) {
    if frame.is_empty() || sample_rate == 0 || cutoff_hz <= 0.0 {
        return;
    }
    let nyquist = sample_rate as f32 * 0.5;
    if cutoff_hz >= nyquist {
        return;
    }
    let w0 = 2.0 * PI * cutoff_hz / sample_rate as f32;
    let (sin_w0, cos_w0) = (w0.sin(), w0.cos());
    // Q = 1/sqrt(2) is the maximally flat (Butterworth) second-order response,
    // so 2Q is sqrt(2).
    let alpha = sin_w0 * std::f32::consts::FRAC_1_SQRT_2;
    let a0 = 1.0 + alpha;
    let b0 = ((1.0 + cos_w0) * 0.5) / a0;
    let b1 = (-(1.0 + cos_w0)) / a0;
    let b2 = b0;
    let a1 = (-2.0 * cos_w0) / a0;
    let a2 = (1.0 - alpha) / a0;

    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for s in frame.iter_mut() {
        let x0 = *s;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y0;
        *s = y0;
    }
}

/// Third-octave resolution for the envelope. Coarse enough that it follows the
/// instrument's spectral slope rather than the partials it is meant to flatten.
pub const BANDS_PER_OCTAVE: usize = 3;

/// A band this far below the frame's loudest band is noise, and whitening must
/// not lift it to unity — that would turn the hiss between partials into
/// chroma evidence.
pub const ENVELOPE_FLOOR: f32 = 0.01;

/// Divides a magnitude frame by a smoothed estimate of its own spectral
/// envelope (Röbel & Rodet 2005, *Efficient spectral envelope estimation and
/// its application to pitch shifting and envelope preservation*).
///
/// A third-octave peak envelope, smoothed and interpolated in log frequency,
/// rather than their cepstral true-envelope iteration. The iteration is far
/// more expensive per frame and what the key stage actually needs from this
/// step is that no single partial dominates the profile match — which a peak
/// envelope delivers. Recorded as an approximation rather than claimed as
/// parity.
pub struct Whitener {
    /// Inclusive FFT-bin span of each band.
    bands: Vec<(usize, usize)>,
    /// Per bin: the lower band index and the interpolation weight toward the
    /// next band, precomputed so no allocation happens per frame.
    per_bin: Vec<(usize, f32)>,
    envelope: Vec<f32>,
    smoothed: Vec<f32>,
    first_bin: usize,
    last_bin: usize,
}

impl Whitener {
    pub fn new(n_bins: usize, bin_hz: f32, f_min_hz: f32, f_max_hz: f32, bands_per_octave: usize) -> Self {
        let mut bands = Vec::new();
        let mut centers = Vec::new();
        if n_bins >= 2 && bin_hz > 0.0 && f_max_hz > f_min_hz && bands_per_octave > 0 {
            let step = 2f32.powf(1.0 / bands_per_octave as f32);
            let half = step.sqrt();
            let mut fc = f_min_hz.max(bin_hz);
            while fc < f_max_hz {
                let lo = ((fc / half) / bin_hz).floor().max(0.0) as usize;
                let hi = (((fc * half) / bin_hz).ceil() as usize).min(n_bins - 1);
                if hi >= lo {
                    bands.push((lo, hi));
                    centers.push(fc / bin_hz);
                }
                fc *= step;
            }
        }
        let first_bin = bands.first().map(|b| b.0).unwrap_or(0);
        let last_bin = bands.last().map(|b| b.1).unwrap_or(0);
        let mut per_bin = vec![(0usize, 0.0f32); n_bins];
        if !centers.is_empty() {
            for (bin, slot) in per_bin.iter_mut().enumerate() {
                let x = bin as f32;
                let mut k = 0usize;
                while k + 1 < centers.len() && centers[k + 1] <= x {
                    k += 1;
                }
                let t = if k + 1 < centers.len() {
                    let span = centers[k + 1] - centers[k];
                    if span > 0.0 { ((x - centers[k]) / span).clamp(0.0, 1.0) } else { 0.0 }
                } else {
                    0.0
                };
                *slot = (k, t);
            }
        }
        let n_bands = bands.len();
        Self {
            bands,
            per_bin,
            envelope: vec![0.0; n_bands],
            smoothed: vec![0.0; n_bands],
            first_bin,
            last_bin,
        }
    }

    /// Flattens `magnitude` in place. A silent frame is left alone: dividing
    /// zeros by an envelope of zeros is how NaNs reach the chroma vector and
    /// then the key badge.
    pub fn apply(&mut self, magnitude: &mut [f32]) {
        if self.bands.is_empty() {
            return;
        }
        let mut loudest = 0.0f32;
        for (b, &(lo, hi)) in self.bands.iter().enumerate() {
            let mut peak = 0.0f32;
            for bin in lo..=hi.min(magnitude.len().saturating_sub(1)) {
                peak = peak.max(magnitude[bin]);
            }
            self.envelope[b] = peak;
            loudest = loudest.max(peak);
        }
        if loudest <= 0.0 {
            return;
        }
        let floor = loudest * ENVELOPE_FLOOR;
        let n = self.envelope.len();
        for b in 0..n {
            let prev = self.envelope[b.saturating_sub(1)];
            let next = self.envelope[(b + 1).min(n - 1)];
            self.smoothed[b] = (0.25 * prev + 0.5 * self.envelope[b] + 0.25 * next).max(floor);
        }
        for bin in 0..magnitude.len() {
            if bin < self.first_bin || bin > self.last_bin || bin >= self.per_bin.len() {
                continue;
            }
            let (k, t) = self.per_bin[bin];
            let lo = self.smoothed[k.min(n - 1)];
            let hi = self.smoothed[(k + 1).min(n - 1)];
            let env = lo + (hi - lo) * t;
            if env > 0.0 {
                magnitude[bin] /= env;
            }
        }
    }
}
