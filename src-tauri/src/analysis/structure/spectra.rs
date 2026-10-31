//! The one spectral pass S9, S10 and S11 share.
//!
//! The expensive parts — the STFT and the HPSS median filters — come from
//! `analysis::frontend`, computed once. Everything here is a reduction over
//! matrices that already exist, held together so the three stages read the same
//! numbers: a per-section energy has to be comparable with the track energy, and
//! the cheapest way to guarantee that is for both to be looking at one set of
//! series.

use ndarray::Array2;

use crate::analysis::frontend::{self, Spectrogram, SpectrogramOpts, WindowKind};
use crate::analysis::structure::bands;
use crate::analysis::structure::{HPSS_FREQ_MEDIAN, HPSS_TIME_MEDIAN};
use crate::error::Result;

/// 4096 at hop 512, both at 44.1 kHz.
///
/// The hop is S9's stated 512; the window is 4096 rather than 2048 because the
/// 84-bin vector starts at C1 (32.7 Hz) and a 2048-point window puts only one
/// FFT bin below 43 Hz, which collapses the kick band the structure features
/// depend on.
pub const N_FFT: usize = 4096;
pub const HOP: usize = 512;

/// Band edges, shared by S9's flux features and S10's spectral ratios so the
/// two stages cannot drift apart.
pub const KICK_HZ: (f32, f32) = (30.0, 150.0);
pub const SNARE_HZ: (f32, f32) = (150.0, 800.0);
pub const HAT_HZ: (f32, f32) = (6000.0, f32::INFINITY);

pub struct Spectra {
    pub magnitude: Spectrogram,
    pub harmonic: Spectrogram,
    pub percussive: Spectrogram,
}

impl Spectra {
    /// S1's decode feeds this and nothing else re-reads the file.
    pub fn compute(samples: &[f32], sample_rate: u32) -> Result<Self> {
        let opts = SpectrogramOpts {
            n_fft: N_FFT,
            hop: HOP,
            window: WindowKind::Hann,
            center: true,
            normalize_by_sqrt_n: false,
        };
        let magnitude = frontend::stft_magnitude(samples, sample_rate, &opts)?;
        let (harmonic, percussive) =
            frontend::hpss(&magnitude, HPSS_TIME_MEDIAN, HPSS_FREQ_MEDIAN)?;
        Ok(Self {
            magnitude,
            harmonic,
            percussive,
        })
    }

    /// Assembles the struct from matrices computed elsewhere. The energy tests
    /// build a track's spectrogram arithmetically so the seven features can be
    /// perturbed one at a time; see `energy::tests`.
    pub fn from_parts(
        magnitude: Array2<f32>,
        harmonic: Array2<f32>,
        percussive: Array2<f32>,
        sample_rate: u32,
    ) -> Self {
        let wrap = |data: Array2<f32>| Spectrogram {
            data,
            n_fft: N_FFT,
            hop: HOP,
            sample_rate,
        };
        Self {
            magnitude: wrap(magnitude),
            harmonic: wrap(harmonic),
            percussive: wrap(percussive),
        }
    }

    pub fn n_frames(&self) -> usize {
        self.magnitude.data.nrows()
    }

    pub fn bin_hz(&self) -> f32 {
        self.magnitude.sample_rate as f32 / self.magnitude.n_fft as f32
    }

    pub fn fps(&self) -> f64 {
        self.magnitude.sample_rate as f64 / self.magnitude.hop as f64
    }

    pub fn frame_to_ms(&self, frame: usize) -> f64 {
        frame as f64 * 1000.0 / self.fps()
    }

    pub fn ms_to_frame(&self, ms: f64) -> usize {
        (ms * self.fps() / 1000.0).round().max(0.0) as usize
    }
}

/// Per-frame series every downstream feature is built from.
pub struct Reductions {
    pub fps: f64,
    pub kick: Vec<f32>,
    pub snare: Vec<f32>,
    pub hat: Vec<f32>,
    pub total: Vec<f32>,
    /// Power, not magnitude: these three are the numerator and denominator of
    /// S10's `low_ratio` and `high_ratio`, which the feature table defines as
    /// energy ratios. See [`bands::band_power_rows`] for why the distinction
    /// decides whether the feature measures loudness or bandwidth.
    pub low_power: Vec<f32>,
    pub high_power: Vec<f32>,
    pub total_power: Vec<f32>,
    pub harmonic: Vec<f32>,
    pub percussive: Vec<f32>,
    /// Spectral centroid in **Hz**. The score's shared `log x` step is what
    /// turns it into the log-Hz quantity the feature table specifies, so
    /// storing Hz here keeps the log in one place.
    pub centroid: Vec<f32>,
    /// Half-wave-rectified flux over the 84 semitone bins, divided by the
    /// frame's own magnitude. Dimensionless, and therefore unmoved by gain.
    pub flux: Vec<f32>,
    pub log_mag: Array2<f32>,
    pub chroma: Array2<f32>,
}

impl Reductions {
    pub fn new(spectra: &Spectra) -> Self {
        let mag = spectra.magnitude.data.view();
        let bin_hz = spectra.bin_hz();
        let log_mag = bands::log_frequency(mag, bin_hz);

        Self {
            fps: spectra.fps(),
            kick: bands::band_rows(mag, bin_hz, KICK_HZ.0, KICK_HZ.1),
            snare: bands::band_rows(mag, bin_hz, SNARE_HZ.0, SNARE_HZ.1),
            hat: bands::band_rows(mag, bin_hz, HAT_HZ.0, HAT_HZ.1),
            total: bands::total_rows(mag),
            low_power: bands::band_power_rows(mag, bin_hz, KICK_HZ.0, KICK_HZ.1),
            high_power: bands::band_power_rows(mag, bin_hz, HAT_HZ.0, HAT_HZ.1),
            total_power: bands::total_power_rows(mag),
            harmonic: bands::total_rows(spectra.harmonic.data.view()),
            percussive: bands::total_rows(spectra.percussive.data.view()),
            centroid: centroid_rows(mag, bin_hz),
            flux: normalised_flux(&log_mag),
            chroma: bands::chroma(mag, bin_hz),
            log_mag,
        }
    }

    pub fn n_frames(&self) -> usize {
        self.total.len()
    }

    pub fn frame_to_ms(&self, frame: usize) -> f64 {
        frame as f64 * 1000.0 / self.fps
    }

    pub fn ms_to_frame(&self, ms: f64) -> usize {
        (ms * self.fps / 1000.0).round().max(0.0) as usize
    }
}

fn centroid_rows(mag: ndarray::ArrayView2<f32>, bin_hz: f32) -> Vec<f32> {
    mag.rows()
        .into_iter()
        .map(|row| {
            let total: f32 = row.iter().sum();
            if total <= f32::EPSILON {
                return 0.0;
            }
            let weighted: f32 = row
                .iter()
                .enumerate()
                .map(|(b, m)| b as f32 * bin_hz * *m)
                .sum();
            weighted / total
        })
        .collect()
}

/// `Σ_b max(0, m_b[t] − m_b[t−1]) / ‖m[·,t]‖₂`.
///
/// The division is the feature table's "÷ frame RMS", read as the frame's RMS
/// level across the 84 bands — which is `‖m‖₂ / √n`, so dividing the summed rise
/// by `√n · rms` is dividing by the L2 norm. That is not cosmetic: an
/// unnormalised flux is proportional to level, which is the loudness trap the
/// whole energy design exists to avoid, and §7.3's invariance test fails on the
/// first `+6 dB` case without it.
///
/// Dividing by `n · rms` instead — the mean rise against the rms — would also be
/// gain-invariant, but it divides a sum over all bands by a count of all bands
/// twice over and lands the feature two orders of magnitude below where the
/// constants expect it.
fn normalised_flux(log_mag: &Array2<f32>) -> Vec<f32> {
    let mut out = vec![0.0f32; log_mag.nrows()];
    for t in 1..log_mag.nrows() {
        let prev = log_mag.row(t - 1);
        let cur = log_mag.row(t);
        let rise: f32 = cur
            .iter()
            .zip(prev.iter())
            .map(|(c, p)| (*c - *p).max(0.0))
            .sum();
        let norm = cur.iter().map(|v| v * v).sum::<f32>().sqrt();
        out[t] = if norm > f32::EPSILON { rise / norm } else { 0.0 };
    }
    out
}
