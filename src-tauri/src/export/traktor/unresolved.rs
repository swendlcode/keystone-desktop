//! The three values public sources do not settle, isolated so that correcting
//! them is a one-line change rather than an audit.
//!
//! Each one is wrong in a way no unit test can catch, because the file we write
//! is self-consistent either way: only a real Traktor install can say. Half a
//! day with a dozen known-key tracks settles all three. Until then these are
//! documented assumptions, not knowledge, and the UI calls `MUSICAL_KEY` an
//! advanced option for exactly this reason.

use crate::models::KeyMode;

/// **UNVERIFIED — `CUE_V2/@START` units.**
///
/// A real fixture shows `START="52.315876"` on a 192.078-second track, which
/// reads as 52.3 **milliseconds**; that is the reading encoded here. A
/// seconds/ms mixup puts every cue within the first second of the track — the
/// exact symptom of a known Engine DJ bug with the same cause, and the thing to
/// look for first if a test export lands every cue at the head.
///
/// To correct: place a cue at a known time in Traktor, save, and read `START`
/// back. If it is seconds, set this to 1.0.
pub const START_UNITS_PER_SECOND: f64 = 1000.0;

/// **UNVERIFIED — `MUSICAL_KEY/@VALUE` numbering.**
///
/// Mixxx casts the value straight into its `ChromaticKey` enum
/// (`1 = C major .. 12 = B major, 13 = C minor .. 24 = B minor`) with the
/// comment "Traktor happens to use the same key numbering"; community guides
/// say `0-23`. They cannot both be right, and the available fixture cannot
/// settle it because its `INFO/@KEY` is the independent Key Text field read from
/// the file tag.
///
/// Getting it wrong transposes the user's whole library silently. To correct:
/// analyse a dozen tracks of known key in Traktor and read the numbers back; if
/// C major reads 0, set this to 0.
pub const MUSICAL_KEY_FIRST_VALUE: u8 = 1;

/// **UNVERIFIED — the `HOTCUE` slot offset.**
///
/// Traktor displays hot cues 1-8 while the attribute may be 0-based, and real
/// files use `HOTCUE="0"` on the `AutoGrid` marker, which is not a pad at all —
/// so the grid either shares slot 0 with a hot cue or the pads start at 1. The
/// 0-based reading is encoded here because it maps our eight slots straight
/// onto A-H.
///
/// To correct: set a cue on pad 1 in Traktor and read `HOTCUE` back; if it is 1,
/// set this to 1.
pub const HOTCUE_SLOT_OFFSET: i32 = 0;

/// The grid marker's own `HOTCUE`, as real collections carry it. Kept separate
/// from the pad numbering so that correcting the offset above cannot silently
/// move the grid onto a pad.
pub const AUTOGRID_HOTCUE: i32 = 0;

/// Position in whatever unit `START` turns out to be.
pub fn start_attr(position_ms: f64) -> f64 {
    position_ms * (START_UNITS_PER_SECOND / 1000.0)
}

/// `MUSICAL_KEY/@VALUE`: chromatic from C, the twelve majors then the twelve
/// minors, offset by whichever base turns out to be right.
pub fn musical_key_value(root: u8, mode: KeyMode) -> u8 {
    let minor_offset = match mode {
        KeyMode::Major => 0,
        KeyMode::Minor => 12,
    };
    MUSICAL_KEY_FIRST_VALUE + (root % 12) + minor_offset
}

/// The `HOTCUE` slot for our 0-7 cue slot.
pub fn hotcue_attr(slot: u8) -> i32 {
    slot.min(7) as i32 + HOTCUE_SLOT_OFFSET
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_numbering_covers_all_24_values_without_collision() {
        let mut seen = std::collections::HashSet::new();
        for root in 0u8..12 {
            for mode in [KeyMode::Major, KeyMode::Minor] {
                assert!(
                    seen.insert(musical_key_value(root, mode)),
                    "two keys share one MUSICAL_KEY value"
                );
            }
        }
        assert_eq!(seen.len(), 24);
        let lowest = *seen.iter().min().unwrap();
        let highest = *seen.iter().max().unwrap();
        assert_eq!(lowest, MUSICAL_KEY_FIRST_VALUE);
        assert_eq!(highest, MUSICAL_KEY_FIRST_VALUE + 23);
    }

    #[test]
    fn switching_the_base_moves_every_value_together() {
        // The whole point of the single constant: C major and its relative
        // minor keep their 12-apart relationship whichever base is right.
        use crate::models::KeyMode::{Major, Minor};
        assert_eq!(
            musical_key_value(0, Minor) - musical_key_value(0, Major),
            12
        );
    }

    #[test]
    fn the_eight_slots_map_to_eight_distinct_hotcues() {
        let mapped: Vec<i32> = (0u8..8).map(hotcue_attr).collect();
        let unique: std::collections::HashSet<i32> = mapped.iter().copied().collect();
        assert_eq!(unique.len(), 8);
        // A slot past the eighth pad is clamped rather than inventing a ninth.
        assert_eq!(hotcue_attr(9), hotcue_attr(7));
    }

    #[test]
    fn start_is_a_single_conversion_from_milliseconds() {
        assert_eq!(start_attr(0.0), 0.0);
        assert_eq!(start_attr(52.315876), 52.315876 * (START_UNITS_PER_SECOND / 1000.0));
    }
}
