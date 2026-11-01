//! The analysis engine: the module tree, and the context one pass runs in. The
//! sequence itself is [`pipeline`], which is the only place the stage order
//! lives.
//!
//! One decode feeds every stage, so the only optimisation that matters at this
//! level is not decoding twice.
//!
//! Stage order is S1..S12 and it is not rearrangeable. S2 (tuning) runs before
//! S3 and S4 because the key model's front end has one bin per semitone and was
//! measured collapsing to the wrong tonic *and* wrong mode on a detuned input.

pub mod camelot;
pub mod chroma;
pub mod cues;
pub mod decode;
pub mod energy;
pub mod frontend;
pub mod key;
/// S1..S11 for one track, in the one order they may run in.
mod pipeline;
/// S6 + S7 + S8: onsets, tempo, the beat grid, downbeats and the phrase period.
pub mod rhythm;
/// S2 + S3: tuning, whitening, the constant-Q front end and the HPCP.
pub mod spectral;
pub mod structure;
pub mod tuning;
pub mod version;

/// S6, S7 and S8 are reached by their stage names as well as through
/// [`rhythm`], because that is how the rest of the engine refers to them: S9
/// takes a `beats::BeatAnalysis` and the track repo folds a tempo with
/// `tempo::fold_into_range`. They were briefly a second, unimplemented copy of
/// these three modules sitting beside the finished ones, which compiled and did
/// nothing — every `todo!()` in the pair was reachable from the names the
/// product used while the working code was not in the module tree at all. One
/// implementation, two paths to it.
pub use rhythm::{beats, onset, tempo};

use std::path::PathBuf;
use std::sync::Arc;

use crate::models::Settings;

pub use self::pipeline::analyze_track;

/// Everything one analysis pass needs that is not the audio: the loaded model
/// session (shared across workers — an ONNX session is expensive to build and
/// cheap to call), where peaks are cached, and the settings that affect what is
/// computed rather than how it is displayed.
pub struct AnalysisContext {
    pub skey: Option<Arc<key::skey::SkeySession>>,
    /// `<app data>/peaks`. Peaks never go into the `tracks` row: the
    /// virtualized table selects that row on every page.
    pub peaks_dir: PathBuf,
    /// The spec's "Cache audio wave image during analysis".
    pub cache_peaks: bool,
    /// The classical downbeat phase is the default. Beat This! is behind a gate
    /// because its fp32 export is 190 MB and it saturates at 60-68x realtime;
    /// ENGINE.md §2 says measure the agreement before bundling it.
    ///
    /// **Nothing can honour this yet, and `analyze_track` does not read it.**
    /// The seam is `beats::analyze_with_arbiter`, which takes downbeat *times*,
    /// and the only implementation of the trait that produces them is behind the
    /// `beat-this` cargo feature with no model to load. The field stays so the
    /// context does not change shape on the day a session exists; what it must
    /// not become is a flag that reads as "the arbiter ran".
    pub beat_arbiter: bool,
    pub settings: Settings,
}

impl AnalysisContext {
    pub fn new(peaks_dir: PathBuf, settings: Settings) -> Self {
        Self {
            skey: None,
            peaks_dir,
            cache_peaks: settings.cache_waveform_during_analysis,
            beat_arbiter: false,
            settings,
        }
    }
}

