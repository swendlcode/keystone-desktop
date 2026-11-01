//! Harmonic/percussive separation by median filtering (Fitzgerald 2010, with
//! Driedger's soft masks).
//!
//! The idea in one line: a sustained note is a ridge along time and an outlier
//! along frequency, a drum hit is the reverse, so a median along time keeps the
//! note and a median along frequency keeps the hit.
//!
//! This is by a long way the most expensive thing in the shared front end — at
//! 4096/512 it is 21 million medians per minute of audio — so both passes walk
//! the spectrogram one row at a time. The obvious shape, "gather a bin's
//! column, filter it, scatter it back", puts 8 KB between consecutive reads;
//! measured per minute of audio, release build, one M-series core: 1.58 s that
//! way, 1.04 s row-wise, 0.72 s row-wise with the window update below.
//!
//! Even so it is ~3.6 s of the per-track budget on a five minute track, which
//! is more than §2 of the engine plan allows for the whole structure stage. The
//! separation is only ever consumed as a per-frame total, so the way to close
//! that gap is to hand this stage a spectrogram with fewer bins rather than to
//! micro-optimise the filter further — that decision belongs to the caller
//! that chooses the transform size, not here.

use ndarray::Array2;

use super::Spectrogram;
use crate::error::{KeystoneError, Result};

/// Masks are built from the **squared** medians.
///
/// Soft masks rather than binary ones, and squared rather than linear, because
/// `h²/(h²+p²)` is both softer in the ambiguous middle and sharper at the
/// extremes than the linear ratio. The two masks sum to exactly 1, so
/// `harmonic + percussive == magnitude` cell by cell — S10's
/// `perc / (perc + harm)` is only a ratio of the mix if the two components
/// actually add back up to it.
const MASK_POWER: i32 = 2;

/// Harmonic/percussive separation by median filtering: median across time gives
/// the harmonic component, across frequency the percussive one. Roughly 17
/// frames x 17 bins at hop 512.
///
/// **On those lengths.** At 44.1 kHz and hop 512 a frame is 11.6 ms, so 17
/// frames is 198 ms: long enough that a kick or a snare is a clear outlier in
/// the time median, short enough that a chord change two frames later is not
/// smeared into the harmonic component. Longer and sustained-but-moving
/// material (a filter sweep, a pitch bend) stops looking harmonic; shorter and
/// a 200 ms tom roll starts to.
///
/// 17 bins at 4096/44.1 kHz is 10.8 Hz per bin, so 183 Hz: wider than the few
/// bins a single partial occupies, which is what makes a partial an outlier in
/// the frequency median, and that is the whole mechanism. It is also the limit
/// of the method in the bass — below about 180 Hz the harmonics of a bassline
/// are spaced closer than the filter is wide, so the median stops rejecting
/// them and some bass leaks into the percussive component. Widening the filter
/// makes that worse rather than better, since it then spans more harmonics
/// still; the honest answer is that the kick band is where this separation is
/// weakest, and the band-limited flux features exist alongside it for exactly
/// that reason.
///
/// Both lengths are forced odd so the median has a true centre element.
pub fn hpss(
    spec: &Spectrogram,
    time_median: usize,
    freq_median: usize,
) -> Result<(Spectrogram, Spectrogram)> {
    let (frames, bins) = (spec.data.nrows(), spec.data.ncols());
    if frames == 0 || bins == 0 {
        let empty = || Array2::<f32>::zeros((frames, bins));
        return Ok((wrap(spec, empty()), wrap(spec, empty())));
    }

    // `as_slice` is None only for a non-standard layout, which no caller
    // produces; the copy is the correctness backstop, not the normal path.
    let copied: Option<Vec<f32>> = match spec.data.as_slice() {
        Some(_) => None,
        None => Some(spec.data.iter().copied().collect()),
    };
    let mag: &[f32] = match (spec.data.as_slice(), &copied) {
        (Some(s), _) => s,
        (None, Some(v)) => v,
        (None, None) => unreachable!(),
    };

    let mut harmonic = vec![0.0f32; frames * bins];
    let mut percussive = vec![0.0f32; frames * bins];
    along_time(mag, frames, bins, time_median, &mut harmonic);
    along_frequency(mag, frames, bins, freq_median, &mut percussive);
    mask(mag, &mut harmonic, &mut percussive);

    Ok((
        wrap(spec, shape(frames, bins, harmonic)?),
        wrap(spec, shape(frames, bins, percussive)?),
    ))
}

/// One sorted window per bin, all advanced together as the frames go past.
///
/// Holding 2,049 windows at once is 139 KB, which sits in L2, and in exchange
/// every read of the spectrogram and every write of the output is sequential.
fn along_time(mag: &[f32], frames: usize, bins: usize, width: usize, out: &mut [f32]) {
    let w = odd(width);
    if w == 1 {
        out.copy_from_slice(mag);
        return;
    }
    let r = (w / 2) as isize;
    let row = |t: isize| {
        let t = t.clamp(0, frames as isize - 1) as usize;
        &mag[t * bins..(t + 1) * bins]
    };

    let mut windows = vec![0.0f32; bins * w];
    for k in -r..=r {
        let src = row(k);
        let slot = (k + r) as usize;
        for b in 0..bins {
            windows[b * w + slot] = src[b];
        }
    }
    for b in 0..bins {
        windows[b * w..(b + 1) * w].sort_by(f32::total_cmp);
    }

    for t in 0..frames as isize {
        if t > 0 {
            let (leaving, entering) = (row(t - 1 - r), row(t + r));
            for b in 0..bins {
                replace(&mut windows[b * w..(b + 1) * w], leaving[b], entering[b]);
            }
        }
        let dst = &mut out[t as usize * bins..(t as usize + 1) * bins];
        for (b, slot) in dst.iter_mut().enumerate() {
            *slot = windows[b * w + w / 2];
        }
    }
}

fn along_frequency(mag: &[f32], frames: usize, bins: usize, width: usize, out: &mut [f32]) {
    let w = odd(width);
    let mut win: Vec<f32> = Vec::with_capacity(w);
    for t in 0..frames {
        let (lo, hi) = (t * bins, (t + 1) * bins);
        median_line(&mag[lo..hi], w, &mut out[lo..hi], &mut win);
    }
}

/// Sliding median over `width` samples, edges held at the end value.
///
/// Edges replicate the first and last value rather than padding with zeros.
/// Zero padding would halve the time median over the opening frames, which
/// reads as "no harmonic content at the start of the track" — and the start of
/// the track is precisely where S11 looks for the intro cue.
fn median_line(signal: &[f32], w: usize, out: &mut [f32], win: &mut Vec<f32>) {
    let n = signal.len();
    debug_assert_eq!(out.len(), n);
    if n == 0 {
        return;
    }
    let r = (w / 2) as isize;
    let at = |i: isize| signal[i.clamp(0, n as isize - 1) as usize];
    win.clear();
    win.extend((-r..=r).map(at));
    win.sort_by(f32::total_cmp);
    out[0] = win[w / 2];
    for i in 1..n as isize {
        replace(win, at(i - 1 - r), at(i + r));
        out[i as usize] = win[w / 2];
    }
}

/// Odd, so there is a centre element. A width wider than the data is left
/// alone: the clamped edges fill it, which is the right answer for a very short
/// input rather than something to reject.
fn odd(width: usize) -> usize {
    width.max(1) | 1
}

/// Swaps one value out of a sorted window and another in, keeping it sorted
/// and its length fixed.
///
/// `copy_within` of the span between the two positions, not a fresh sort per
/// output and not an insert plus a remove: at 21 million outputs per minute of
/// audio, growing and shrinking the window measured 1.04 s against 0.72 s for
/// the fixed-length shift here.
///
/// The positions come from counting how many values are smaller, which the
/// window being sorted makes equivalent to a binary search and which measured
/// faster at this width. Plain `<` is enough because the magnitudes are finite
/// by construction — the STFT guards that — so it agrees with the total order
/// the window was sorted under; a NaN that did arrive would misplace its own
/// window and nothing else.
fn replace(win: &mut [f32], old: f32, new: f32) {
    let i = win.iter().filter(|x| **x < old).count().min(win.len() - 1);
    let j = win.iter().filter(|x| **x < new).count();
    if j <= i {
        win.copy_within(j..i, j + 1);
        win[j] = new;
    } else {
        win.copy_within(i + 1..j, i);
        win[j - 1] = new;
    }
}

/// Turns the two medians into the two components, in place.
fn mask(mag: &[f32], harmonic: &mut [f32], percussive: &mut [f32]) {
    for ((h, p), m) in harmonic.iter_mut().zip(percussive.iter_mut()).zip(mag.iter()) {
        let hp = h.powi(MASK_POWER);
        let pp = p.powi(MASK_POWER);
        let total = hp + pp;
        if total <= f32::MIN_POSITIVE {
            *h = 0.0;
            *p = 0.0;
        } else {
            *h = *m * hp / total;
            *p = *m * pp / total;
        }
    }
}

fn shape(frames: usize, bins: usize, data: Vec<f32>) -> Result<Array2<f32>> {
    Array2::from_shape_vec((frames, bins), data)
        .map_err(|e| KeystoneError::Analysis(format!("hpss shape: {e}")))
}

fn wrap(spec: &Spectrogram, data: Array2<f32>) -> Spectrogram {
    Spectrogram {
        data,
        n_fft: spec.n_fft,
        hop: spec.hop,
        sample_rate: spec.sample_rate,
    }
}
