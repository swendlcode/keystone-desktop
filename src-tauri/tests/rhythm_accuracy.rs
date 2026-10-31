//! S6-S8 against click tracks whose answer is arithmetic.
//!
//! A click track is used on purpose. Asserting 128.0 BPM on a real file asserts
//! one annotator's reading of that file, and when it breaks nobody can tell
//! whether the detector or the fixture is wrong. Real material belongs in the
//! tempo gate of ENGINE.md 7.4 — **GiantSteps Tempo, 664 tracks, Accuracy1 and
//! Accuracy2 reported separately because the gap between them *is* the octave
//! problem** — and that run needs S1 decode, which is still a stub. None of the
//! numbers here are a substitute for it.

#![cfg(feature = "eval")]

use keystone_lib::analysis::rhythm::fixtures::{self, Groove};
use keystone_lib::analysis::rhythm::{self, tempo};

const SR: u32 = fixtures::SAMPLE_RATE;
const SECONDS: f64 = 30.0;

/// The tempogram's own resolution is 0.1 BPM on its search grid, and it cannot
/// do better than roughly 1.3-5.0 BPM from integer lag quantisation at 100 fps.
/// Landing inside this tolerance therefore proves the **least-squares refit over
/// the whole grid** ran and was trusted, not just that the tempogram peaked in
/// the right place.
const EXACT_TOLERANCE_BPM: f64 = 0.25;

fn run(f: &fixtures::Fixture) -> (tempo::TempoEstimate, tempo::BeatGrid) {
    let curves = rhythm::onset::novelty(&f.samples, SR).expect("novelty");
    tempo::estimate(&curves).expect("tempo")
}

#[test]
fn click_tracks_at_known_tempos_land_exact() {
    // 70 is below the display range and 174 is drum and bass at the top of it:
    // both are the cases where an octave rule that folds *before* choosing
    // produces a confident wrong answer that no later stage can detect.
    //
    // 70 is a kick-only click. The four-to-the-floor fixture at 70 puts a hat
    // exactly between every pair of kicks, which makes the material genuinely
    // periodic at 140 as well — a tempo the engine is entitled to prefer and the
    // next test measures rather than asserts.
    for (bpm, groove) in [
        (70.0f64, Groove::KickOnly),
        (90.0, Groove::FourToTheFloor),
        (100.0, Groove::FourToTheFloor),
        (128.0, Groove::FourToTheFloor),
        (140.0, Groove::FourToTheFloor),
        (174.0, Groove::FourToTheFloor),
    ] {
        let f = fixtures::click_track(bpm, SECONDS, groove);
        let (t, grid) = run(&f);
        println!(
            "{bpm:6.1} BPM {groove:>15?} -> raw {:8.3}  display {:7.2}  shift {:+}  \
             residual {:5.2} ms  coverage {:.2}  confidence {:.2}",
            t.raw_bpm,
            tempo::display_bpm(t.raw_bpm, tempo::DEFAULT_RANGE.0, tempo::DEFAULT_RANGE.1),
            t.octave_shift,
            t.residual_ms,
            grid.coverage,
            t.confidence
        );
        assert!(
            (t.raw_bpm - bpm).abs() < EXACT_TOLERANCE_BPM,
            "{bpm} BPM came back as {:.3}",
            t.raw_bpm
        );
        assert!(!t.variable, "{bpm} BPM steady click flagged variable");
        assert!(
            t.residual_ms < tempo::TWO_DECIMAL_RESIDUAL_MS,
            "{bpm} BPM: grid residual {:.2} ms would suppress the two decimals a \
             constant-tempo track is entitled to",
            t.residual_ms
        );
    }
}

#[test]
fn an_off_beat_hat_between_every_kick_is_an_octave_the_engine_may_prefer() {
    // Four-to-the-floor at 70 with a hat on every off-beat eighth is periodic at
    // 70 in each band and at 140 across both, and 140 is where the perceptual
    // prior sits. Asserting 70 here would be asserting one of two defensible
    // readings, which is exactly the Accuracy1-versus-Accuracy2 distinction
    // ENGINE.md 7.4 requires be reported *separately* on GiantSteps Tempo rather
    // than collapsed into one figure.
    //
    // What is required is that the answer is an octave of the truth and that the
    // user sees the same number either way, because the display folds.
    let f = fixtures::click_track(70.0, SECONDS, Groove::FourToTheFloor);
    let (t, _) = run(&f);
    let octave = (t.raw_bpm / 70.0).log2();
    println!(
        "70 BPM four-to-the-floor -> raw {:.3} ({:+.0} octave), display {:.2}, \
         confidence {:.2}",
        t.raw_bpm,
        octave,
        tempo::display_bpm(t.raw_bpm, tempo::DEFAULT_RANGE.0, tempo::DEFAULT_RANGE.1),
        t.confidence
    );
    assert!(
        (octave - octave.round()).abs() < 0.02,
        "raw {:.3} is not an octave of 70",
        t.raw_bpm
    );
    assert!(
        octave.round().abs() <= 1.0,
        "raw {:.3} is {:+.0} octaves from 70, which is not an ambiguity",
        t.raw_bpm,
        octave
    );
    // Whichever octave it chose, folding puts the displayed tempo on 140.
    let shown = tempo::display_bpm(t.raw_bpm, tempo::DEFAULT_RANGE.0, tempo::DEFAULT_RANGE.1);
    assert!((shown - 140.0).abs() < 1.0, "displayed {shown:.2}");
}

#[test]
fn the_octave_rule_folds_last_and_the_result_lands_inside_the_range() {
    let (lo, hi) = tempo::DEFAULT_RANGE;
    // 70 BPM is outside the default 79-192 range, so the display has to be
    // doubled while the *raw* value stays 70. Storing the folded number is what
    // makes a correct 170 BPM track and a wrong 85 BPM estimate
    // indistinguishable in a report.
    let f = fixtures::click_track(70.0, SECONDS, Groove::KickOnly);
    let (t, _) = run(&f);
    assert!((t.raw_bpm - 70.0).abs() < EXACT_TOLERANCE_BPM, "raw {:.3}", t.raw_bpm);
    let display = tempo::display_bpm(t.raw_bpm, lo, hi);
    assert!((display - 140.0).abs() < 2.0 * EXACT_TOLERANCE_BPM, "display {display:.3}");
    assert_eq!(t.octave_shift, -1, "a doubling has to be recorded as a shift");

    // Every tempo the engine can report folds into the user's range, and the
    // fold is pure arithmetic over the stored raw value: changing the Tempo
    // setting re-folds from the database and never re-reads a file.
    for raw in [40.0f64, 70.0, 78.9, 79.0, 128.0, 192.0, 192.1, 256.0, 400.0] {
        let (folded, shift) = tempo::fold_into_range(raw, lo, hi);
        assert!((lo..=hi).contains(&folded), "{raw} folded to {folded}, outside {lo}-{hi}");
        assert!((folded / raw - 2f64.powi(-shift)).abs() < 1e-9, "{raw}: shift {shift}");
        assert!((tempo::display_bpm(raw, lo, hi) - folded).abs() < 1e-9);
    }

    // A narrower user range re-folds the same stored value without re-analysis.
    assert_eq!(tempo::fold_into_range(174.0, 79.0, 160.0), (87.0, 1));
    assert_eq!(tempo::fold_into_range(174.0, 79.0, 192.0), (174.0, 0));
}

#[test]
fn a_variable_tempo_file_reports_a_residual_too_high_for_two_decimals() {
    // Two 20 s halves at 124 and 138 BPM. A single least-squares fit over the
    // whole grid cannot describe both, so the residual goes far above the 15 ms
    // gate and the track owes one decimal and a variable-tempo flag. Without
    // this gate the spec's two-decimal option is false precision on exactly the
    // material that needs the honesty most.
    let f = fixtures::tempo_change(124.0, 138.0, 20.0);
    let (t, grid) = run(&f);
    println!(
        "124 -> 138 BPM: raw {:.3}  residual {:.2} ms  variable {}  coverage {:.2}  \
         confidence {:.2}",
        t.raw_bpm, t.residual_ms, t.variable, grid.coverage, t.confidence
    );
    assert!(
        !(t.residual_ms < tempo::TWO_DECIMAL_RESIDUAL_MS),
        "residual {:.2} ms is under the {:.0} ms gate on a file that changes tempo",
        t.residual_ms,
        tempo::TWO_DECIMAL_RESIDUAL_MS
    );
    assert!(t.variable, "a tempo change has to set the variable flag");
    assert!(
        t.confidence < 0.8,
        "a tempo change read at confidence {:.2} — ENGINE.md 8 says such material \
         gets a low-confidence result, not a confident one",
        t.confidence
    );

    // The formatter is where the gate becomes visible to the user, and it
    // overrides the setting rather than trusting it.
    let two_decimals = tempo::format_bpm(t.raw_bpm, Some(t.residual_ms), 2);
    assert_eq!(
        two_decimals.split('.').nth(1).map(str::len),
        Some(1),
        "variable-tempo BPM formatted as {two_decimals} with the two-decimal setting on"
    );

    // And a steady click of the same length does get its two decimals, or the
    // gate is simply refusing everything.
    let steady = fixtures::click_track(128.0, 40.0, Groove::FourToTheFloor);
    let (s, _) = run(&steady);
    let steady_text = tempo::format_bpm(s.raw_bpm, Some(s.residual_ms), 2);
    println!("steady 128: {steady_text} (residual {:.2} ms)", s.residual_ms);
    assert_eq!(steady_text.split('.').nth(1).map(str::len), Some(2), "{steady_text}");
}

#[test]
fn the_octave_traps_are_either_resolved_or_reported_as_contested() {
    // Half-time is genuinely ambiguous rather than wrong, and ENGINE.md 7.2
    // says such material owes a low confidence rather than a confident answer.
    // The accent-every-two fixture is the opposite: autocorrelation prefers the
    // two-beat period and only the kick-band evidence can see that every beat
    // carries a kick, so it must come back at the faster tempo.
    let accent = fixtures::click_track(128.0, SECONDS, Groove::AccentEveryTwo);
    let (t, _) = run(&accent);
    println!("accent-every-two 128: raw {:.3} confidence {:.2}", t.raw_bpm, t.confidence);
    assert!(
        (t.raw_bpm - 128.0).abs() < 1.0,
        "the kick-band evidence did not settle the octave: {:.3}",
        t.raw_bpm
    );

    let half = fixtures::click_track(140.0, SECONDS, Groove::HalfTime);
    let (h, _) = run(&half);
    println!("half-time 140: raw {:.3} confidence {:.2}", h.raw_bpm, h.confidence);
    let plausible = [70.0f64, 140.0, 280.0].iter().any(|c| (h.raw_bpm - c).abs() < 1.5);
    assert!(plausible, "half-time came back at {:.3}, which is not an octave of 140", h.raw_bpm);
    assert!(
        h.confidence <= 0.9,
        "half-time read at confidence {:.2}; the octave is genuinely ambiguous here",
        h.confidence
    );
}
