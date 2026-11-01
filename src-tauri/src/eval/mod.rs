//! The accuracy harness. **Dev-only, behind the `eval` feature, never shipped
//! in the bundle.**
//!
//! **Build this before the detector.** The first commits of the engine are this
//! harness and a re-score of the community S-KEY ONNX export, because nobody
//! has independently confirmed its claimed 72.1 and that one number decides
//! whether the research track is funded at all.
//!
//! Licence boundary: evaluating against CC BY-SA data does not infect MIT code.
//! **Deriving and shipping key profiles or energy constants from it does** raise
//! ShareAlike, so anything fitted here stays here unless it was fitted on a
//! corpus we own.
//!
//! Two corpora, and the difference between them is the whole reason the module
//! is laid out this way:
//!
//! - [`synthetic`] over [`corpus`] runs anywhere, needs no files, and states its
//!   own ground truth. It answers engineering questions: is the scorer right, is
//!   a detune recovered, is percussion refused. It **cannot** answer whether the
//!   engine is accurate, and [`report::CAVEAT`] says so inside every report it
//!   prints.
//! - [`giantsteps`] is the real gate from ENGINE.md 7.1. S1 decode has landed,
//!   so [`run`] reads the archive's audio and scores it. What it reports is
//!   still the **template voter alone**, because there is no bundled graph in
//!   the tree for the model voter to load — so the figure it prints is a floor
//!   for the engine, not the engine's number.

pub mod corpus;
pub mod giantsteps;
pub mod key_path;
pub mod mirex;
pub mod one_track;
pub mod report;
pub mod synthetic;

#[cfg(test)]
mod mirex_tests;

use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;

use crate::analysis::decode::{self, DecodeOptions};
use crate::error::{KeystoneError, Result};
use crate::models::KeyMode;

pub struct HarnessOptions {
    /// Score the template voter alone, which is what isolates the ensemble's
    /// contribution.
    pub skey: bool,
    pub template: bool,
    pub tuning_correction: bool,
    pub threads: usize,
}

impl Default for HarnessOptions {
    fn default() -> Self {
        // S-KEY off because there is no bundled graph in the tree. Stated as a
        // default rather than hidden, so a report that says "template only" is
        // reporting a configuration and not an accident.
        Self { skey: false, template: true, tuning_correction: true, threads: 1 }
    }
}

pub struct HarnessReport {
    pub tracks_scored: usize,
    pub key: mirex::KeyScore,
    pub elapsed_ms: u64,
    /// Annotations the loader found, which is **not** the number scored.
    ///
    /// Reported separately and never reconciled silently: a harness that
    /// quietly shrinks its own corpus is the single easiest way to publish a
    /// wrong number, so the two counts are printed side by side and the files
    /// behind the difference are listed.
    pub annotations_loaded: usize,
    /// Every annotation whose audio could not be read, with the reason.
    pub unreadable: Vec<(PathBuf, String)>,
    /// Annotations recording a key change, which the 24-key taxonomy cannot
    /// represent. Counted, not dropped — dropping the tracks every app failed
    /// is exactly what makes a marketed accuracy figure meaningless.
    pub key_changes: usize,
}

/// Runs the engine over an annotated corpus on disk.
///
/// The key path is [`key_path`], the same one the synthetic harness uses, so
/// the two corpora are scored by identical code and a difference between them
/// is a difference in the material rather than in the measurement.
pub fn run(corpus: &Path, opts: &HarnessOptions) -> Result<HarnessReport> {
    if opts.skey {
        // Refused rather than attempted: `SkeySession::from_path` is a `todo!()`
        // and a panic inside a 600-track run would read as a harness bug.
        return Err(KeystoneError::Analysis(
            "S-KEY was asked for and there is no bundled graph in the tree, so there is \
             nothing to load. Run with `skey: false` and the report is the template voter \
             alone, which ENGINE.md §7.1 wants measured in its own right."
                .into(),
        ));
    }
    if !opts.template {
        return Err(KeystoneError::Analysis(
            "both voters are switched off, so there is nothing to score".into(),
        ));
    }
    let annotations = giantsteps::load(corpus)?;
    if annotations.is_empty() {
        return Err(KeystoneError::NotFound(format!(
            "no .key annotations under {} — GiantSteps+ is Zenodo record 1095691 and the \
             archive is not in this repository",
            corpus.display()
        )));
    }

    let started = Instant::now();
    let mut report = HarnessReport {
        tracks_scored: 0,
        key: mirex::KeyScore::default(),
        elapsed_ms: 0,
        annotations_loaded: annotations.len(),
        unreadable: Vec::new(),
        key_changes: annotations.iter().filter(|a| a.has_key_change).count(),
    };
    let mut pairs: Vec<mirex::Pair> = Vec::with_capacity(annotations.len());
    for (annotation, outcome) in scored(&annotations, opts) {
        match outcome {
            Ok(estimate) => pairs.push(((annotation.root, annotation.mode), estimate)),
            Err(e) => report
                .unreadable
                .push((annotation.audio_path.clone(), e.to_string())),
        }
    }

    report.key = mirex::score(&pairs);
    report.tracks_scored = pairs.len();
    report.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(report)
}

/// Every annotation decoded and scored, `opts.threads` at a time, in corpus
/// order so the report lists misses in the order the loader found them.
///
/// Decode is ~90% of the per-track cost here as it is in the app, so the only
/// thing worth parallelising is the whole per-track pass.
fn scored<'a>(
    annotations: &'a [giantsteps::Annotation],
    opts: &HarnessOptions,
) -> Vec<(&'a giantsteps::Annotation, Result<(u8, KeyMode)>)> {
    let work = || {
        annotations
            .par_iter()
            .map(|a| (a, score_one(a, opts)))
            .collect()
    };
    match rayon::ThreadPoolBuilder::new()
        .num_threads(opts.threads.max(1))
        .build()
    {
        Ok(pool) => pool.install(work),
        // A pool that will not build is not a reason to refuse the run.
        Err(_) => annotations.iter().map(|a| (a, score_one(a, opts))).collect(),
    }
}

/// One annotated track: decode it, run the key path, return the estimate.
///
/// Only the 44.1 kHz buffer and no peaks — the harness scores keys, and asking
/// for the model's rate or a waveform pyramid would spend most of a 600-track
/// run on bytes nothing here reads.
fn score_one(
    annotation: &giantsteps::Annotation,
    opts: &HarnessOptions,
) -> Result<(u8, KeyMode)> {
    let decoded = decode::decode(
        &annotation.audio_path,
        &DecodeOptions {
            want_key_rate: false,
            want_peaks: false,
            peak_zoom_levels: Vec::new(),
        },
    )?;
    let sr = decode::ANALYSIS_SAMPLE_RATE;
    let run = if opts.tuning_correction {
        key_path::run(&decoded.mono_44100, sr, None)?
    } else {
        key_path::run_without_tuning(&decoded.mono_44100, sr, None)?
    };
    Ok((run.detection.result.root, run.detection.result.mode))
}
