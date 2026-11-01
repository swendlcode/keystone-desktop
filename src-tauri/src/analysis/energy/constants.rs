//! The frozen energy constants.
//!
//! These are compiled into the binary and stamped as `ENERGY_MODEL_VERSION`.
//! Freezing is the whole trick: it is what makes a 7 mean the same thing in
//! 2029, and what lets a library be re-tagged without re-analysing a file.
//! Energy numbers get written into the user's file tags, so this table is a
//! permanent compatibility surface — changing a single number here is a
//! migration with a version bump, not a tweak.
//!
//! **There is no reference store and no correlation step.** The score is
//! `energy = round(1 + 9 · Q(Σ wᶠ · Φ((ln xᶠ − μᶠ)/σᶠ)))` and nothing else: the
//! numbers below are the whole model. That is a deliberate design constraint,
//! not an implementation detail — see the patent note in the parent module. The
//! constants below were *fitted* on a corpus, and that corpus is deliberately
//! **not shipped**: what ships is eleven knots and fourteen moments, which is a
//! formula, not a database of reference compositions to look a track up in.
//!
//! ## Provenance
//!
//! * The **seven weights** are the engine plan's, in `EnergyFeatures` field
//!   order, and they sum to exactly 1 so `raw` lands in [0, 1] by construction.
//!   **They are not fitted.** ENGINE.md §7.3 test 4 fits them from ~300
//!   forced-choice human pairs under a Bradley–Terry loss; until those pairs
//!   exist, fitting weights on a corpus that cannot say which of two tracks is
//!   the more intense would be inventing the answer.
//! * **μ and σ** are the location and scale of `ln x` per feature, **fitted
//!   2026-11-01** on the corpus below. μ is the median and σ is half the
//!   15.9–84.1 interpercentile range — robust rather than the sample mean and
//!   standard deviation, because `z` is clamped at ±3 and a heavy tail must not
//!   be allowed to inflate σ and so switch a feature off. Which is exactly what
//!   version 1 did: its μ and σ were set from each feature's *definition*
//!   rather than measured, and on real masters `flux_mean` sat at `z = +2.8`
//!   with **99.4% of tracks above Φ = 0.98** while `dynamic_span` had 40.9%
//!   above it. Two of the seven features, 0.26 of the weight, were contributing
//!   a constant.
//! * The **quantile table** is the decile ladder of `raw` over the same fit
//!   corpus. It is the one place a change is visible to a user as every number
//!   moving at once, so it is stated as eleven explicit knots rather than
//!   derived at runtime.
//!
//! ## The corpus these were fitted on, and what would invalidate them
//!
//! The corpus is `docs/ACCURACY.md` §1 rebuilt verbatim — 1,063 distinct files
//! from one DJ's library — split in half by a hash of each filename, stratified
//! by folder, **before any audio was read**. μ, σ and the ladder come from the
//! fit half of its **full-track** groups only (`club`, `capstone`, `readyfile`,
//! `misc`: n = 176 of 350). The held half chose nothing here.
//!
//! What the held half did on arrival — 174 held-out full tracks measured
//! through the engine, one per track, the 175th being a truncated file that
//! does not decode:
//!
//! | energy | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
//! |---|---|---|---|---|---|---|---|---|---|---|
//! | version 1 | 0 | 1 | 0 | 1 | 0 | 1 | 10 | 25 | 64 | 72 |
//! | version 2 | 17 | 15 | 5 | 32 | 12 | 20 | 23 | 20 | 13 | 17 |
//!
//! Mean 9.07 -> 5.62, seven distinct values -> ten. The Spearman correlation
//! between version 1's `raw` and version 2's on those same tracks is **0.957**,
//! so this is a restretch and not a reordering: version 1 already ranked the
//! library correctly and then threw the ranking away in the mapping onto 1-10.
//! Fitting the ladder guarantees the *spread*; it does not and cannot prove the
//! *order* is right. That needs ENGINE.md §7.3 test 4, the human pairs.
//!
//! The vocal-stem and acapella groups — 712 of the 1,063 files — were
//! deliberately **excluded from the fit**. They are sample-pack material, not
//! records anybody mixes, and letting two thirds of the corpus be drum-less
//! stems would put the median where a stem is and push every real track back to
//! the top of the scale, which is the defect this version exists to fix. The
//! consequence is in the open rather than hidden: an isolated acapella now
//! reads 1 or 2.
//!
//! So **a 5 is the median full track in a house and pop DJ library.** The scale
//! is relative to the kind of record a DJ actually owns, which is the only
//! anchor available without a genre detector — and the parent module is
//! explicit that normalising per genre is forbidden, because the user's library
//! is not a sample of the world.
//!
//! What would invalidate this fit: a library whose centre of gravity is not
//! four-to-the-floor house and pop (there is no classical, jazz, live or rubato
//! material among those 350 tracks), or any change to the seven features in
//! [`super::features`], to the HPSS reductions they read, or to the
//! core-region rule — each of which moves `x` under a fixed μ. Re-run the
//! harness's `--energy-features` over the corpus and refit; do not nudge a
//! knot.

/// Weights in `EnergyFeatures` field order: `onset_rate`, `flux_mean`,
/// `low_ratio`, `high_ratio`, `centroid`, `perc_ratio`, `dynamic_span`.
///
/// Unchanged from version 1; the provenance note says why they were not fitted.
pub const WEIGHTS: [f32; 7] = [0.22, 0.20, 0.14, 0.14, 0.10, 0.14, 0.06];

/// Median of `ln x`, per feature, over the fit corpus.
///
/// | # | feature | centre of `x` | version 1 assumed |
/// |---|---|---|---|
/// | 1 | `onset_rate` | 1.17 onsets per beat | 2.0 |
/// | 2 | `flux_mean` | 0.558, the summed per-band rise against the frame's L2 norm | 0.09 — six times out |
/// | 3 | `low_ratio` | 0.576 of frame **power** below 150 Hz — a power ratio, so the bass dominates | 0.40 |
/// | 4 | `high_ratio` | 0.0229 of frame power above 6 kHz | 0.012 |
/// | 5 | `centroid` | 3,744 Hz — stored in Hz, so this `ln` *is* the log-Hz the feature table asks for | 1,800 Hz |
/// | 6 | `perc_ratio` | 0.483 percussive after HPSS | 0.42 |
/// | 7 | `dynamic_span` | 1/0.98, the reciprocal of a 0.98 LU short-term spread | 1/3.0 |
pub const MU: [f32; 7] =
    [0.155_10, -0.584_34, -0.551_27, -3.775_06, 8.228_19, -0.717_07, 0.018_65];

/// Half the 15.9–84.1 interpercentile range of `ln x`, per feature, over the
/// fit corpus. `z` is clamped to ±3, so each of these also fixes the range over
/// which a feature still moves the score — and because these are σ of a log,
/// that range is multiplicative:
///
/// | feature | still moves the score across |
/// |---|---|
/// | `onset_rate` | 0.22–6.3 onsets per beat |
/// | `flux_mean` | 0.356–0.873 |
/// | `low_ratio` | 0.277 upwards (±3σ exceeds 1, which its own definition caps) |
/// | `high_ratio` | 0.0034–0.155 |
/// | `centroid` | 2,210–6,350 Hz |
/// | `perc_ratio` | 0.334–0.699 |
/// | `dynamic_span` | 0.147–7.1, i.e. a 0.14–6.8 LU spread |
///
/// Five of the seven are tighter than version 1 guessed, `perc_ratio` by almost
/// three times. A σ wider than the library's own spread is not the safe choice:
/// it flattens the feature, and a flattened feature is how a scale ends up
/// reporting 8, 9 or 10 for every record a DJ owns.
pub const SIGMA: [f32; 7] =
    [0.558_98, 0.149_40, 0.244_69, 0.637_77, 0.176_16, 0.122_88, 0.646_77];

/// `z` is clamped to this before Φ. Three standard deviations is where a
/// feature stops carrying information and starts carrying an outlier: an
/// 80-onset-per-beat glitch bar must not be able to drag a track to 10.
pub const Z_CLAMP: f32 = 3.0;

/// Decile boundaries of `raw`, from `q[0]` (the floor, energy 1) to `q[10]`
/// (the ceiling, energy 10). Monotone, which [`super::score::quantile_map`]
/// relies on.
///
/// `q[1..=9]` are the 10th to 90th percentiles of `raw` over the fit corpus, so
/// each decile of a real library is one integer wide — which is the whole claim
/// the 1-10 scale makes. `q[0]` and `q[10]` are 0 and 1 rather than corpus
/// extremes: `raw` is a weighted sum of CDFs whose weights sum to 1, so it
/// cannot leave [0, 1], and a clamp fitted from the two most extreme single
/// observations in the corpus would be the least reliable number in the table.
pub const QUANTILES: [f32; 11] = [
    0.0, 0.248_61, 0.321_88, 0.366_15, 0.451_16, 0.483_19, 0.533_70, 0.583_58, 0.659_00,
    0.731_47, 1.0,
];

/// Where each quantile knot lands in `Q`'s output.
///
/// `energy = round(1 + 9·Q)`, so the integer boundaries sit at
/// `Q = (k − 0.5)/9` for `k = 1..9`. Mapping the corpus deciles onto *those*
/// values rather than onto `k/10` is what makes each decile of the library one
/// integer wide, which is the whole claim the 1-10 scale makes.
pub const QUANTILE_TARGETS: [f32; 11] = [
    0.0,
    1.0 / 18.0,
    3.0 / 18.0,
    5.0 / 18.0,
    7.0 / 18.0,
    9.0 / 18.0,
    11.0 / 18.0,
    13.0 / 18.0,
    15.0 / 18.0,
    17.0 / 18.0,
    1.0,
];

/// The frozen constants, as one value so a caller cannot mix a new weight with
/// an old quantile table.
#[derive(Debug, Clone)]
pub struct FrozenConstants {
    pub mu: [f32; 7],
    pub sigma: [f32; 7],
    pub weights: [f32; 7],
    pub quantiles: [f32; 11],
    pub quantile_targets: [f32; 11],
}

impl Default for FrozenConstants {
    fn default() -> Self {
        Self {
            mu: MU,
            sigma: SIGMA,
            weights: WEIGHTS,
            quantiles: QUANTILES,
            quantile_targets: QUANTILE_TARGETS,
        }
    }
}

/// The one set of constants the whole app uses, track and section alike.
pub fn constants() -> FrozenConstants {
    FrozenConstants::default()
}
