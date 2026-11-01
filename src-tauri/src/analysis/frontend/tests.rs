//! Shared signal generators for the front-end tests, and the two test modules
//! that use them.
//!
//! Everything above this layer inherits a scaling mistake made here as a key or
//! energy error with no visible cause, so the transform is pinned three ways: a
//! sine must land in one bin at a magnitude the window's coherent gain predicts
//! in closed form, silence must produce zeros, and Parseval's relation must
//! hold frame by frame.

mod reductions;
mod transform;

use ndarray::Array2;

use super::{Spectrogram, SpectrogramOpts, WindowKind};

pub(super) const SR: u32 = 44_100;



pub(super) fn opts(n_fft: usize, hop: usize, center: bool) -> SpectrogramOpts {
    SpectrogramOpts {
        n_fft,
        hop,
        window: WindowKind::Hann,
        center,
        normalize_by_sqrt_n: false,
    }
}

/// A sine at exactly the centre of `bin`, so there is no leakage to reason
/// about beyond the window's own main lobe.
pub(super) fn bin_sine(bin: usize, n_fft: usize, amplitude: f32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| {
            amplitude
                * (2.0 * std::f32::consts::PI * bin as f32 * i as f32 / n_fft as f32).sin()
        })
        .collect()
}

/// Deterministic noise. A seeded LCG rather than a crate: the test needs a
/// signal with energy in every bin, not statistical quality.
pub(super) fn noise(len: usize) -> Vec<f32> {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) as f32 / (1u64 << 31) as f32) - 1.0
        })
        .collect()
}

/// A sustained tone in one bin plus one broadband click: the separation the
/// structure and energy stages depend on, in its simplest statable form.
pub(super) fn tone_and_click(frames: usize, bins: usize, click_frame: usize) -> Spectrogram {
    let mut data = Array2::<f32>::zeros((frames, bins));
    for t in 0..frames {
        data[(t, bins / 2)] = 1.0;
    }
    for b in 0..bins {
        data[(click_frame, b)] += 1.0;
    }
    Spectrogram {
        data,
        n_fft: 4096,
        hop: 512,
        sample_rate: SR,
    }
}
