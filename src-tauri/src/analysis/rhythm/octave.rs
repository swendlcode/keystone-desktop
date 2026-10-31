//! S7, part two: the octave problem.
//!
//! 64, 128 and 192 BPM all have the same autocorrelation peaks, because a
//! 128 BPM kick drum is also periodic at two beats and at two thirds of a bar.
//! Four steps settle it, **in this order**, and the third is the one that
//! actually decides: a comb with 1/k weighting, a capped perceptual prior,
//! beat-level kick and hat evidence, then folding — last, and never as a way of
//! choosing.

use crate::analysis::rhythm::onset::{self, NoveltyCurves};
use crate::analysis::rhythm::tempogram::{Tempogram, BPM_MAX, BPM_MIN};
use crate::error::{KeystoneError, Result};

/// Log-Gaussian perceptual prior. Capped: it must never overturn a candidate
/// with a 2:1 or better salience margin, or a genuine 174 BPM drum and bass
/// track gets pulled to 87.
pub const PRIOR_CENTRE_BPM: f64 = 120.0;
pub const PRIOR_SIGMA_OCTAVES: f64 = 0.35;
pub const PRIOR_SALIENCE_CAP: f64 = 2.0;

/// The halfway points must carry at least this share of the on-beat kick
/// energy before the faster candidate wins.
///
/// Set well below 1.0 on purpose. A real kick on an off-beat is quieter than
/// the one on the beat — different sample, less headroom, often side-chained —
/// so requiring parity would reject genuine four-to-the-floor at double time.
pub const SUBDIVIDE_KICK_RATIO: f64 = 0.55;

/// And they must carry at least this share of the on-beat **hat** energy, which
/// is the same threshold asking a different question: are the halfway points the
/// same *kind* of event as the beats they sit between?
///
/// Without this, doubling fires on an off-beat hat. A hat is a sharp broadband
/// transient and a kick is a slow one, so in the 30-250 Hz band a hat's flux can
/// exceed the kick's own — measured at 4.5x on the hats of a 90 BPM
/// four-to-the-floor click, which read as 180 BPM for that reason alone. The
/// pair of ratios is what separates the two cases: real double-time kicks look
/// like the beats in both bands, while a hat is loud in one and quiet in the
/// other. Timbral disagreement between the two classes is evidence *for* the
/// slower reading, not against it.
pub const SUBDIVIDE_HAT_RATIO: f64 = 0.55;

/// Beats are sampled with this much slack, in frames, because the novelty peak
/// of a kick sits a frame or two after the nominal beat.
const BEAT_TOLERANCE_FRAMES: usize = 3;

/// Candidate phases tried when aligning a trial grid to the curve.
const PHASE_STEPS: usize = 24;

#[derive(Debug, Clone, Copy)]
pub struct OctaveDecision {
    pub bpm: f64,
    /// Best salience over the strongest non-octave peak. High means the
    /// tempogram found one tempo, not a contest.
    pub salience_ratio: f64,
    /// 0..1: how decisively step 3 separated the two candidates.
    pub evidence: f64,
    /// The hats say one tempo and the kicks say another. Half-time trap, and
    /// the honest answer is a low confidence rather than a coin toss.
    pub contested: bool,
    /// Kick strength on the halfway points over kick strength on the candidate
    /// beats, and the same in the hat band. Kept on the decision because they
    /// are the whole argument for the octave it chose.
    pub kick_ratio: f64,
    pub hat_ratio: f64,
}

/// `exp(-log2(bpm/120)^2 / (2 sigma^2))`.
pub fn perceptual_prior(bpm: f64) -> f64 {
    if bpm <= 0.0 {
        return 0.0;
    }
    let octaves = (bpm / PRIOR_CENTRE_BPM).log2();
    (-(octaves * octaves) / (2.0 * PRIOR_SIGMA_OCTAVES * PRIOR_SIGMA_OCTAVES)).exp()
}

/// Steps 1 and 2: the comb's winner, then the prior, which may move it between
/// **octaves of that winner and nowhere else**.
///
/// The restriction is not a refinement, it is the difference between working
/// and not. Under 1/k weighting a 3:2 tempo relation can never reach a 2:1
/// salience margin -- a 174 BPM track scores 1.833 at 174 and 1.500 at 116,
/// because 116's lag collects two harmonics inside the 120-frame ceiling and
/// 174's collects three. An unrestricted prior therefore relabels every drum
/// and bass record as 116 BPM, and the cap cannot save it. Step 2 is the octave
/// step; choosing between unrelated tempi is what the comb's salience is for.
fn best_after_prior(t: &Tempogram) -> Result<(f64, f64)> {
    let (raw_bpm, raw_salience) = t
        .best()
        .ok_or_else(|| KeystoneError::Analysis("empty tempogram".into()))?;

    let mut prior_best = (raw_bpm, raw_salience, raw_salience * perceptual_prior(raw_bpm));
    for shift in [-2i32, -1, 1, 2] {
        let bpm = raw_bpm * 2f64.powi(shift);
        if !(BPM_MIN..=BPM_MAX).contains(&bpm) {
            continue;
        }
        let Some(salience) = t.salience_at(bpm) else { continue };
        let weighted = salience * perceptual_prior(bpm);
        if weighted > prior_best.2 {
            prior_best = (bpm, salience, weighted);
        }
    }

    // The cap. A 2:1 salience margin is the comb speaking clearly, and a
    // preference for 120 BPM is not evidence about this track.
    if raw_salience >= PRIOR_SALIENCE_CAP * prior_best.1 {
        Ok((raw_bpm, raw_salience))
    } else {
        Ok((prior_best.0, prior_best.1))
    }
}

/// Mean curve strength on the even and odd members of a grid at `period / 2`,
/// at the phase that best fits the curve.
///
/// Even members are the candidate beats at `period`; odd members are the
/// halfway points between them. Equal strength means the real beat is at
/// `period / 2`.
fn half_grid_support(curve: &[f32], period: f64, span: usize) -> (f64, f64) {
    let half = period * 0.5;
    if curve.is_empty() || half < 1.0 {
        return (0.0, 0.0);
    }
    let mut best = (f64::NEG_INFINITY, 0.0, 0.0);
    for p in 0..PHASE_STEPS {
        let phase = half * p as f64 / PHASE_STEPS as f64;
        let (mut even, mut odd, mut n_even, mut n_odd) = (0.0f64, 0.0f64, 0usize, 0usize);
        let mut i = 0usize;
        loop {
            let frame = phase + half * i as f64;
            if frame >= span as f64 {
                break;
            }
            let v = onset::peak_near(curve, frame, BEAT_TOLERANCE_FRAMES) as f64;
            if i % 2 == 0 {
                even += v;
                n_even += 1;
            } else {
                odd += v;
                n_odd += 1;
            }
            i += 1;
        }
        if n_even == 0 || n_odd == 0 {
            continue;
        }
        let (even, odd) = (even / n_even as f64, odd / n_odd as f64);
        // The phase that maximises the strong class is the aligned one; scoring
        // on the total would let a half-period shift win by symmetry.
        if even > best.0 {
            best = (even, even, odd);
        }
    }
    if best.0.is_finite() {
        (best.1, best.2)
    } else {
        (0.0, 0.0)
    }
}

fn ratio(even: f64, odd: f64) -> f64 {
    if even <= 1e-9 {
        0.0
    } else {
        odd / even
    }
}

/// Step 3, then step 4's input. Returns the chosen unfolded tempo.
pub fn resolve(t: &Tempogram, curves: &NoveltyCurves) -> Result<OctaveDecision> {
    let (bpm, salience) = best_after_prior(t)?;
    let span = curves.len();
    let fps = curves.fps as f64;
    let period = 60.0 * fps / bpm;

    let (kick_even, kick_odd) = half_grid_support(&curves.kick, period, span);
    let (hat_even, hat_odd) = half_grid_support(&curves.hat, period, span);
    let kick_r = ratio(kick_even, kick_odd);
    let hat_r = ratio(hat_even, hat_odd);

    // Step 3 chooses between the winning lag and half of it, and nothing else.
    // It does not need a halving branch: at lag L/2 the comb can only collect
    // the even harmonics when the halfway points are empty, which is exactly
    // half the score the slower candidate collects, so the comb has already
    // rejected a double-time reading before this runs.
    // Both bands, not just the kick band: see SUBDIVIDE_HAT_RATIO.
    let wants_faster = kick_r >= SUBDIVIDE_KICK_RATIO && hat_r >= SUBDIVIDE_HAT_RATIO;
    let room = bpm * 2.0 <= BPM_MAX;
    let chosen = if wants_faster && room { bpm * 2.0 } else { bpm };

    // Hats on the halfway points where the kicks are not is the half-time
    // signature: the pulse a listener follows is twice the kick rate, both
    // readings are defensible, and so is refusing to pick. The blocked-by-range
    // case counts too — the kicks asked for a tempo the range cannot hold, and
    // that is a doubt, not a decision.
    let contested = (hat_r >= SUBDIVIDE_HAT_RATIO && kick_r < SUBDIVIDE_KICK_RATIO)
        || (wants_faster && !room);

    let margin = (kick_r - SUBDIVIDE_KICK_RATIO).abs() / SUBDIVIDE_KICK_RATIO;
    let evidence = margin.min(1.0) * if contested { 0.3 } else { 1.0 };

    let second = t.second_best_non_octave(chosen).map(|(_, s)| s).unwrap_or(0.0);
    let salience_ratio = if salience <= 1e-9 {
        0.0
    } else {
        (1.0 - second / salience).clamp(0.0, 1.0)
    };

    Ok(OctaveDecision {
        bpm: chosen,
        salience_ratio,
        evidence,
        contested,
        kick_ratio: kick_r,
        hat_ratio: hat_r,
    })
}

/// The scaffold's shape, kept so `analysis::tempo::resolve_octave` stays a
/// documented entry point: the chosen tempo and a 0..1 confidence in the octave
/// itself.
pub fn resolve_octave(t: &Tempogram, curves: &NoveltyCurves) -> Result<(f64, f64)> {
    let d = resolve(t, curves)?;
    Ok((d.bpm, (d.salience_ratio * 0.5 + d.evidence * 0.5).clamp(0.0, 1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rhythm::{fixtures, onset, tempogram};

    fn decide(bpm: f64, groove: fixtures::Groove) -> OctaveDecision {
        let f = fixtures::click_track(bpm, 24.0, groove);
        let c = onset::novelty(&f.samples, fixtures::SAMPLE_RATE).unwrap();
        let t = tempogram::tempogram(&c).unwrap();
        resolve(&t, &c).unwrap()
    }

    #[test]
    fn the_prior_is_centred_on_120_and_symmetric_in_octaves() {
        assert!((perceptual_prior(120.0) - 1.0).abs() < 1e-12);
        assert!((perceptual_prior(60.0) - perceptual_prior(240.0)).abs() < 1e-12);
        assert!(perceptual_prior(120.0) > perceptual_prior(174.0));
        assert_eq!(perceptual_prior(0.0), 0.0);
    }

    #[test]
    fn an_accent_every_two_beats_does_not_halve_the_tempo() {
        // The octave trap: the ACF prefers the two-beat period because the
        // accent is louder, and only the kick-band evidence can see that every
        // beat carries a kick. 128 must not come back as 64.
        let d = decide(128.0, fixtures::Groove::AccentEveryTwo);
        assert!((d.bpm - 128.0).abs() < 2.0, "resolved to {}", d.bpm);
    }

    #[test]
    fn four_to_the_floor_at_128_stays_at_128() {
        let d = decide(128.0, fixtures::Groove::FourToTheFloor);
        assert!((d.bpm - 128.0).abs() < 2.0, "resolved to {}", d.bpm);
        assert!(!d.contested);
    }

    #[test]
    fn drum_and_bass_at_174_is_not_pulled_towards_120() {
        // The capped prior exists for this track. 174 is 0.54 octaves above the
        // prior's centre, so an uncapped prior would prefer 87.
        let d = decide(174.0, fixtures::Groove::FourToTheFloor);
        assert!((d.bpm - 174.0).abs() < 3.0, "resolved to {}", d.bpm);
    }

    #[test]
    fn half_time_is_reported_as_contested_rather_than_guessed() {
        // Kick on 1 and 3 at 140 with hats on every eighth: the kick rate is
        // 70 and the hat rate is 280. Either reading is defensible, so the
        // engine owes low evidence, not a confident number.
        let d = decide(140.0, fixtures::Groove::HalfTime);
        assert!(d.contested, "half-time read as settled at {}", d.bpm);
        assert!(d.evidence < 0.5, "evidence {}", d.evidence);
    }
}
