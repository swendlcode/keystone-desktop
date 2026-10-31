//! Key profiles, with their provenance recorded.
//!
//! **Licence boundary, and it is the reason this file exists at all.** Keystone
//! ships MIT. Sha'ath's numeric coefficients — the ones that score best on dance
//! music — exist in numeric form only inside GPL-3.0 libKeyFinder source; his
//! thesis plots them as bar charts with no table. `edmkey`, the bgate reference
//! implementation, declares no licence, which means all rights reserved. Both
//! shortcuts are closed.
//!
//! So: the profiles below are either published in books and papers
//! (Krumhansl-Schmuckler, Temperley) or constructed here from the papers'
//! *descriptions*. Nothing is transcribed from GPL source, and nothing fitted on
//! a CC BY-SA dataset ships — evaluating against GiantSteps+ does not infect MIT
//! code, but deriving and shipping profiles from it raises ShareAlike.

use crate::models::KeyMode;

/// Which of the four templates matched. The second minor profile exists for
/// tracks whose natural third outweighs the flat third, which is common in EDM
/// because rich synth spectra reinforce the tonic's fifth harmonic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileId {
    Major,
    MinorFlatThird,
    MinorNaturalThird,
    /// All energy on the tonic. **Amodal results are reported as minor.**
    Amodal,
}

impl ProfileId {
    /// What a win on this profile is reported as.
    ///
    /// Amodal maps to minor on purpose: an amodal track has no third to decide
    /// on, and a DJ mixing by Camelot letter is better served by the A ring than
    /// by a coin flip.
    pub fn mode(self) -> KeyMode {
        match self {
            ProfileId::Major => KeyMode::Major,
            _ => KeyMode::Minor,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyProfile {
    pub id: ProfileId,
    /// Relative weights for the 12 pitch classes, tonic first.
    pub weights: [f32; 12],
    /// Where these numbers came from. Required, not decorative: it is the
    /// record that the licence boundary above was respected.
    pub provenance: &'static str,
}

#[derive(Debug, Clone)]
pub struct ProfileSet {
    pub profiles: Vec<KeyProfile>,
}

/// A win on the amodal profile is scaled by this before it competes.
///
/// Without it the amodal template wins on any track where one pitch class leads
/// — which in EDM is most of them — and every result becomes minor. That is
/// exactly the trap `edmm` fell into: a flat major profile labelled nearly
/// everything minor, which flatters a weighted score on a dataset that is 84.8%
/// minor (72.0 on GiantSteps) and collapses on a modally balanced one (63.8 on
/// Beatport). The penalty makes amodal a last resort for material that genuinely
/// has no third rather than a shortcut to the A ring.
pub const AMODAL_PENALTY: f32 = 0.9;

/// Krumhansl & Schmuckler's probe-tone ratings, published in *Cognitive
/// Foundations of Musical Pitch* (Krumhansl 1990, Table 2.1). Freely citable
/// numbers from a book, which is exactly why they are the baseline.
///
/// Expect them to be weak on EDM: they describe tonal-hierarchy perception in
/// Western common-practice music, not four-to-the-floor spectra.
pub fn krumhansl_schmuckler() -> ProfileSet {
    ProfileSet {
        profiles: vec![
            KeyProfile {
                id: ProfileId::Major,
                weights: [
                    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
                ],
                provenance: "Krumhansl 1990, Cognitive Foundations of Musical Pitch, major probe-tone ratings",
            },
            KeyProfile {
                id: ProfileId::MinorFlatThird,
                weights: [
                    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
                ],
                provenance: "Krumhansl 1990, minor probe-tone ratings",
            },
        ],
    }
}

/// Temperley's revised weights (Temperley 2001, *The Cognition of Basic Musical
/// Structures*). Best of the published sets on The Beatles (81.2 MIREX) and
/// **actively harmful on EDM (48.6)** — kept because the eval harness needs a
/// genre-contrasting baseline, not because it is a candidate for the default.
pub fn temperley() -> ProfileSet {
    ProfileSet {
        profiles: vec![
            KeyProfile {
                id: ProfileId::Major,
                weights: [5.0, 2.0, 3.5, 2.0, 4.5, 4.0, 2.0, 4.5, 2.0, 3.5, 1.5, 4.0],
                provenance: "Temperley 2001, The Cognition of Basic Musical Structures, major",
            },
            KeyProfile {
                id: ProfileId::MinorFlatThird,
                weights: [5.0, 2.0, 3.5, 4.5, 2.0, 4.0, 2.0, 4.5, 3.5, 2.0, 1.5, 4.0],
                provenance: "Temperley 2001, minor",
            },
        ],
    }
}

/// Provenance shared by the four shipping profiles, spelled out once.
const SHIPPING_PROVENANCE: &str = "Constructed for Keystone from the description of electronic-dance-music key profiles in Faraldo, Jorda & Herrera (2016), starting from Temperley's published vectors and reshaping them as that description sets out: tonic triad raised, leading tone cut back, flat seventh raised. Not transcribed from any implementation, and not fitted on GiantSteps. Provisional - ENGINE.md 7.1 refits these on the eval corpus.";

/// The four-profile set the engine ships: one major, two minor, one amodal.
///
/// **bgate, not edmm**, despite edmm scoring 72.0 against bgate's 72.5 on
/// GiantSteps: edmm's major profile is flat so it labels nearly everything
/// minor, and GiantSteps is 84.8% minor. On the modally balanced Beatport set
/// edmm drops to 63.8 while bgate holds 73.4, and edmm produced 48% parallel
/// errors on The Beatles. A weighted score flattered by a dataset's mode
/// imbalance is not an accuracy improvement.
///
/// Three shape decisions, all from the cited description rather than from any
/// implementation, and all of them visible in the tests:
///
/// - The **leading tone is cut** (0.52 against Temperley's 4.0 of 5.0). It is
///   the degree common-practice harmony leans on hardest and the one EDM uses
///   least, and leaving it high is most of why Temperley scores 48.6 here.
/// - The **flat seventh is raised** in both modes. Modal writing over a static
///   bass is the idiom, and in minor it is what separates a key from its
///   relative major.
/// - The **flat third is high in the minor profile** (0.84). That looks
///   backwards — it is the relative major's tonic — but the relative pair is
///   separated by *which* degrees are strong together, and a chroma with a
///   strong tonic, fifth, flat third and flat seventh is minor while the same
///   notes with the major's weighting are not. Lowering it is what made an
///   earlier draft read `i - iv - i - bVII - bVI` as the relative major in
///   every one of the twelve keys.
///
/// And one constraint on the second minor profile that is easy to get wrong.
/// A "minor with a natural third" must stay **minor in its other degrees** —
/// flat sixth strong, natural sixth and leading tone near zero. Give it the
/// major's sixth and seventh as well and it becomes a slightly damped copy of
/// the major profile, which then splits the vote with it on the same root: a
/// plain `I - IV - V7 - I` in G measured 0.440 major against 0.438 minor, a
/// parallel coin flip on a progression with no minor third anywhere in it.
pub fn shipping_set() -> ProfileSet {
    ProfileSet {
        profiles: vec![
            KeyProfile {
                id: ProfileId::Major,
                weights: [
                    1.00, 0.16, 0.60, 0.18, 0.78, 0.68, 0.18, 0.86, 0.18, 0.48, 0.34, 0.52,
                ],
                provenance: SHIPPING_PROVENANCE,
            },
            KeyProfile {
                id: ProfileId::MinorFlatThird,
                weights: [
                    1.00, 0.16, 0.56, 0.84, 0.18, 0.66, 0.18, 0.86, 0.64, 0.18, 0.56, 0.46,
                ],
                provenance: SHIPPING_PROVENANCE,
            },
            KeyProfile {
                id: ProfileId::MinorNaturalThird,
                // A minor key whose third reads sharp, not a major key with
                // minor trimmings. The flat third stays the strongest third
                // because it is still the real one; the natural third is raised
                // only to the level the leakage puts it at. Flat sixth and flat
                // seventh stay strong and the natural sixth and leading tone
                // stay near zero, because those four degrees - not the third -
                // are what separate this from the major profile on the same
                // root. See the note above.
                weights: [
                    1.00, 0.16, 0.54, 0.72, 0.52, 0.64, 0.18, 0.86, 0.62, 0.16, 0.50, 0.20,
                ],
                provenance: SHIPPING_PROVENANCE,
            },
            KeyProfile {
                id: ProfileId::Amodal,
                // All energy on the tonic, per the bgate description. A one-hot
                // vector is not a mistake here: its cosine against a chroma is
                // that chroma's tonic share, which is precisely the question
                // "is this one note and a drum loop".
                weights: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                provenance: SHIPPING_PROVENANCE,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_baselines_are_well_formed() {
        for set in [krumhansl_schmuckler(), temperley()] {
            for p in &set.profiles {
                assert!(!p.provenance.is_empty(), "a profile shipped without provenance");
                assert!(
                    p.weights.iter().all(|w| *w > 0.0),
                    "a zero weight makes the cosine match ignore a pitch class"
                );
                // The tonic must be the strongest degree, or the match finds
                // the relative key more often than the tonic.
                let tonic = p.weights[0];
                assert!(
                    p.weights[1..].iter().all(|w| *w <= tonic),
                    "{:?}: a non-tonic degree outweighs the tonic",
                    p.id
                );
            }
        }
    }

    #[test]
    fn the_shipping_set_is_one_major_two_minor_and_an_amodal() {
        let set = shipping_set();
        assert_eq!(set.profiles.len(), 4);
        let ids: Vec<ProfileId> = set.profiles.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec![
                ProfileId::Major,
                ProfileId::MinorFlatThird,
                ProfileId::MinorNaturalThird,
                ProfileId::Amodal
            ]
        );
        assert_eq!(ProfileId::Major.mode(), KeyMode::Major);
        for id in [ProfileId::MinorFlatThird, ProfileId::MinorNaturalThird, ProfileId::Amodal] {
            assert_eq!(id.mode(), KeyMode::Minor, "{id:?} must report as minor");
        }
        for p in &set.profiles {
            assert!(!p.provenance.is_empty());
            let tonic = p.weights[0];
            assert!(tonic > 0.0);
            assert!(
                p.weights[1..].iter().all(|w| *w <= tonic),
                "{:?}: a non-tonic degree outweighs the tonic",
                p.id
            );
        }
    }

    #[test]
    fn the_major_profile_is_not_flat_which_is_the_edmm_failure() {
        // edmm's major profile was flat enough that nearly everything matched
        // minor. A flat major profile is the single defect that most flatters a
        // weighted score on a minor-heavy dataset, so it is worth a test of its
        // own: the diatonic degrees must stand clear of the chromatic ones.
        let set = shipping_set();
        let major = &set.profiles[0].weights;
        let diatonic = [0usize, 2, 4, 5, 7, 9, 11];
        let chromatic = [1usize, 3, 6, 8, 10];
        let weakest_diatonic =
            diatonic.iter().map(|i| major[*i]).fold(f32::INFINITY, f32::min);
        let strongest_chromatic =
            chromatic.iter().map(|i| major[*i]).fold(0.0f32, f32::max);
        assert!(
            weakest_diatonic > strongest_chromatic,
            "the major profile has gone flat: weakest diatonic {weakest_diatonic} \
             is not above strongest chromatic {strongest_chromatic}"
        );
    }
}
