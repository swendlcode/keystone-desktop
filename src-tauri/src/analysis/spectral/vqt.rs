//! The constant-Q / variable-Q front end.
//!
//! Log-spaced bins with a bandwidth that tracks their centre frequency, so one
//! bin means one musical interval everywhere rather than one fixed number of
//! hertz. That is what makes a chroma fold meaningful and what the 84-bin
//! log-frequency feature the structure stage wants is built from.
//!
//! Built as a **sparse filterbank over STFT magnitudes**, not as a time-domain
//! constant-Q transform. One STFT already feeds tuning and chroma, and a
//! genuine CQT needs a different window length per octave, which means several
//! more passes over the audio. The cost of the approximation is stated rather
//! than hidden: a bin whose bandwidth falls below one FFT bin cannot be
//! resolved, so it is widened to one bin and reads blurred. At `n_fft` 8192 and
//! 44.1 kHz that is everything under roughly 90 Hz at 12 bins per octave —
//! the bottom octave of a 7-octave chroma, which after the fold is mostly kick
//! anyway. A multi-resolution VQT is the upgrade path if a stage ever needs the
//! bottom octave sharp.
//!
//! Sparse, never a dense matrix multiply: the same shape measured 45 ms against
//! 1.07 s for a dense product on the onset filterbank, a 24x penalty for a
//! two-line difference.

/// Which log-frequency grid to build.
#[derive(Debug, Clone, Copy)]
pub struct VqtSpec {
    pub f_min: f32,
    pub bins_per_octave: usize,
    pub n_bins: usize,
    /// VQT bandwidth offset in Hz (Schörkhuber et al.). Zero is a pure
    /// constant-Q; a positive value trades frequency resolution for time
    /// resolution at the bottom, which is the right trade for onsets and the
    /// wrong one for chroma.
    pub gamma: f32,
}

impl VqtSpec {
    /// Seven octaves from A0, 12 bins per octave: the grid behind the
    /// beat-synchronous chroma.
    pub const CHROMA_7_OCTAVES: VqtSpec =
        VqtSpec { f_min: 27.5, bins_per_octave: 12, n_bins: 84, gamma: 0.0 };

    /// The structure stage's 84-bin log-frequency magnitude vector, from C1.
    pub const LOG_MAGNITUDE_84: VqtSpec =
        VqtSpec { f_min: 32.703_196, bins_per_octave: 12, n_bins: 84, gamma: 0.0 };
}

struct Band {
    start_bin: usize,
    weights: Vec<f32>,
}

/// A built kernel bank. Build once per stage and reuse across frames.
pub struct Vqt {
    bands: Vec<Band>,
    centers_hz: Vec<f32>,
    spec: VqtSpec,
    /// Pitch class of bin 0, so a chroma fold does not have to rediscover it.
    pitch_class_offset: usize,
}

impl Vqt {
    pub fn new(spec: VqtSpec, sample_rate: u32, n_fft: usize) -> Self {
        let mut bands = Vec::with_capacity(spec.n_bins);
        let mut centers_hz = Vec::with_capacity(spec.n_bins);
        let bin_hz = if n_fft > 0 { sample_rate as f32 / n_fft as f32 } else { 0.0 };
        let n_fft_bins = n_fft / 2 + 1;
        // Q for a filterbank whose -3 dB points meet: one over the fractional
        // spacing of adjacent bins.
        let q = 1.0 / (2f32.powf(1.0 / spec.bins_per_octave.max(1) as f32) - 1.0);
        for k in 0..spec.n_bins {
            let fc = spec.f_min * 2f32.powf(k as f32 / spec.bins_per_octave.max(1) as f32);
            centers_hz.push(fc);
            // Half-width equal to the spacing to the next bin, so adjacent Hann
            // responses overlap and sum to roughly unity.
            let half = (fc / q + spec.gamma).max(bin_hz);
            if bin_hz <= 0.0 || n_fft_bins == 0 || fc >= sample_rate as f32 * 0.5 {
                bands.push(Band { start_bin: 0, weights: Vec::new() });
                continue;
            }
            let lo = (((fc - half) / bin_hz).floor().max(0.0)) as usize;
            let hi = ((((fc + half) / bin_hz).ceil()).max(0.0) as usize).min(n_fft_bins - 1);
            if hi < lo {
                bands.push(Band { start_bin: 0, weights: Vec::new() });
                continue;
            }
            let mut weights = Vec::with_capacity(hi - lo + 1);
            let mut sum = 0.0f32;
            for bin in lo..=hi {
                let f = bin as f32 * bin_hz;
                let d = (f - fc).abs() / half;
                let w = if d >= 1.0 { 0.0 } else { 0.5 + 0.5 * (std::f32::consts::PI * d).cos() };
                sum += w;
                weights.push(w);
            }
            if sum > 0.0 {
                for w in weights.iter_mut() {
                    *w /= sum;
                }
            }
            bands.push(Band { start_bin: lo, weights });
        }
        let pitch_class_offset =
            ((12.0 * (spec.f_min / 440.0).log2() + 9.0).round() as i32).rem_euclid(12) as usize;
        Self { bands, centers_hz, spec, pitch_class_offset }
    }

    pub fn n_bins(&self) -> usize {
        self.bands.len()
    }

    pub fn center_hz(&self, bin: usize) -> f32 {
        self.centers_hz.get(bin).copied().unwrap_or(0.0)
    }

    /// Adds one magnitude frame's projection onto `out`, which must be at least
    /// [`Self::n_bins`] long. Accumulating rather than assigning is what lets a
    /// caller aggregate every frame inside a beat without a second buffer.
    pub fn accumulate(&self, magnitude: &[f32], out: &mut [f32]) {
        for (k, band) in self.bands.iter().enumerate() {
            if k >= out.len() {
                break;
            }
            let mut acc = 0.0f32;
            for (i, w) in band.weights.iter().enumerate() {
                let bin = band.start_bin + i;
                if bin >= magnitude.len() {
                    break;
                }
                acc += *w * magnitude[bin];
            }
            out[k] += acc;
        }
    }

    /// Folds a log-frequency vector onto the 12 pitch classes, 0 = C.
    ///
    /// Only meaningful when `bins_per_octave` is a multiple of 12; anything else
    /// has no pitch class to fold onto and returns zeros rather than a plausible
    /// wrong answer.
    pub fn fold_to_chroma(&self, bins: &[f32]) -> [f32; 12] {
        let mut out = [0.0f32; 12];
        let bpo = self.spec.bins_per_octave;
        if bpo == 0 || bpo % 12 != 0 {
            return out;
        }
        let per_semitone = bpo / 12;
        for (k, v) in bins.iter().enumerate().take(self.bands.len()) {
            let semitone = k / per_semitone;
            out[(semitone + self.pitch_class_offset) % 12] += *v;
        }
        out
    }
}
