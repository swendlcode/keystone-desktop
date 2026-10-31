//! S7, part one: the autocorrelation comb tempogram.

use std::ops::Range;

use crate::analysis::rhythm::onset::NoveltyCurves;
use crate::error::Result;

pub const BPM_MIN: f64 = 50.0;
pub const BPM_MAX: f64 = 220.0;
pub const BPM_GRID_STEP: f64 = 0.1;
/// Comb harmonics, weighted 1/k.
pub const COMB_HARMONICS: usize = 4;

/// Lags 1..=120 frames, which at 100 fps reaches down to 50 BPM.
///
/// The ceiling is load-bearing rather than an economy. A harmonic `k * lag`
/// past it contributes **zero**, and that is the whole reason the comb
/// suppresses the sub-octave: on a 120 BPM signal the true lag of 50 frames
/// collects k=1 and k=2 for a salience of 1.500 while the half-tempo candidate
/// at lag 100 collects only k=1 for 1.000. Raise the ceiling and both
/// candidates collect every harmonic, the scores equalise, and the octave
/// decision falls entirely to the prior. ENGINE.md §2's verified numbers are
/// the numbers this ceiling produces.
pub const MAX_LAG_FRAMES: usize = 120;

/// Run the tempogram on the densest window by novelty RMS, not the whole file,
/// so a long ambient intro does not poison it.
pub const TEMPOGRAM_WINDOW_SECONDS: f32 = 60.0;

pub struct Tempogram {
    /// The 0.1 BPM grid from 50 to 220.
    pub bpm: Vec<f64>,
    pub salience: Vec<f64>,
}

impl Tempogram {
    /// Ascending-BPM argmax, so a tie between grid points that round to the
    /// same lag resolves to the lowest of them. That is what makes the 120 BPM
    /// reference case report 119.5 rather than an arbitrary member of its tie.
    pub fn best(&self) -> Option<(f64, f64)> {
        let mut best: Option<(f64, f64)> = None;
        for (i, s) in self.salience.iter().enumerate() {
            if best.is_none_or(|(_, bs)| *s > bs) {
                best = Some((self.bpm[i], *s));
            }
        }
        best
    }

    /// Salience at the grid point nearest `bpm`.
    pub fn salience_at(&self, bpm: f64) -> Option<f64> {
        if self.bpm.is_empty() {
            return None;
        }
        let i = ((bpm - BPM_MIN) / BPM_GRID_STEP).round();
        if i < 0.0 || i as usize >= self.bpm.len() {
            return None;
        }
        Some(self.salience[i as usize])
    }

    /// Strongest peak that is a genuinely different grid from `bpm`.
    ///
    /// Not simply the runner-up, and not simply the non-octave runner-up. The
    /// 120-frame ceiling guarantees that every small-integer multiple and
    /// submultiple of the true tempo is a strong comb peak: a 174 BPM track
    /// scores 1.12 at 116, a 128 BPM track scores 0.81 at 51.2. Those are the
    /// same beat grid counted at a different metrical level, not a rival
    /// reading, and a confidence measured against them reports every correctly
    /// detected track as uncertain.
    ///
    /// So the competitor must (a) not sit within 5% of `p/q` of `bpm` for any
    /// `p, q` up to 5, and (b) be a local maximum of the salience curve, which
    /// is what keeps the shoulder of the winning peak from counting as a rival.
    pub fn competing_peak(&self, bpm: f64) -> Option<(f64, f64)> {
        let mut best: Option<(f64, f64)> = None;
        for i in 1..self.salience.len().saturating_sub(1) {
            let s = self.salience[i];
            if s <= self.salience[i - 1] || s < self.salience[i + 1] {
                continue;
            }
            if is_metrical_relative(self.bpm[i], bpm) {
                continue;
            }
            if best.is_none_or(|(_, bs)| s > bs) {
                best = Some((self.bpm[i], s));
            }
        }
        best
    }

    /// Kept under its original name because it is the documented entry point;
    /// it is [`Self::competing_peak`].
    pub fn second_best_non_octave(&self, bpm: f64) -> Option<(f64, f64)> {
        self.competing_peak(bpm)
    }
}

/// Within 5% of `p/q` of `b`, for `p` and `q` up to 5.
fn is_metrical_relative(a: f64, b: f64) -> bool {
    if b <= 0.0 || a <= 0.0 {
        return false;
    }
    let r = a / b;
    (1..=5u32).any(|p| {
        (1..=5u32).any(|q| (r / (p as f64 / q as f64) - 1.0).abs() < 0.05)
    })
}

/// Normalised autocorrelation of a mean-centred curve, clamped at zero.
///
/// Mean-centring first is what makes an absent harmonic score zero instead of
/// the DC floor of a rectified curve, and the comb's 1/k weighting only
/// suppresses the sub-octave if the gaps between peaks really are zero.
pub fn autocorrelation(curve: &[f32], max_lag: usize) -> Vec<f64> {
    let n = curve.len();
    let mut acf = vec![0.0f64; max_lag + 1];
    if n < 2 {
        return acf;
    }
    let mean: f64 = curve.iter().map(|v| *v as f64).sum::<f64>() / n as f64;
    let centred: Vec<f64> = curve.iter().map(|v| *v as f64 - mean).collect();
    let energy: f64 = centred.iter().map(|v| v * v).sum::<f64>() / n as f64;
    if energy <= 1e-12 {
        return acf;
    }
    acf[0] = 1.0;
    for lag in 1..=max_lag.min(n - 1) {
        let mut acc = 0.0f64;
        for t in 0..n - lag {
            acc += centred[t] * centred[t + lag];
        }
        acf[lag] = (acc / (n - lag) as f64 / energy).max(0.0);
    }
    acf
}

/// `score = sum over k of ACF(round(k * lag)) / k`, on the 0.1 BPM grid.
pub fn comb(acf: &[f64], fps: f64) -> Tempogram {
    let steps = ((BPM_MAX - BPM_MIN) / BPM_GRID_STEP).round() as usize + 1;
    let mut bpm = Vec::with_capacity(steps);
    let mut salience = Vec::with_capacity(steps);
    for i in 0..steps {
        let candidate = BPM_MIN + i as f64 * BPM_GRID_STEP;
        let lag = 60.0 * fps / candidate;
        let mut score = 0.0f64;
        for k in 1..=COMB_HARMONICS {
            let idx = (k as f64 * lag).round() as usize;
            if idx < acf.len() {
                score += acf[idx] / k as f64;
            }
        }
        bpm.push(candidate);
        salience.push(score);
    }
    Tempogram { bpm, salience }
}

/// ACF comb over the densest [`TEMPOGRAM_WINDOW_SECONDS`] of novelty.
pub fn tempogram(curves: &NoveltyCurves) -> Result<Tempogram> {
    let window = densest_window(curves, TEMPOGRAM_WINDOW_SECONDS);
    let slice = &curves.full[window];
    let acf = autocorrelation(slice, MAX_LAG_FRAMES);
    Ok(comb(&acf, curves.fps as f64))
}

/// Index range of the densest `seconds` window by novelty RMS.
pub fn densest_window(curves: &NoveltyCurves, seconds: f32) -> Range<usize> {
    let n = curves.len();
    let width = (seconds * curves.fps).round() as usize;
    if n == 0 || width == 0 || n <= width {
        return 0..n;
    }
    let mut prefix = vec![0.0f64; n + 1];
    for i in 0..n {
        let v = curves.full[i] as f64;
        prefix[i + 1] = prefix[i] + v * v;
    }
    let mut best = (0usize, f64::NEG_INFINITY);
    for start in 0..=n - width {
        let energy = prefix[start + width] - prefix[start];
        if energy > best.1 {
            best = (start, energy);
        }
    }
    best.0..best.0 + width
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rhythm::{fixtures, onset};

    /// A unit impulse train at lag 50, which is 120 BPM at 100 fps.
    fn delta_acf(lag: usize) -> Vec<f64> {
        let mut acf = vec![0.0f64; MAX_LAG_FRAMES + 1];
        acf[0] = 1.0;
        let mut k = lag;
        while k <= MAX_LAG_FRAMES {
            acf[k] = 1.0;
            k += lag;
        }
        acf
    }

    #[test]
    fn the_comb_reproduces_the_verified_reference_numbers() {
        // ENGINE.md §2: on a 120 BPM test signal the comb returns 119.5 BPM at
        // salience 1.500 with the half-tempo second at 1.000. Both numbers fall
        // out of MAX_LAG_FRAMES, so this test is the guard on that ceiling.
        let t = comb(&delta_acf(50), 100.0);
        let (bpm, salience) = t.best().unwrap();
        assert!((bpm - 119.5).abs() < 1e-9, "best was {bpm}");
        assert!((salience - 1.5).abs() < 1e-9, "salience was {salience}");

        let half = t.bpm.iter().position(|b| (*b - 60.0).abs() < 1e-9).unwrap();
        assert!((t.salience[half] - 1.0).abs() < 1e-9);
        assert!(salience > t.salience[half]);
    }

    #[test]
    fn a_pure_pulse_train_has_no_competing_grid() {
        // Every peak a clean impulse train produces is a metrical level of the
        // same grid, so there is nothing for the confidence to measure against
        // and `None` is the honest answer. A runner-up here would be the
        // half-tempo peak, which is exactly what must not count.
        let t = comb(&delta_acf(50), 100.0);
        let (bpm, _) = t.best().unwrap();
        assert!(t.competing_peak(bpm).is_none());
    }

    #[test]
    fn metrical_relatives_are_recognised_and_unrelated_tempi_are_not() {
        for ratio in [0.5, 2.0 / 3.0, 0.75, 1.0, 1.5, 2.0, 2.5] {
            assert!(is_metrical_relative(128.0 * ratio, 128.0), "{ratio}");
        }
        // 128 against 113 is 0.883, which is no small-integer ratio: a real
        // rival reading rather than the same grid recounted.
        assert!(!is_metrical_relative(113.0, 128.0));
        assert!(!is_metrical_relative(128.0, 0.0));
    }

    #[test]
    fn the_densest_window_moves_past_an_ambient_intro() {
        let f = fixtures::ambient_intro(128.0, 20.0, 20.0);
        let c = onset::novelty(&f.samples, fixtures::SAMPLE_RATE).unwrap();
        let w = densest_window(&c, 15.0);
        // The window must sit inside the drums, which start at 20 s = frame
        // 2000, not in the pad that precedes them.
        assert!(w.start > 1500, "window started at frame {}", w.start);
    }

    #[test]
    fn a_curve_shorter_than_the_window_returns_the_whole_curve() {
        let c = NoveltyCurves {
            fps: 100.0,
            full: vec![1.0; 50],
            kick: Vec::new(),
            mid: Vec::new(),
            hat: Vec::new(),
        };
        assert_eq!(densest_window(&c, 60.0), 0..50);
        assert_eq!(densest_window(&c, 0.0), 0..50);
    }

    #[test]
    fn silence_has_no_tempo_rather_than_a_random_one() {
        let acf = autocorrelation(&[0.0f32; 500], MAX_LAG_FRAMES);
        assert!(acf.iter().all(|v| *v == 0.0));
    }
}
