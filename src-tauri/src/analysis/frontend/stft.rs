//! The frame loop: a materialised frames x bins magnitude matrix.
//!
//! Materialised, unlike the key path's streaming front end, because the
//! structure and energy stages read the same spectrogram several times over
//! and from both axes — a self-similarity matrix is not expressible as a single
//! pass over frames. The cost is real (a five minute track at 4096/512 is
//! 25,800 frames of 2,049 bins, 212 MB) which is why there is exactly one of
//! these per track and §2 of the engine plan budgets it once.
//!
//! Scratch buffers are allocated once per call and reused across frames: this
//! runs over every track in a library, and a per-frame allocation would be
//! 25,800 of them per track times eight workers.

use ndarray::Array2;
use realfft::RealFftPlanner;

use super::{window, Spectrogram, SpectrogramOpts};
use crate::error::{KeystoneError, Result};

pub fn stft_magnitude(
    samples: &[f32],
    sample_rate: u32,
    opts: &SpectrogramOpts,
) -> Result<Spectrogram> {
    if opts.n_fft < 2 || opts.n_fft % 2 != 0 {
        return Err(KeystoneError::Analysis(format!(
            "stft n_fft must be even and at least 2, got {}",
            opts.n_fft
        )));
    }
    if opts.hop == 0 {
        return Err(KeystoneError::Analysis("stft hop must be non-zero".into()));
    }

    let n_bins = opts.n_fft / 2 + 1;
    let frames = n_frames(samples.len(), opts.n_fft, opts.hop, opts.center);
    let mut data = vec![0.0f32; frames * n_bins];

    if frames > 0 {
        fill(samples, opts, n_bins, &mut data)?;
    }

    let data = Array2::from_shape_vec((frames, n_bins), data)
        .map_err(|e| KeystoneError::Analysis(format!("spectrogram shape: {e}")))?;
    Ok(Spectrogram {
        data,
        n_fft: opts.n_fft,
        hop: opts.hop,
        sample_rate,
    })
}

/// How many frames `len` samples yield.
///
/// Centred: frame `t` is centred on sample `t * hop`, so the count is the
/// librosa one and the last frames run past the end into padding. Uncentred:
/// frame `t` starts at sample `t * hop` and only whole frames count, except
/// that an input shorter than one window still yields one zero-padded frame —
/// a 3 s chunk is a legitimate input and returning nothing would read as
/// silence to everything downstream.
fn n_frames(len: usize, n_fft: usize, hop: usize, center: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if center {
        1 + len / hop
    } else if len >= n_fft {
        1 + (len - n_fft) / hop
    } else {
        1
    }
}

fn fill(samples: &[f32], opts: &SpectrogramOpts, n_bins: usize, data: &mut [f32]) -> Result<()> {
    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(opts.n_fft);
    let mut input = r2c.make_input_vec();
    let mut spectrum = r2c.make_output_vec();
    let mut scratch = r2c.make_scratch_vec();
    let win = window(opts.window, opts.n_fft);
    // 1/sqrt(n_fft) rather than 1/n_fft: the Beat This! front end divides the
    // magnitude by sqrt(n_fft), which is the unitary scaling, and its trained
    // weights encode that level. Everything else here wants the raw transform.
    let scale = if opts.normalize_by_sqrt_n {
        1.0 / (opts.n_fft as f32).sqrt()
    } else {
        1.0
    };
    let offset = if opts.center {
        -((opts.n_fft / 2) as isize)
    } else {
        0
    };

    for (t, row) in data.chunks_mut(n_bins).enumerate() {
        let start = offset + (t * opts.hop) as isize;
        gather(samples, start, opts.center, &mut input);
        for (v, w) in input.iter_mut().zip(win.iter()) {
            *v *= *w;
        }
        r2c.process_with_scratch(&mut input, &mut spectrum, &mut scratch)
            .map_err(|e| KeystoneError::Analysis(format!("fft: {e:?}")))?;
        for (m, c) in row.iter_mut().zip(spectrum.iter()) {
            let v = c.norm() * scale;
            // One non-finite sample — a corrupt float WAV, a decoder edge case —
            // would otherwise NaN a whole self-similarity row, the template
            // match and the model input, and the track would come back
            // analysed-but-empty with nothing to point at. Clamped here, at the
            // one place every magnitude in the engine is born.
            *m = if v.is_finite() { v } else { 0.0 };
        }
    }
    Ok(())
}

/// Copies one frame into `out`, reflecting at the edges when centred.
///
/// Reflection, not zero padding, for the centred case: a zero-padded first
/// frame is half silence, so the track's opening half-window reads as a fade-in
/// that is not in the audio, and S11's "intro start" cue is picked off exactly
/// that region. Reflecting at the boundary keeps the level of the first frames
/// honest. The padding is computed per sample instead of building a padded copy
/// of the signal, which on a ten minute track would be another 100 MB per
/// worker for the sake of `n_fft` samples at each end.
fn gather(samples: &[f32], start: isize, center: bool, out: &mut [f32]) {
    let len = samples.len() as isize;
    let n = out.len() as isize;
    if start >= 0 && start + n <= len {
        let s = start as usize;
        out.copy_from_slice(&samples[s..s + out.len()]);
        return;
    }
    for (i, slot) in out.iter_mut().enumerate() {
        let idx = start + i as isize;
        *slot = if idx >= 0 && idx < len {
            samples[idx as usize]
        } else if center {
            samples[reflect(idx, len)]
        } else {
            0.0
        };
    }
}

/// Mirrors an out-of-range index back into `[0, len)` without repeating the
/// edge sample, so index -1 reads sample 1.
fn reflect(i: isize, len: isize) -> usize {
    if len <= 1 {
        return 0;
    }
    let period = 2 * (len - 1);
    let mut m = i.rem_euclid(period);
    if m >= len {
        m = period - m;
    }
    m as usize
}
