//! What a version bump invalidates, declared in one place.
//!
//! The two numbers are deliberately independent. Analysis invalidates on
//! `content_hash` + `ANALYSIS_VERSION`; the energy constants are a separate,
//! permanent compatibility surface because energy numbers get written into the
//! user's file tags. Changing them is a migration, not a tweak.

/// Bump when any stage's output would differ for the same bytes. Every cached
/// result at a lower version is re-analysed.
pub const ANALYSIS_VERSION: i32 = 1;

/// Bump only when the frozen energy constants change — the means, the standard
/// deviations, the seven weights or the quantile table. Freezing is the whole
/// trick: it is what makes a 7 mean the same thing in 2029, and what lets a
/// library be re-tagged without re-analysing a file.
pub const ENERGY_MODEL_VERSION: i32 = 1;

/// A cached result at this version can be read back as-is.
pub fn analysis_current(stored: i32) -> bool {
    stored == ANALYSIS_VERSION
}

/// The energy score can be reused without re-running the DSP. False means the
/// energy columns need recomputing from the cached features, not that the whole
/// track needs re-analysing.
pub fn energy_current(stored: i32) -> bool {
    stored == ENERGY_MODEL_VERSION
}
