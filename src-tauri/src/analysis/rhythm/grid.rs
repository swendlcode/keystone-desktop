//! S7, part three: laying a beat grid on the novelty curve, and the
//! least-squares refit that is the only honest source of two-decimal BPM.

/// Weight on the log-period penalty in the dynamic program.
///
/// Tuned against the novelty curve scaled to a unit peak: at 100 a 6% deviation
/// from the target period costs 0.34, about a third of a strong onset. Lower
/// and the grid chases every ghost note; higher and it ignores a real tempo
/// change, which would hide the thing `tempo_variable` exists to report.
const TIGHTNESS: f64 = 100.0;

/// Inter-beat intervals within this fraction of the median count as the same
/// tempo when measuring coverage.
const CONSISTENT_IBI_TOLERANCE: f64 = 0.06;

/// Beat times in ms, from a dynamic program over the novelty curve.
///
/// Ellis's formulation (2007): maximise the onset strength collected at the
/// chosen beats minus a penalty on every inter-beat interval that departs from
/// the target period. It is used here rather than a fixed grid laid from one
/// anchor because a fixed grid accumulates its own error: 0.5% off at 128 BPM
/// is 15 ms by beat 64 and a bar out by the end of a seven-minute track, which
/// would put every late cue on the wrong beat.
pub fn track_beats(curve: &[f32], fps: f64, period_frames: f64) -> Vec<f64> {
    let n = curve.len();
    if n == 0 || !(period_frames > 1.0) {
        return Vec::new();
    }
    let peak = curve.iter().copied().fold(0.0f32, f32::max).max(1e-6) as f64;
    let scaled: Vec<f64> = curve.iter().map(|v| *v as f64 / peak).collect();

    let lo_gap = (period_frames * 0.5).round().max(1.0) as usize;
    let hi_gap = (period_frames * 2.0).round() as usize;
    let mut score = vec![f64::NEG_INFINITY; n];
    let mut back = vec![usize::MAX; n];

    for t in 0..n {
        let mut best = (scaled[t], usize::MAX);
        let from_hi = t.saturating_sub(hi_gap);
        let from_lo = t.saturating_sub(lo_gap);
        for prev in from_hi..=from_lo {
            if prev >= t || score[prev] == f64::NEG_INFINITY {
                continue;
            }
            let gap = (t - prev) as f64 / period_frames;
            let penalty = TIGHTNESS * gap.ln() * gap.ln();
            let candidate = score[prev] + scaled[t] - penalty;
            if candidate > best.0 {
                best = (candidate, prev);
            }
        }
        score[t] = best.0;
        back[t] = best.1;
    }

    backtrace(&score, &back, fps, period_frames)
}

/// Walks back from the best ending beat.
///
/// The end is taken from the last `period` frames rather than the global
/// argmax, because the cumulative score grows with the number of beats and the
/// global maximum is therefore always near the end anyway — but only "near",
/// and picking the true argmax truncates the grid before a quiet outro.
fn backtrace(score: &[f64], back: &[usize], fps: f64, period_frames: f64) -> Vec<f64> {
    let n = score.len();
    let tail = n.saturating_sub(period_frames.round() as usize);
    let mut end = tail;
    for t in tail..n {
        if score[t] > score[end] {
            end = t;
        }
    }
    if score[end] == f64::NEG_INFINITY {
        return Vec::new();
    }
    let mut frames = vec![end];
    let mut cur = end;
    while back[cur] != usize::MAX {
        cur = back[cur];
        frames.push(cur);
    }
    frames.reverse();
    frames.iter().map(|f| *f as f64 * 1000.0 / fps).collect()
}

/// Least-squares fit of time against beat index, which is the only place
/// two-decimal BPM can come from.
///
/// The tempogram cannot produce them at all: integer lag quantisation at 100 fps
/// gives a BPM step of 1.32 at 90, 2.35 at 120, 2.66 at 128 and 5.04 at 174, and
/// 200 fps only halves it. With 10 ms beat jitter this fit has sd(BPM) around
/// 0.019 at 200 beats and 0.010 at 400 — which is where the second decimal
/// comes from, and why it is gated on the residual rather than on a setting.
///
/// Returns `(bpm, residual sd in ms)`.
pub fn refit_bpm(beats_ms: &[f64]) -> (f64, f64) {
    let n = beats_ms.len();
    if n < 2 {
        return (0.0, f64::INFINITY);
    }
    let mean_i = (n - 1) as f64 / 2.0;
    let mean_t: f64 = beats_ms.iter().sum::<f64>() / n as f64;
    let mut sxx = 0.0f64;
    let mut sxy = 0.0f64;
    for (i, t) in beats_ms.iter().enumerate() {
        let di = i as f64 - mean_i;
        sxx += di * di;
        sxy += di * (t - mean_t);
    }
    if sxx <= 0.0 || sxy <= 0.0 {
        return (0.0, f64::INFINITY);
    }
    let slope = sxy / sxx;
    let intercept = mean_t - slope * mean_i;
    let dof = (n as f64 - 2.0).max(1.0);
    let sse: f64 = beats_ms
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let r = t - (intercept + slope * i as f64);
            r * r
        })
        .sum();
    (60_000.0 / slope, (sse / dof).sqrt())
}

/// Fraction of the track spanned by the longest run of beats holding one tempo.
///
/// Measured as a span rather than a count because that is what the number is
/// used for: a confidence that says "the grid describes this much of the file".
/// Twenty consistent beats in a four-minute track is a usable grid for twenty
/// beats and nothing else.
pub fn coverage(beats_ms: &[f64], duration_ms: f64) -> f64 {
    if beats_ms.len() < 3 || duration_ms <= 0.0 {
        return 0.0;
    }
    let mut ibis: Vec<f64> = beats_ms.windows(2).map(|w| w[1] - w[0]).collect();
    let mut sorted = ibis.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    if median <= 0.0 {
        return 0.0;
    }
    ibis.iter_mut().for_each(|v| *v = (*v / median - 1.0).abs());

    let (mut best, mut run_start) = (0.0f64, 0usize);
    for i in 0..=ibis.len() {
        if i == ibis.len() || ibis[i] > CONSISTENT_IBI_TOLERANCE {
            let span = beats_ms[i] - beats_ms[run_start];
            best = best.max(span);
            run_start = i + 1;
        }
    }
    (best / duration_ms).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rhythm::{fixtures, onset};

    #[test]
    fn the_refit_recovers_an_exact_tempo_from_an_exact_grid() {
        let beats: Vec<f64> = (0..200).map(|i| i as f64 * 60_000.0 / 128.37).collect();
        let (bpm, residual) = refit_bpm(&beats);
        assert!((bpm - 128.37).abs() < 0.001, "bpm {bpm}");
        assert!(residual < 1e-6, "residual {residual}");
    }

    #[test]
    fn jitter_shows_up_in_the_residual_and_not_in_the_tempo() {
        let mut beats: Vec<f64> = (0..400).map(|i| i as f64 * 60_000.0 / 128.0).collect();
        // +-10 ms of alternating jitter, which is the figure the two-decimal
        // claim in ENGINE.md is quoted against.
        for (i, t) in beats.iter_mut().enumerate() {
            *t += if i % 2 == 0 { 10.0 } else { -10.0 };
        }
        let (bpm, residual) = refit_bpm(&beats);
        assert!((bpm - 128.0).abs() < 0.05, "bpm {bpm}");
        assert!((residual - 10.0).abs() < 1.0, "residual {residual}");
    }

    #[test]
    fn a_tempo_change_pushes_the_residual_past_the_two_decimal_gate() {
        let a: Vec<f64> = (0..60).map(|i| i as f64 * 60_000.0 / 120.0).collect();
        let base = *a.last().unwrap();
        let b: Vec<f64> = (1..60).map(|i| base + i as f64 * 60_000.0 / 132.0).collect();
        let beats: Vec<f64> = a.into_iter().chain(b).collect();
        let (_, residual) = refit_bpm(&beats);
        assert!(residual > 15.0, "residual {residual} would allow two decimals");
    }

    #[test]
    fn a_degenerate_grid_does_not_divide_by_zero() {
        assert_eq!(refit_bpm(&[]).0, 0.0);
        assert_eq!(refit_bpm(&[100.0]).0, 0.0);
        assert_eq!(refit_bpm(&[100.0, 100.0]).0, 0.0);
        assert_eq!(coverage(&[], 1000.0), 0.0);
        assert_eq!(coverage(&[0.0, 10.0, 20.0], 0.0), 0.0);
    }

    #[test]
    fn coverage_is_one_on_a_steady_grid_and_halves_on_a_change() {
        let steady: Vec<f64> = (0..100).map(|i| i as f64 * 500.0).collect();
        assert!(coverage(&steady, 50_000.0) > 0.98);

        let mut mixed: Vec<f64> = (0..50).map(|i| i as f64 * 500.0).collect();
        let base = *mixed.last().unwrap();
        mixed.extend((1..50).map(|i| base + i as f64 * 400.0));
        let c = coverage(&mixed, *mixed.last().unwrap());
        assert!(c > 0.4 && c < 0.75, "coverage {c}");
    }

    #[test]
    fn the_tracker_recovers_a_click_grid_to_within_a_frame() {
        let f = fixtures::click_track(128.0, 16.0, fixtures::Groove::FourToTheFloor);
        let c = onset::novelty(&f.samples, fixtures::SAMPLE_RATE).unwrap();
        let period = 60.0 * c.fps as f64 / 128.0;
        let beats = track_beats(&c.full, c.fps as f64, period);
        let (bpm, residual) = refit_bpm(&beats);
        assert!((bpm - 128.0).abs() < 0.5, "bpm {bpm}");
        assert!(residual < 15.0, "residual {residual}");
        assert!(beats.len() > 28, "{} beats", beats.len());
    }
}
