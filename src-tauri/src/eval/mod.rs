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
//! - [`giantsteps`] is the real gate from ENGINE.md 7.1, and it is blocked on
//!   S1: `analysis::decode::decode` is still a stub, so no audio can be read off
//!   disk yet. The loader and the scorer are finished and tested, so the day S1
//!   lands this is a run rather than a build.

pub mod corpus;
pub mod giantsteps;
pub mod key_path;
pub mod mirex;
pub mod report;
pub mod synthetic;

#[cfg(test)]
mod mirex_tests;

use std::path::Path;

use crate::error::{KeystoneError, Result};

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
}

/// Runs the engine over an annotated corpus on disk.
///
/// The annotations load and score here; the audio does not, because S1 is a
/// stub. Returning the error rather than calling `decode` keeps that a stated
/// gap instead of a `todo!()` panic on whoever runs the harness first, and the
/// count in the message is proof the corpus itself was read.
pub fn run(corpus: &Path, _opts: &HarnessOptions) -> Result<HarnessReport> {
    let annotations = giantsteps::load(corpus)?;
    Err(KeystoneError::Analysis(format!(
        "the annotated-corpus path is blocked on S1: {} annotation(s) loaded from {}, but \
         analysis::decode::decode is still unimplemented, so no audio can be read. \
         eval::synthetic::run() is the path that works today, and its numbers do not \
         transfer to real audio.",
        annotations.len(),
        corpus.display()
    )))
}
