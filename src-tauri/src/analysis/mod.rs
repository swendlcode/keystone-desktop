//! The analysis engine. One decode feeds every stage: decode is roughly 90% of
//! the per-track cost, so the only optimisation that matters at this level is
//! not decoding twice.
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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::Result;
use crate::models::{AnalysisResult, Settings};

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

/// S1..S11 for one file, returning the whole `analysis_cache` payload.
///
/// Owes, in order: decode once (S1), tuning (S2), chroma (S3), key (S4), Camelot
/// (S5), onsets (S6), tempo and grid (S7), downbeats and phrase period (S8),
/// structure and regions (S9), energy (S10), cues (S11). Persistence (S12) is
/// the caller's: this function must not touch the pool.
///
/// Never holds a decoded track longer than the stage that needs it — a
/// 10-minute 48 kHz stereo f32 buffer is 230 MB, and eight workers hold eight.
pub fn analyze_track(_path: &Path, _ctx: &AnalysisContext) -> Result<AnalysisResult> {
    todo!("owes: S1..S11 orchestration over one shared decode, returning AnalysisResult")
}
