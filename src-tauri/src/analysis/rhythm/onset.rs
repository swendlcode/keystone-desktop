//! S6 — the novelty front end: one full-band curve at 100 fps plus three
//! band-limited siblings.
//!
//! The kick curve is what settles the tempo octave and makes four-to-the-floor
//! trivial, so it is not an optional extra: autocorrelation cannot tell a
//! 170 BPM track from an 85 BPM one, and the kick band can.

use crate::analysis::rhythm::spectral::{self, BandMatrix};
use crate::error::Result;

pub const N_FFT: usize = 2048;
/// 44,100 / 441 = exactly 100 fps.
pub const HOP: usize = 441;
pub const FPS: f32 = 100.0;
pub const N_BANDS: usize = 64;
pub const BAND_F_MIN_HZ: f32 = 30.0;
/// `log(1 + 1000 * |X|)`.
pub const COMPRESSION_GAMMA: f32 = 1000.0;
/// Local-mean subtraction window: 1.5 s.
pub const LOCAL_MEAN_FRAMES: usize = 150;

pub const KICK_BAND_HZ: (f32, f32) = (30.0, 250.0);
pub const MID_BAND_HZ: (f32, f32) = (250.0, 2500.0);
pub const HAT_BAND_HZ: (f32, f32) = (2500.0, f32::INFINITY);

pub struct NoveltyCurves {
    pub fps: f32,
    pub full: Vec<f32>,
    pub kick: Vec<f32>,
    pub mid: Vec<f32>,
    pub hat: Vec<f32>,
}

impl NoveltyCurves {
    pub fn len(&self) -> usize {
        self.full.len()
    }

    pub fn is_empty(&self) -> bool {
        self.full.is_empty()
    }

    pub fn frame_to_ms(&self, frame: usize) -> f64 {
        frame as f64 * 1000.0 / self.fps as f64
    }

    pub fn ms_to_frame(&self, ms: f64) -> usize {
        (ms * self.fps as f64 / 1000.0).round().max(0.0) as usize
    }

    pub fn duration_ms(&self) -> f64 {
        self.frame_to_ms(self.len())
    }

}

/// Curve value at a fractional frame, taking the strongest sample inside
/// `tolerance` frames.
///
/// Every grid test downstream asks "is there an onset near here", and a point
/// sample answers "is there an onset exactly here" — which is false for most
/// real onsets, since the novelty peak of a kick sits a frame or two after the
/// nominal beat. The STFT is also uncentred, so the whole curve leads the audio
/// by up to half a window.
pub fn peak_near(curve: &[f32], frame: f64, tolerance: usize) -> f32 {
    if curve.is_empty() {
        return 0.0;
    }
    let centre = frame.round();
    if !centre.is_finite() || centre < 0.0 {
        return 0.0;
    }
    let centre = centre as usize;
    let lo = centre.saturating_sub(tolerance);
    let hi = (centre + tolerance).min(curve.len() - 1);
    if lo > hi {
        return 0.0;
    }
    curve[lo..=hi].iter().copied().fold(0.0f32, f32::max)
}

/// STFT at 2048 / 441, a sparse 64-band log filterbank from 30 Hz,
/// `log(1 + 1000|X|)` compression, half-wave-rectified first-order difference
/// summed across bands, local-mean subtraction over 150 frames, rectify, divide
/// by the global sd — and the same over each band split.
///
/// The hop is derived from the sample rate rather than fixed at 441, so the
/// 100 fps contract every frame index downstream assumes holds for a file the
/// decoder handed over at some other rate.
pub fn novelty(samples: &[f32], sample_rate: u32) -> Result<NoveltyCurves> {
    let hop = ((sample_rate as f32 / FPS).round() as usize).max(1);
    let fps = sample_rate as f32 / hop as f32;
    let bands = spectral::log_bands(
        N_FFT / 2 + 1,
        sample_rate,
        N_FFT,
        N_BANDS,
        BAND_F_MIN_HZ,
        sample_rate as f32 * 0.5,
    );
    let mags = spectral::band_magnitudes(samples, N_FFT, hop, &bands, COMPRESSION_GAMMA);

    let centres: Vec<f32> = bands.iter().map(|b| b.centre_hz).collect();
    let window = (LOCAL_MEAN_FRAMES as f32 * fps / FPS).round() as usize;
    let curve = |range: (f32, f32)| {
        let mut c = flux(&mags, &centres, range);
        local_normalize(&mut c, window.max(1));
        c
    };

    Ok(NoveltyCurves {
        fps,
        full: curve((0.0, f32::INFINITY)),
        kick: curve(KICK_BAND_HZ),
        mid: curve(MID_BAND_HZ),
        hat: curve(HAT_BAND_HZ),
    })
}

/// Half-wave-rectified first-order difference summed across the bands whose
/// centre falls in `range`.
fn flux(mags: &BandMatrix, centres: &[f32], range: (f32, f32)) -> Vec<f32> {
    if mags.n_frames == 0 {
        return Vec::new();
    }
    let selected: Vec<usize> = centres
        .iter()
        .enumerate()
        .filter(|(_, c)| **c >= range.0 && **c < range.1)
        .map(|(i, _)| i)
        .collect();
    let mut out = vec![0.0f32; mags.n_frames];
    for t in 1..mags.n_frames {
        let (prev, cur) = (mags.row(t - 1), mags.row(t));
        let mut acc = 0.0f32;
        for &b in &selected {
            acc += (cur[b] - prev[b]).max(0.0);
        }
        out[t] = acc;
    }
    out
}

/// Subtract the running mean over `window` frames, half-wave rectify, then
/// divide by the global standard deviation.
///
/// The window is what makes the curve comparable across a track whose loudness
/// changes: without it a quiet intro contributes nothing to the tempogram and a
/// loud drop contributes everything, which is how a long ambient opening ends
/// up setting the tempo.
pub fn local_normalize(curve: &mut [f32], window: usize) {
    let n = curve.len();
    if n == 0 {
        return;
    }
    let mut prefix = vec![0.0f64; n + 1];
    for i in 0..n {
        prefix[i + 1] = prefix[i] + curve[i] as f64;
    }
    let half = window / 2;
    for i in 0..n {
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(n);
        let mean = (prefix[hi] - prefix[lo]) / (hi - lo) as f64;
        curve[i] = (curve[i] as f64 - mean).max(0.0) as f32;
    }
    let mean: f64 = curve.iter().map(|v| *v as f64).sum::<f64>() / n as f64;
    let var: f64 = curve
        .iter()
        .map(|v| (*v as f64 - mean) * (*v as f64 - mean))
        .sum::<f64>()
        / n as f64;
    let sd = var.sqrt();
    if sd > 1e-9 {
        curve.iter_mut().for_each(|v| *v = (*v as f64 / sd) as f32);
    }
}

/// Local maxima above a threshold. Used by the energy stage's onset rate and by
/// S11's per-curve peak picking.
///
/// Ties inside the window resolve to the earliest frame, so a plateau reports
/// one onset at its start rather than one per frame.
pub fn pick_onsets(curve: &[f32], window_frames: usize, threshold: f32) -> Vec<usize> {
    let w = window_frames.max(1);
    let mut out = Vec::new();
    for i in 0..curve.len() {
        if curve[i] < threshold {
            continue;
        }
        let lo = i.saturating_sub(w);
        let hi = (i + w + 1).min(curve.len());
        let is_peak = curve[lo..i].iter().all(|v| *v < curve[i])
            && curve[i + 1..hi].iter().all(|v| *v <= curve[i]);
        if is_peak {
            out.push(i);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rhythm::fixtures;

    #[test]
    fn a_click_track_puts_a_novelty_peak_on_every_click() {
        let track = fixtures::click_track(128.0, 8.0, fixtures::Groove::FourToTheFloor);
        let c = novelty(&track.samples, fixtures::SAMPLE_RATE).unwrap();
        assert!((c.fps - 100.0).abs() < 0.01);
        let peaks = pick_onsets(&c.full, 10, 1.0);
        // 128 BPM over 8 s is about 17 beats, and four-to-the-floor puts a hat
        // on every off-beat, so the curve owes roughly 34 onsets.
        assert!(peaks.len() >= 30 && peaks.len() <= 36, "{} peaks", peaks.len());
    }

    #[test]
    fn the_kick_and_hat_curves_mark_different_halves_of_the_beat() {
        // The band split is not tested by comparing curve magnitudes: each
        // curve is divided by its own standard deviation, so their absolute
        // levels are not comparable by construction. What has to hold is the
        // property the octave rule depends on — the kick curve marks the beat
        // and the hat curve marks the off-beat, half a beat away.
        let track = fixtures::click_track(120.0, 12.0, fixtures::Groove::FourToTheFloor);
        let c = novelty(&track.samples, fixtures::SAMPLE_RATE).unwrap();
        // 120 BPM at 100 fps is 50 frames per beat.
        let phase = |curve: &[f32]| -> f64 {
            let peaks = pick_onsets(curve, 10, 0.8);
            assert!(peaks.len() > 8, "{} peaks", peaks.len());
            let (mut sin, mut cos) = (0.0f64, 0.0f64);
            for p in &peaks {
                let a = std::f64::consts::TAU * (*p % 50) as f64 / 50.0;
                sin += a.sin();
                cos += a.cos();
            }
            let mut angle = sin.atan2(cos);
            if angle < 0.0 {
                angle += std::f64::consts::TAU;
            }
            angle * 50.0 / std::f64::consts::TAU
        };
        let apart = (phase(&c.kick) - phase(&c.hat)).abs();
        let apart = apart.min(50.0 - apart);
        assert!(apart > 18.0, "curves are only {apart:.1} frames apart");
    }

    #[test]
    fn silence_produces_a_flat_curve_rather_than_a_division_by_zero() {
        let c = novelty(&vec![0.0f32; fixtures::SAMPLE_RATE as usize], fixtures::SAMPLE_RATE)
            .unwrap();
        assert!(c.full.iter().all(|v| v.abs() < 1e-6));
        assert!(pick_onsets(&c.full, 10, 1.0).is_empty());
    }

    #[test]
    fn a_plateau_reports_one_onset_at_its_start() {
        let curve = vec![0.0, 0.0, 5.0, 5.0, 5.0, 0.0, 0.0];
        assert_eq!(pick_onsets(&curve, 2, 1.0), vec![2]);
    }
}
