//! The triangular filterbanks and the compression applied straight after a
//! band projection.
//!
//! Both banks are built once per configuration and reused for every frame of
//! every track: the weight table is a function of `(n_fft, sample_rate,
//! n_bands, f_min, f_max)` and nothing else, so recomputing it per frame would
//! be 25,800 identical tables per track.

use super::{FilterBand, TriangularFilterbank};

/// How a band's weights are scaled once its triangle is laid over the bins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Norm {
    /// Weights sum to 1, so a band magnitude is a weighted average of its
    /// bins.
    UnitSum,
    /// The triangle's apex is 1 and the weights sum to roughly the band's width
    /// in bins.
    UnitPeak,
}

/// Log-spaced bands, which is what the onset front end uses: 64 bands from
/// 30 Hz at 100 fps.
///
/// Unit-sum weights. A band magnitude is then a weighted average rather than a
/// sum, which matters because S6 compares the band-limited flux curves against
/// each other: the bands above 2.5 kHz cover tens of bins each where the kick
/// band covers four, so unit-peak triangles would make the hat curve dominate
/// the across-band sum purely by width and the kick curve — the one that
/// settles the tempo octave — would be the quietest thing in the set.
pub fn log_filterbank(
    n_bins: usize,
    sample_rate: u32,
    n_fft: usize,
    n_bands: usize,
    f_min: f32,
    f_max: f32,
) -> TriangularFilterbank {
    let bin_hz = sample_rate as f32 / n_fft as f32;
    let (lo, hi) = span(bin_hz, sample_rate, f_min, f_max);
    let ratio = (hi / lo).ln() / (n_bands + 1) as f32;
    let edges: Vec<f32> = (0..n_bands + 2)
        .map(|i| lo * (ratio * i as f32).exp())
        .collect();
    build(&edges, n_bins, bin_hz, Norm::UnitSum)
}

/// Mel-spaced bands.
///
/// Unit-**peak** triangles and the HTK mel formula, which is what the model
/// front end this exists for was configured with. The choice is not
/// interchangeable with [`log_filterbank`]'s: a trained network encodes the
/// absolute level of its input features, so an area-normalised bank would feed
/// it mel energies an order of magnitude below anything it saw in training.
/// If a model's accuracy ever disagrees with its reference implementation,
/// this normalisation is the first knob to check, not the last.
pub fn mel_filterbank(
    n_bins: usize,
    sample_rate: u32,
    n_fft: usize,
    n_mels: usize,
    f_min: f32,
    f_max: f32,
) -> TriangularFilterbank {
    let bin_hz = sample_rate as f32 / n_fft as f32;
    let (lo, hi) = span(bin_hz, sample_rate, f_min, f_max);
    let (m_lo, m_hi) = (hz_to_mel(lo), hz_to_mel(hi));
    let step = (m_hi - m_lo) / (n_mels + 1) as f32;
    let edges: Vec<f32> = (0..n_mels + 2)
        .map(|i| mel_to_hz(m_lo + step * i as f32))
        .collect();
    build(&edges, n_bins, bin_hz, Norm::UnitPeak)
}

/// Clamps the requested range to what the transform can actually represent:
/// below one bin there is no resolution at all, and above Nyquist there are no
/// bins.
fn span(bin_hz: f32, sample_rate: u32, f_min: f32, f_max: f32) -> (f32, f32) {
    let lo = f_min.max(bin_hz);
    let hi = f_max.min(sample_rate as f32 * 0.5).max(lo * 2.0);
    (lo, hi)
}

/// `edges[i], edges[i+1], edges[i+2]` are one band's left, apex and right.
fn build(edges: &[f32], n_bins: usize, bin_hz: f32, norm: Norm) -> TriangularFilterbank {
    let n_bands = edges.len().saturating_sub(2);
    let mut bands = Vec::with_capacity(n_bands);
    for i in 0..n_bands {
        let (left, centre, right) = (edges[i], edges[i + 1], edges[i + 2]);
        let first = (left / bin_hz).ceil().max(0.0) as usize;
        let last = ((right / bin_hz).floor() as usize)
            .min(n_bins.saturating_sub(1))
            .max(first);
        let mut weights: Vec<f32> = (first..=last)
            .map(|b| triangle(b as f32 * bin_hz, left, centre, right))
            .collect();
        scale(&mut weights, norm);
        bands.push(FilterBand {
            start_bin: first.min(n_bins.saturating_sub(1)),
            weights,
        });
    }
    TriangularFilterbank { bands }
}

fn triangle(f: f32, left: f32, centre: f32, right: f32) -> f32 {
    if f <= centre {
        ((f - left) / (centre - left).max(1e-6)).clamp(0.0, 1.0)
    } else {
        ((right - f) / (right - centre).max(1e-6)).clamp(0.0, 1.0)
    }
}

/// A band narrower than one bin falls entirely between bin centres and comes
/// out all zeros. It still has to carry its bin: a zero band is a constant-zero
/// row in the band matrix, which contributes nothing to the flux sum and
/// silently removes a slice of the spectrum from every curve built on it.
fn scale(weights: &mut [f32], norm: Norm) {
    let sum: f32 = weights.iter().sum();
    if sum <= 0.0 {
        if let Some(w) = weights.first_mut() {
            *w = 1.0;
        }
        return;
    }
    match norm {
        Norm::UnitSum => weights.iter_mut().for_each(|w| *w /= sum),
        Norm::UnitPeak => {
            let peak = weights.iter().copied().fold(0.0f32, f32::max);
            if peak > 0.0 {
                weights.iter_mut().for_each(|w| *w /= peak);
            }
        }
    }
}

/// HTK mel, the form the model front ends in this engine were trained with.
fn hz_to_mel(f: f32) -> f32 {
    2595.0 * (1.0 + f / 700.0).log10()
}

fn mel_to_hz(m: f32) -> f32 {
    700.0 * (10.0f32.powf(m / 2595.0) - 1.0)
}

/// `log(1 + gamma * x)` compression, applied in place.
///
/// Clamped at zero first. Magnitudes are non-negative by construction, but a
/// caller that has already subtracted something (a whitened or background-
/// removed frame) can hand over a small negative value, and `ln` of a negative
/// is a NaN that propagates silently through the flux sum into the tempogram.
pub fn log_compress(frame: &mut [f32], gamma: f32) {
    for v in frame.iter_mut() {
        *v = (gamma * v.max(0.0)).ln_1p();
    }
}
