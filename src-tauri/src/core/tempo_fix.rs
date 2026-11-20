//! Tempi that are out by a ratio, and which ratio it is.
//!
//! Folding — `analysis::rhythm::tempo::fold_into_range` — only ever halves or
//! doubles, and that is correct: an octave is the one transform that cannot
//! change which beats are beats. It is also why four house tracks in this
//! library read 162.6, 164.8, 167.8 and 170.7 and were never corrected. They
//! sit *inside* the default 79–192 range, so folding had nothing to do, and
//! halving them would give 81–85, which is not the answer either. The answer
//! is three quarters: 122, 124, 126, 128.
//!
//! That happens when the detector locks onto a dotted or triplet reading of
//! the bar — three beats counted where there are four, or the other way
//! round. It is a different mistake from an octave and needs a different fix,
//! so it lives here rather than in the detector:
//!
//! * The detector must keep reporting what it measured. `tempo_raw` is the
//!   evidence, and a 170 BPM drum and bass track really is 170.
//! * A correction is the **user's** claim about the track, written with
//!   `ValueSource::User` so the next analysis pass does not undo it.
//!
//! So this module only ever *suggests*. Nothing here writes.

/// The ratios tried, simplest first.
///
/// Order is the whole decision procedure: when more than one lands inside the
/// wanted range the first is taken, because an octave is a more likely
/// mistake than a dotted reading, and a dotted reading is more likely than a
/// 3:2. Anything further out than these is not a misread bar, it is a
/// different track.
const RATIOS: &[(f64, &str)] = &[
    (0.5, "half"),
    (2.0, "double"),
    (0.75, "three quarters"),
    (4.0 / 3.0, "four thirds"),
    (2.0 / 3.0, "two thirds"),
    (1.5, "three halves"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// What the track would read at.
    pub bpm: f64,
    /// The multiplier applied, as a number and in words.
    pub ratio: f64,
    pub ratio_name: &'static str,
    /// Every other ratio that also lands in range. Non-empty means the range
    /// does not settle it on its own and somebody should listen to the track.
    pub also: Vec<f64>,
}

/// A ratio that brings `bpm` inside `[lo, hi]`, if there is one.
///
/// `None` when the tempo is already in range — the common case, and the one
/// where a suggestion would be an invitation to break something that works.
pub fn suggest(bpm: f64, lo: f64, hi: f64) -> Option<Suggestion> {
    if !bpm.is_finite() || bpm <= 0.0 || !lo.is_finite() || lo <= 0.0 || hi < lo {
        return None;
    }
    if (lo..=hi).contains(&bpm) {
        return None;
    }
    let landing: Vec<(f64, &'static str)> = RATIOS
        .iter()
        .map(|(r, name)| (bpm * r, *name))
        .filter(|(candidate, _)| (lo..=hi).contains(candidate))
        .collect();

    let (best, name) = *landing.first()?;
    Some(Suggestion {
        bpm: round2(best),
        ratio: round4(best / bpm),
        ratio_name: name,
        also: landing.iter().skip(1).map(|(b, _)| round2(*b)).collect(),
    })
}

/// Tempi are compared and reported to two decimals everywhere else in the
/// app, so a suggestion is too — `121.95000000000002` is noise in a report
/// somebody reads.
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four tracks that prompted this, against a house range.
    #[test]
    fn the_house_tracks_that_folding_could_not_reach() {
        for (detected, expected) in [
            (162.6, 121.95),
            (164.8, 123.6),
            (167.8, 125.85),
            (170.7, 128.03),
        ] {
            let s = suggest(detected, 115.0, 135.0).expect("a suggestion");
            assert_eq!(s.ratio_name, "three quarters", "{detected}");
            assert!(
                (s.bpm - expected).abs() < 0.01,
                "{detected} suggested {} not {expected}",
                s.bpm
            );
        }
    }

    /// Inside the range is not a problem to solve.
    #[test]
    fn a_tempo_already_in_range_is_left_alone() {
        assert!(suggest(128.0, 115.0, 135.0).is_none());
        assert!(suggest(115.0, 115.0, 135.0).is_none());
        assert!(suggest(135.0, 115.0, 135.0).is_none());
    }

    /// The ordinary octave error still resolves as an octave, not as
    /// something more exotic that happens to fit.
    #[test]
    fn an_octave_error_is_named_an_octave() {
        let s = suggest(64.0, 115.0, 135.0).expect("a suggestion");
        assert_eq!(s.ratio_name, "double");
        assert_eq!(s.bpm, 128.0);

        let s = suggest(256.0, 115.0, 135.0).expect("a suggestion");
        assert_eq!(s.ratio_name, "half");
        assert_eq!(s.bpm, 128.0);
    }

    /// A wide range lets several ratios land, and that ambiguity is reported
    /// rather than hidden behind the first one.
    #[test]
    fn more_than_one_landing_is_declared() {
        // 200 BPM into 79-192: half is 100 and three quarters is 150, both in
        // range. The octave wins and the other is named.
        let s = suggest(200.0, 79.0, 192.0).expect("a suggestion");
        assert_eq!(s.ratio_name, "half");
        assert_eq!(s.bpm, 100.0);
        assert!(s.also.contains(&150.0), "also: {:?}", s.also);
    }

    /// Nothing within a sensible ratio of 40 reaches a drum and bass range,
    /// and inventing one would be worse than saying nothing.
    #[test]
    fn a_tempo_no_ratio_can_reach_gets_no_suggestion() {
        assert!(suggest(40.0, 170.0, 180.0).is_none());
    }

    #[test]
    fn a_nonsense_tempo_or_range_is_refused_quietly() {
        assert!(suggest(f64::NAN, 115.0, 135.0).is_none());
        assert!(suggest(0.0, 115.0, 135.0).is_none());
        assert!(suggest(128.0, 0.0, 135.0).is_none());
        assert!(suggest(128.0, 135.0, 115.0).is_none());
    }

    /// The reported ratio has to reconstruct the suggestion from the
    /// detection, or the report is lying about what was done.
    #[test]
    fn the_ratio_reconstructs_the_suggested_tempo() {
        let s = suggest(162.6, 115.0, 135.0).expect("a suggestion");
        assert!((162.6 * s.ratio - s.bpm).abs() < 0.01);
    }
}
