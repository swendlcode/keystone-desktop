//! S5 — key to Camelot, and the compatibility set the wheel filter depends on.
//!
//! Verified against Mixed In Key's own wheel: 8B = C major, 8A = A minor, +1 is
//! up a perfect fifth. The documented advanced moves (+2 energy boost, -5 jaws,
//! -3 pay attention) stay out of the default set — they are deliberate
//! transitions, not compatibility.

use serde::{Deserialize, Serialize};

use crate::models::{KeyMode, KeyNotation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CamelotLetter {
    /// Minor. Amodal results are reported here too.
    A,
    /// Major.
    B,
}

impl CamelotLetter {
    pub fn other(self) -> Self {
        match self {
            CamelotLetter::A => CamelotLetter::B,
            CamelotLetter::B => CamelotLetter::A,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CamelotLetter::A => "A",
            CamelotLetter::B => "B",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Camelot {
    /// 1-12.
    pub number: u8,
    pub letter: CamelotLetter,
}

/// 0 = C. Sharps.
const SHARP_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
/// 0 = C. Flats, with the typographic flat sign the UI shows.
const FLAT_NAMES: [&str; 12] = [
    "C", "D♭", "D", "E♭", "E", "F", "G♭", "G", "A♭", "A", "B♭", "B",
];
/// ID3v2's TKEY accepts only A-G, `b`, `#`, `m` and `o`, so a tag value cannot
/// carry the typographic flat.
const FLAT_NAMES_ASCII: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

impl Camelot {
    pub fn from_key(root: u8, mode: KeyMode) -> Self {
        let p = (root % 12) as i32;
        let (offset, letter) = match mode {
            KeyMode::Major => (7, CamelotLetter::B),
            KeyMode::Minor => (4, CamelotLetter::A),
        };
        let number = (((7 * p + offset).rem_euclid(12)) + 1) as u8;
        Self { number, letter }
    }

    /// The inverse. 7 is its own multiplicative inverse mod 12 (7 * 7 = 49 = 1),
    /// which is why both branches multiply by 7 again.
    pub fn to_key(self) -> (u8, KeyMode) {
        let n = self.number as i32;
        match self.letter {
            CamelotLetter::B => ((7 * (n - 8)).rem_euclid(12) as u8, KeyMode::Major),
            CamelotLetter::A => ((7 * (n - 5)).rem_euclid(12) as u8, KeyMode::Minor),
        }
    }

    /// `8A`, or `08A` when the user turned on zero padding.
    pub fn label(self, pad_single_digits: bool) -> String {
        if pad_single_digits && self.number < 10 {
            format!("0{}{}", self.number, self.letter.as_str())
        } else {
            format!("{}{}", self.number, self.letter.as_str())
        }
    }

    /// Accepts `8A`, `08a`, `12B`. Returns `None` for anything off the wheel,
    /// so a stored value that drifted cannot silently become segment 1.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let (digits, letter) = s.split_at(s.len().checked_sub(1)?);
        let letter = match letter {
            "A" | "a" => CamelotLetter::A,
            "B" | "b" => CamelotLetter::B,
            _ => return None,
        };
        let number: u8 = digits.parse().ok()?;
        if !(1..=12).contains(&number) {
            return None;
        }
        Some(Self { number, letter })
    }

    /// The whole wheel filter: this segment, its relative across the ring, and
    /// ±1 around the ring with 12 wrapping to 1.
    pub fn compatible(self) -> [Camelot; 4] {
        let up = if self.number == 12 { 1 } else { self.number + 1 };
        let down = if self.number == 1 { 12 } else { self.number - 1 };
        [
            self,
            Camelot { number: self.number, letter: self.letter.other() },
            Camelot { number: down, letter: self.letter },
            Camelot { number: up, letter: self.letter },
        ]
    }
}

/// `F#m`, `G♭m`, `C`, or `8A`, depending on the Key Notation setting.
pub fn key_string(root: u8, mode: KeyMode, notation: KeyNotation, pad: bool) -> String {
    match notation {
        KeyNotation::Camelot => Camelot::from_key(root, mode).label(pad),
        KeyNotation::Sharps => decorate(SHARP_NAMES[(root % 12) as usize], mode),
        KeyNotation::Flats => decorate(FLAT_NAMES[(root % 12) as usize], mode),
    }
}

/// The ASCII form a tag may carry. `TKEY` is restricted to three characters
/// from A-G, `b`, `#`, `m`, plus `o` for off-key, so the typographic flat and
/// anything longer than `F#m` must not reach it.
pub fn key_tag_string(root: u8, mode: KeyMode, notation: KeyNotation) -> String {
    let name = match notation {
        KeyNotation::Flats => FLAT_NAMES_ASCII[(root % 12) as usize],
        // Camelot is not legal in TKEY; callers who opted into writing it there
        // go through Camelot::label directly and accept the non-standard value.
        _ => SHARP_NAMES[(root % 12) as usize],
    };
    decorate(name, mode)
}

fn decorate(name: &str, mode: KeyMode) -> String {
    match mode {
        KeyMode::Major => name.to_string(),
        KeyMode::Minor => format!("{name}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_from_the_published_wheel() {
        // 8B = C major, 8A = A minor, +1 = up a perfect fifth.
        assert_eq!(Camelot::from_key(0, KeyMode::Major).label(false), "8B");
        assert_eq!(Camelot::from_key(7, KeyMode::Major).label(false), "9B");
        assert_eq!(Camelot::from_key(2, KeyMode::Major).label(false), "10B");
        assert_eq!(Camelot::from_key(9, KeyMode::Minor).label(false), "8A");
        assert_eq!(Camelot::from_key(4, KeyMode::Minor).label(false), "9A");
        assert_eq!(Camelot::from_key(11, KeyMode::Minor).label(false), "10A");
    }

    #[test]
    fn every_key_round_trips_and_the_wheel_is_a_bijection() {
        let mut seen = std::collections::HashSet::new();
        for root in 0u8..12 {
            for mode in [KeyMode::Major, KeyMode::Minor] {
                let c = Camelot::from_key(root, mode);
                assert!((1..=12).contains(&c.number));
                assert_eq!(c.to_key(), (root, mode));
                assert_eq!(Camelot::parse(&c.label(false)), Some(c));
                assert_eq!(Camelot::parse(&c.label(true)), Some(c));
                assert!(seen.insert(c.label(false)), "two keys landed on {c:?}");
            }
        }
        assert_eq!(seen.len(), 24);
    }

    #[test]
    fn compatibility_is_relative_plus_one_either_way_and_wraps() {
        let set: Vec<String> = Camelot::parse("8A")
            .unwrap()
            .compatible()
            .iter()
            .map(|c| c.label(false))
            .collect();
        assert_eq!(set, ["8A", "8B", "7A", "9A"]);

        let wrapped: Vec<String> = Camelot::parse("12B")
            .unwrap()
            .compatible()
            .iter()
            .map(|c| c.label(false))
            .collect();
        assert_eq!(wrapped, ["12B", "12A", "11B", "1B"]);

        let wrapped_down: Vec<String> = Camelot::parse("1A")
            .unwrap()
            .compatible()
            .iter()
            .map(|c| c.label(false))
            .collect();
        assert_eq!(wrapped_down, ["1A", "1B", "12A", "2A"]);
    }

    #[test]
    fn tag_values_stay_inside_what_tkey_allows() {
        for root in 0u8..12 {
            for mode in [KeyMode::Major, KeyMode::Minor] {
                for notation in [KeyNotation::Sharps, KeyNotation::Flats] {
                    let v = key_tag_string(root, mode, notation);
                    assert!(v.len() <= 3, "{v} is longer than TKEY allows");
                    assert!(
                        v.chars().all(|c| matches!(c, 'A'..='G' | 'b' | '#' | 'm')),
                        "{v} carries a character TKEY does not allow"
                    );
                }
            }
        }
    }

    #[test]
    fn off_the_wheel_values_are_rejected_rather_than_coerced() {
        for bad in ["0A", "13A", "8C", "", "A", "8", "-1A"] {
            assert!(Camelot::parse(bad).is_none(), "{bad} was accepted");
        }
    }
}
