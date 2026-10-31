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
//! not an implementation detail — see the patent note in the parent module.
//!
//! ## Provenance
//!
//! * The **seven weights** are the engine plan's, in `EnergyFeatures` field
//!   order, and they sum to exactly 1 so `raw` lands in [0, 1] by construction.
//! * **μ and σ** are the mean and standard deviation of `ln x` per feature.
//!   They are a first calibration, set from each feature's definition and its
//!   expected range on four-to-the-floor material rather than fitted, because
//!   the calibration corpus has to be ours: nothing fitted on the CC BY-SA
//!   evaluation sets may ship, for the same ShareAlike reason the key profiles
//!   cannot be. Replacing them with corpus-fitted values is the first thing
//!   `ENERGY_MODEL_VERSION = 2` will do.
//! * The **quantile table** is the decile ladder of `raw`. It is the one place
//!   a change is visible to a user as every number moving at once, so it is
//!   stated as eleven explicit knots rather than derived at runtime.

/// Weights in `EnergyFeatures` field order: `onset_rate`, `flux_mean`,
/// `low_ratio`, `high_ratio`, `centroid`, `perc_ratio`, `dynamic_span`.
pub const WEIGHTS: [f32; 7] = [0.22, 0.20, 0.14, 0.14, 0.10, 0.14, 0.06];

/// Mean of `ln x`, per feature.
///
/// | # | feature | centre of `x` |
/// |---|---|---|
/// | 1 | `onset_rate` | 2.0 onsets per beat |
/// | 2 | `flux_mean` | 0.09, the summed per-band rise against the frame's L2 norm |
/// | 3 | `low_ratio` | 0.40 of frame **power** below 150 Hz — a power ratio, so the bass dominates |
/// | 4 | `high_ratio` | 0.012 of frame power above 6 kHz |
/// | 5 | `centroid` | 1,800 Hz — stored in Hz, so this `ln` *is* the log-Hz the feature table asks for |
/// | 6 | `perc_ratio` | 0.42 percussive after HPSS |
/// | 7 | `dynamic_span` | 1/3.0, the reciprocal of a 3 LU short-term spread |
pub const MU: [f32; 7] = [0.693_15, -2.407_95, -0.916_29, -4.422_85, 7.495_54, -0.867_50, -1.098_61];

/// Standard deviation of `ln x`, per feature. `z` is clamped to ±3, so each of
/// these also fixes the range over which a feature still moves the score:
/// `high_ratio` at σ = 0.8 saturates outside roughly 0.002–0.2.
pub const SIGMA: [f32; 7] = [0.55, 0.65, 0.50, 0.90, 0.45, 0.35, 0.60];

/// `z` is clamped to this before Φ. Three standard deviations is where a
/// feature stops carrying information and starts carrying an outlier: an
/// 80-onset-per-beat glitch bar must not be able to drag a track to 10.
pub const Z_CLAMP: f32 = 3.0;

/// Decile boundaries of `raw`, from `q[0]` (the floor, energy 1) to `q[10]`
/// (the ceiling, energy 10). Monotone, which [`super::score::quantile_map`]
/// relies on.
pub const QUANTILES: [f32; 11] = [
    0.020, 0.295, 0.365, 0.416, 0.459, 0.500, 0.541, 0.584, 0.635, 0.705, 0.980,
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
