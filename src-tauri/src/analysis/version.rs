//! What a version bump invalidates, declared in one place.
//!
//! The two numbers are deliberately independent. Analysis invalidates on
//! `content_hash` + `ANALYSIS_VERSION`; the energy constants are a separate,
//! permanent compatibility surface because energy numbers get written into the
//! user's file tags. Changing them is a migration, not a tweak.

/// Bump when any stage's output would differ for the same bytes. Every cached
/// result at a lower version is re-analysed.
///
/// **2 (2026-11-01): the key confidence calibration.** No estimate moved — the
/// key, the Camelot code and the tempo are bit-identical, and that was verified
/// on 532 real tracks — but `confidence`, `mode_ambiguous` and the runner-up
/// all changed, because `fuse`'s gates were refit off synthetic material onto a
/// real corpus. A row left at version 1 would keep the confidence of exactly
/// 0.000 that this calibration exists to remove, so it has to be re-read.
pub const ANALYSIS_VERSION: i32 = 2;

/// Bump only when the frozen energy constants change — the means, the standard
/// deviations, the seven weights or the quantile table. Freezing is the whole
/// trick: it is what makes a 7 mean the same thing in 2029, and what lets a
/// library be re-tagged without re-analysing a file.
///
/// **2 (2026-11-01): the first corpus-fitted constants.** Version 1's μ and σ
/// came from each feature's definition rather than from measurement, and on
/// real masters two of the seven features saturated — the whole full-track half
/// of a real library landed on 8, 9 or 10. Version 2 fits μ, σ and the decile
/// ladder on a held-out-split corpus; see `energy::constants` for what it was
/// fitted on. **Every stored energy number from version 1 is incomparable with
/// a version 2 one**, including the ones already written into users' Grouping
/// tags and filenames.
pub const ENERGY_MODEL_VERSION: i32 = 2;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The one combination that would show a user two different energy scales
    /// in one column. `analysis_cache`'s key is `(content_hash,
    /// analysis_version)` and `AnalysisRepository::lookup` reads nothing else,
    /// so an energy bump on its own is invisible to the cache: every stored row
    /// would keep being served with the energies the previous constants
    /// produced, next to freshly analysed tracks carrying the new ones. Both
    /// went to 2 together on 2026-11-01 for exactly that reason.
    ///
    /// Analysis may legitimately run ahead of energy — a key or tempo change
    /// does not touch the ladder. Energy may never run ahead of analysis, and
    /// if a cheap re-score path ever lands (`energy_current` plus the cached
    /// features) this test is what has to be rewritten to allow it.
    #[test]
    fn an_energy_bump_cannot_ship_without_an_analysis_bump() {
        assert!(
            ENERGY_MODEL_VERSION <= ANALYSIS_VERSION,
            "energy model {ENERGY_MODEL_VERSION} is ahead of analysis version \
             {ANALYSIS_VERSION}, so stored energies would not be invalidated"
        );
    }
}
