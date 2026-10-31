//! EBU R128 short-term loudness, the core region, and `dynamic_span`.
//!
//! Loudness appears in the energy model exactly twice, and never as a level:
//! once to find the core region, and once as the *spread* between two
//! percentiles. Both are differences, so both are unchanged by gain — which is
//! the property §7.3's first test exists to protect.

use ebur128::{EbuR128, Mode};

use crate::analysis::energy::score::percentile;
use crate::error::{KeystoneError, Result};

/// The EBU R128 short-term window.
pub const SHORT_TERM_SECONDS: f64 = 3.0;
/// One measurement per 100 ms, which is R128's own short-term overlap.
pub const SHORT_TERM_STEP_MS: f64 = 100.0;

/// The core region is the contiguous span between the first and last downbeat
/// where short-term loudness is within this of the track's 95th-percentile
/// short-term loudness.
///
/// Without it a 7-minute club edit reads lower than its own radio edit, purely
/// because of the intro and outro it is allowed to have.
pub const CORE_LU_WINDOW: f32 = 6.0;

/// A fully compressed master can show a spread near zero; the reciprocal needs
/// a floor or `dynamic_span` goes to infinity on exactly the loudest tracks.
const SPAN_FLOOR_LU: f32 = 0.1;

pub struct ShortTerm {
    /// One value per [`SHORT_TERM_STEP_MS`]. Non-finite where the window is not
    /// yet full or the audio is silent, and every reader filters for that.
    pub lufs: Vec<f32>,
    pub step_ms: f64,
    pub window_ms: f64,
}

impl ShortTerm {
    pub fn measure(samples: &[f32], sample_rate: u32) -> Result<Self> {
        let mut meter = EbuR128::new(1, sample_rate, Mode::S)
            .map_err(|e| KeystoneError::Analysis(format!("loudness meter: {e}")))?;
        let block = ((sample_rate as f64 * SHORT_TERM_STEP_MS / 1000.0).round() as usize).max(1);
        let mut lufs = Vec::with_capacity(samples.len() / block + 1);

        for chunk in samples.chunks(block) {
            meter
                .add_frames_f32(chunk)
                .map_err(|e| KeystoneError::Analysis(format!("loudness frames: {e}")))?;
            lufs.push(meter.loudness_shortterm().unwrap_or(f64::NEG_INFINITY) as f32);
        }

        Ok(Self {
            lufs,
            step_ms: SHORT_TERM_STEP_MS,
            window_ms: SHORT_TERM_SECONDS * 1000.0,
        })
    }

    /// Assembles the series directly, for tests and for a cached re-score.
    pub fn from_values(lufs: Vec<f32>, step_ms: f64) -> Self {
        Self {
            lufs,
            step_ms,
            window_ms: SHORT_TERM_SECONDS * 1000.0,
        }
    }

    /// Centre of measurement `i`. The meter reports the window that has just
    /// closed, so the value describes the 3 s *behind* it; attributing it to the
    /// window's end would push the core region half a window late.
    pub fn centre_ms(&self, i: usize) -> f64 {
        (i + 1) as f64 * self.step_ms - self.window_ms / 2.0
    }

    pub fn finite(&self) -> Vec<f32> {
        self.lufs.iter().copied().filter(|v| v.is_finite()).collect()
    }

    pub fn in_span(&self, span: (f64, f64)) -> Vec<f32> {
        self.lufs
            .iter()
            .enumerate()
            .filter(|(i, v)| {
                v.is_finite() && {
                    let c = self.centre_ms(*i);
                    c >= span.0 && c <= span.1
                }
            })
            .map(|(_, v)| *v)
            .collect()
    }
}

/// `1 / (P95 − P50)` of short-term loudness over the span.
///
/// The feature table says "95th − 50th percentile, **inverted**". A reciprocal
/// is the inversion that survives the score's `ln` step as a clean sign flip,
/// so a tightly compressed section reads high and a dynamic one reads low
/// without a second constant to tune.
pub fn dynamic_span(st: &ShortTerm, span: (f64, f64)) -> f32 {
    let values = st.in_span(span);
    if values.is_empty() {
        return 1.0 / SPAN_FLOOR_LU;
    }
    let spread = percentile(&values, 0.95) - percentile(&values, 0.50);
    1.0 / spread.max(SPAN_FLOOR_LU)
}

/// The core region, as a millisecond span snapped to downbeats.
pub fn core_region(st: &ShortTerm, downbeats_ms: &[f64]) -> Result<(f64, f64)> {
    let first = *downbeats_ms.first().unwrap_or(&0.0);
    let last = *downbeats_ms.last().unwrap_or(&0.0);
    if last <= first {
        return Err(KeystoneError::Analysis(
            "core region needs at least two downbeats".into(),
        ));
    }

    let finite = st.finite();
    if finite.is_empty() {
        return Ok((first, last));
    }
    let floor = percentile(&finite, 0.95) - CORE_LU_WINDOW;
    let run = longest_run(&st.lufs, floor);

    let Some((lo, hi)) = run else {
        return Ok((first, last));
    };
    let start = st.centre_ms(lo).max(first);
    let end = st.centre_ms(hi).min(last);
    if end - start < 1.0 {
        return Ok((first, last));
    }
    Ok((snap_up(downbeats_ms, start), snap_down(downbeats_ms, end)))
}

/// Longest contiguous run of measurements at or above `floor`.
fn longest_run(lufs: &[f32], floor: f32) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    let mut start: Option<usize> = None;
    for (i, v) in lufs.iter().enumerate() {
        let inside = v.is_finite() && *v >= floor;
        match (inside, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                best = longer(best, (s, i - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        best = longer(best, (s, lufs.len() - 1));
    }
    best
}

fn longer(a: Option<(usize, usize)>, b: (usize, usize)) -> Option<(usize, usize)> {
    match a {
        Some(prev) if prev.1 - prev.0 >= b.1 - b.0 => Some(prev),
        _ => Some(b),
    }
}

fn snap_up(downbeats_ms: &[f64], ms: f64) -> f64 {
    *downbeats_ms
        .iter()
        .find(|d| **d >= ms)
        .unwrap_or_else(|| downbeats_ms.first().unwrap_or(&ms))
}

fn snap_down(downbeats_ms: &[f64], ms: f64) -> f64 {
    *downbeats_ms
        .iter()
        .rev()
        .find(|d| **d <= ms)
        .unwrap_or_else(|| downbeats_ms.last().unwrap_or(&ms))
}
