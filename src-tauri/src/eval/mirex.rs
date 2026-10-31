//! MIREX scoring. **Reports all five categories plus exact match, mode accuracy
//! and key-signature accuracy separately — never just the weighted total.**
//!
//! Two reasons, and they are the reasons the whole harness exists:
//!
//! 1. Most residual error in every published system is **mode**, which is
//!    exactly Camelot's A/B letter and the thing a DJ actually sees.
//! 2. GiantSteps is 84.8% minor, so a minor-biased model posts a good weighted
//!    score while being useless. `edmm` is the worked example: 72.0 on
//!    GiantSteps, 63.8 on a modally balanced set, 48% parallel errors on The
//!    Beatles.
//!
//! The number to beat is **75.70 MIREX weighted / 69.37% exact on GiantSteps**
//! (Mixed In Key 8.3). The ship gate for publishing any accuracy claim at all is
//! **>= 76.0 weighted on the 600-track set**. Below that, ship the product and
//! publish no number — publishing one without naming the measure is precisely
//! what we criticise the incumbent for.
//!
//! **One definition in the literature is genuinely ambiguous, so it is a
//! parameter here rather than a silent choice.** MIREX says "perfect fifth"
//! without stating a direction. `mir_eval` and Faraldo's own evaluation code
//! credit the **dominant only** (estimate = reference + 7 semitones, same mode),
//! and the published figures this harness compares against were produced that
//! way, so [`FifthConvention::Dominant`] is the default. Crediting the
//! subdominant as well raises every system's weighted score, so a score computed
//! one way must never be compared with a score computed the other. [`score`]
//! therefore reports the subdominant count separately and the weighted total
//! under both conventions, which is the only way the comparison stays honest.

use crate::models::KeyMode;

/// MIREX category weights, ISMIR 2018 §3.2.
pub const W_CORRECT: f64 = 1.0;
pub const W_FIFTH: f64 = 0.5;
pub const W_RELATIVE: f64 = 0.3;
pub const W_PARALLEL: f64 = 0.2;

/// The bar, from two independent measurements five years apart that agree within
/// two points.
pub const MIK_WEIGHTED: f64 = 75.70;
pub const MIK_EXACT: f64 = 69.37;
/// Publishing a number below this is not allowed.
pub const PUBLISH_GATE_WEIGHTED: f64 = 76.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Correct,
    Fifth,
    Relative,
    Parallel,
    Other,
}

impl Category {
    pub fn weight(self) -> f64 {
        match self {
            Category::Correct => W_CORRECT,
            Category::Fifth => W_FIFTH,
            Category::Relative => W_RELATIVE,
            Category::Parallel => W_PARALLEL,
            Category::Other => 0.0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Correct => "correct",
            Category::Fifth => "fifth",
            Category::Relative => "relative",
            Category::Parallel => "parallel",
            Category::Other => "other",
        }
    }
}

/// Which fifth relations earn the 0.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FifthConvention {
    /// Estimate is the **dominant** of the reference: `est = ref + 7`, same
    /// mode. What `mir_eval` implements and what the published numbers used, so
    /// it is the default and the only one a comparison may quote.
    #[default]
    Dominant,
    /// Estimate is a fifth either way: `est = ref ± 7`, same mode. Reported
    /// alongside, never instead.
    Both,
}

/// A reference and an estimate, as (pitch class 0-11, mode).
pub type Pair = ((u8, KeyMode), (u8, KeyMode));

#[derive(Debug, Clone, Default)]
pub struct KeyScore {
    pub n: usize,
    pub correct: usize,
    pub fifth: usize,
    pub relative: usize,
    pub parallel: usize,
    pub other: usize,
    /// `r_correct + 0.5*r_fifth + 0.3*r_relative + 0.2*r_parallel`, as a
    /// percentage, under [`KeyScore::convention`].
    pub weighted: f64,
    pub exact: f64,
    /// Reported separately. See the module note.
    pub mode_accuracy: f64,
    pub key_signature_accuracy: f64,
    /// Which fifths were credited when `weighted` was computed.
    pub convention: FifthConvention,
    /// Subdominant estimates: same mode, `est = ref + 5`. Under
    /// [`FifthConvention::Dominant`] these sit inside `other`, and this is how
    /// many of `other` they are.
    pub subdominant: usize,
    /// The same corpus scored under the other fifth convention. The gap between
    /// this and `weighted` is how much the ambiguity in the published definition
    /// is worth on this corpus.
    pub weighted_other_convention: f64,
    /// Fraction of the **references** that are minor. GiantSteps is 0.848, and
    /// a weighted score means something different on a corpus that is not.
    pub reference_minor: f64,
    /// Fraction of the **estimates** that are minor. Further from
    /// `reference_minor` than a point or two is a mode bias, which is the
    /// failure `edmm` posts a respectable weighted score with.
    pub estimate_minor: f64,
}

impl KeyScore {
    /// True only when the corpus is GiantSteps+ and the score clears the gate.
    /// Nothing else may be published as an accuracy claim.
    pub fn clears_publish_gate(&self) -> bool {
        self.weighted >= PUBLISH_GATE_WEIGHTED
    }

    pub fn rate(&self, count: usize) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            100.0 * count as f64 / self.n as f64
        }
    }
}

/// Classifies one estimate against one reference under the published
/// (dominant-only) fifth convention.
pub fn classify(est: (u8, KeyMode), reference: (u8, KeyMode)) -> Category {
    classify_with(est, reference, FifthConvention::Dominant)
}

/// As [`classify`], with the fifth convention stated.
pub fn classify_with(
    est: (u8, KeyMode),
    reference: (u8, KeyMode),
    convention: FifthConvention,
) -> Category {
    let (e_root, e_mode) = (est.0 % 12, est.1);
    let (r_root, r_mode) = (reference.0 % 12, reference.1);
    let interval = (e_root as i32 - r_root as i32).rem_euclid(12);

    if e_root == r_root && e_mode == r_mode {
        return Category::Correct;
    }
    if e_mode == r_mode {
        let fifth = match convention {
            FifthConvention::Dominant => interval == 7,
            FifthConvention::Both => interval == 7 || interval == 5,
        };
        return if fifth { Category::Fifth } else { Category::Other };
    }
    // Different mode from here down. The relative of a minor key is the major
    // three semitones above it (C minor -> Eb major); the relative of a major
    // key is the minor three semitones below it (C major -> A minor).
    let relative = match r_mode {
        KeyMode::Minor => interval == 3,
        KeyMode::Major => interval == 9,
    };
    if relative {
        return Category::Relative;
    }
    if e_root == r_root {
        return Category::Parallel;
    }
    Category::Other
}

/// Tallies every category and computes every reported figure.
pub fn score(pairs: &[Pair]) -> KeyScore {
    score_with(pairs, FifthConvention::Dominant)
}

/// As [`score`], with the fifth convention stated.
pub fn score_with(pairs: &[Pair], convention: FifthConvention) -> KeyScore {
    let mut s = KeyScore { n: pairs.len(), convention, ..KeyScore::default() };
    if pairs.is_empty() {
        return s;
    }

    let mut weight_sum = 0.0f64;
    let mut other_weight_sum = 0.0f64;
    let mut mode_hits = 0usize;
    let mut signature_hits = 0usize;
    let mut reference_minor = 0usize;
    let mut estimate_minor = 0usize;

    let other_convention = match convention {
        FifthConvention::Dominant => FifthConvention::Both,
        FifthConvention::Both => FifthConvention::Dominant,
    };

    for &(reference, est) in pairs {
        let category = classify_with(est, reference, convention);
        match category {
            Category::Correct => s.correct += 1,
            Category::Fifth => s.fifth += 1,
            Category::Relative => s.relative += 1,
            Category::Parallel => s.parallel += 1,
            Category::Other => s.other += 1,
        }
        weight_sum += category.weight();
        other_weight_sum += classify_with(est, reference, other_convention).weight();

        let interval = (est.0 as i32 - reference.0 as i32).rem_euclid(12);
        if est.1 == reference.1 && interval == 5 {
            s.subdominant += 1;
        }
        if est.1 == reference.1 {
            mode_hits += 1;
        }
        // A key signature is shared by exactly one major and one minor key, so
        // the signature is right when the estimate is the reference or its
        // relative. This is the figure that separates "wrong letter" from
        // "wrong notes", and it is why the two are never reported as one.
        if matches!(category, Category::Correct | Category::Relative) {
            signature_hits += 1;
        }
        if reference.1 == KeyMode::Minor {
            reference_minor += 1;
        }
        if est.1 == KeyMode::Minor {
            estimate_minor += 1;
        }
    }

    let n = pairs.len() as f64;
    s.weighted = 100.0 * weight_sum / n;
    s.weighted_other_convention = 100.0 * other_weight_sum / n;
    s.exact = 100.0 * s.correct as f64 / n;
    s.mode_accuracy = 100.0 * mode_hits as f64 / n;
    s.key_signature_accuracy = 100.0 * signature_hits as f64 / n;
    s.reference_minor = reference_minor as f64 / n;
    s.estimate_minor = estimate_minor as f64 / n;
    s
}
