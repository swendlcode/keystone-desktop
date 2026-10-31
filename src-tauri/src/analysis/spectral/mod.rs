//! S2 and S3: the spectral front end the key stage is built on.
//!
//! The order inside here is not rearrangeable. Tuning (S2) runs **before**
//! chroma (S3) and key (S4) because the key model's front end has one bin per
//! semitone and was measured collapsing to the wrong tonic *and* the wrong mode
//! on a detuned input; the chroma stage then takes the offset as a parameter so
//! it can undo the detune in the bin grid.
//!
//! Layout:
//!
//! - [`stft`] — the streaming frame loop, and spectral peaks with sub-bin
//!   interpolation. Everything else here sits on it.
//! - [`whitening`] — the 200 Hz high-pass and the envelope flattening, so a
//!   profile match is decided by which pitch classes are present rather than by
//!   how the track was mastered.
//! - [`tuning`] — S2. The estimate in cents, which the UI shows, and the
//!   corrective resample.
//! - [`vqt`] — the constant-Q / variable-Q filterbank.
//! - [`chroma`] — S3. The 36-bin HPCP, and the beat-synchronous 12-bin chroma
//!   the structure stage reads.

pub mod chroma;
pub mod stft;
pub mod tuning;
pub mod vqt;
pub mod whitening;

// Generators for the tests and for the dev-only eval harness. The harness has
// to build its corpus from the *same* generators the unit tests use: two copies
// of a chord renderer means one generator bug can flatter one stage's numbers
// while hiding from the other's.
#[cfg(any(test, feature = "eval"))]
pub mod test_signals;

#[cfg(any(test, feature = "eval"))]
pub mod test_unpitched;

#[cfg(test)]
mod spectral_tests;
