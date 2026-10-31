//! S2 -> S3 -> S4 over one buffer, which is what "the key path" means in every
//! number this harness reports.
//!
//! It is written out rather than shortcut because the order and the residual
//! bookkeeping are part of what is being measured. S2 runs **first and is not
//! optional**: the model's front end has one bin per semitone and was measured
//! collapsing to the wrong tonic *and* the wrong mode at ±50 cents. Which of
//! S2's two routes the caller takes — resample the audio, or hand the offset to
//! the chroma bin grid — must not change the answer, so the harness takes the
//! same route the orchestrator does and the two are compared directly in the
//! tuning tests.

use crate::analysis::key::{self, skey::SkeySession, KeyDetection, KeyInput};
use crate::analysis::spectral::{chroma, tuning};
use crate::error::Result;

pub struct KeyRun {
    pub detection: KeyDetection,
    /// S2's measured offset for the file, in cents. This is the number the UI
    /// shows, and it is the measurement rather than the residual.
    pub tuning_cents: f32,
    /// Whether S2 asked for the corrective resample. False on unpitched
    /// material however large the offset looked.
    pub corrected: bool,
}

/// The key path, tuning first.
///
/// `skey` is `None` for every number in this harness today: there is no bundled
/// graph in the tree, so what is being scored is the **template voter alone**.
/// ENGINE.md §7.1 wants exactly that number as well as the fused one — scoring
/// each voter on its own is what decides whether the research track is funded —
/// but it has to be labelled, and [`super::report`] labels it.
pub fn run(samples: &[f32], sample_rate: u32, skey: Option<&SkeySession>) -> Result<KeyRun> {
    let est = tuning::estimate(samples, sample_rate)?;
    // Two routes, and the residual is what tells the chroma stage which one was
    // taken. Correcting against a residual of zero on an uncorrected buffer
    // leaves the detune in the chroma; reporting the residual instead of the
    // measurement tells a user with a vinyl rip that their track is at A440.
    let (audio, residual) = if est.needs_correction {
        (tuning::correct(samples, sample_rate, est.cents)?, 0.0)
    } else {
        (samples.to_vec(), est.cents)
    };
    let pcp = chroma::pcp(&audio, sample_rate, residual)?;
    let detection = key::detect_detailed(
        &KeyInput {
            pcp: &pcp,
            mono: &audio,
            sample_rate,
            // The model voter's rate is fixed by its graph. Empty while there
            // is no graph; the fusion treats a missing voter as missing rather
            // than as a vote for nothing.
            mono_22050: &[],
            tuning_cents: est.cents,
            residual_cents: residual,
        },
        skey,
    )?;
    Ok(KeyRun { detection, tuning_cents: est.cents, corrected: est.needs_correction })
}

/// The same path with S2 skipped, for the one question that needs it: how much
/// the tuning stage is actually worth on this corpus.
///
/// Both Faraldo and Sha'ath measured **no aggregate gain** from tuning handling
/// on digitally-produced EDM at A440 — Sha'ath disabled it by default in
/// KeyFinder for that reason. It is cheap insurance against catastrophic tail
/// failures, not a headline win, and the only way to keep that claim honest is
/// to be able to measure the engine without it.
pub fn run_without_tuning(
    samples: &[f32],
    sample_rate: u32,
    skey: Option<&SkeySession>,
) -> Result<KeyRun> {
    let pcp = chroma::pcp(samples, sample_rate, 0.0)?;
    let detection = key::detect_detailed(
        &KeyInput {
            pcp: &pcp,
            mono: samples,
            sample_rate,
            mono_22050: &[],
            tuning_cents: 0.0,
            residual_cents: 0.0,
        },
        skey,
    )?;
    Ok(KeyRun { detection, tuning_cents: 0.0, corrected: false })
}
