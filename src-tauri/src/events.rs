//! Every backend -> frontend event name, declared once.
//!
//! The frontend listens to these in a single sync hook, so a name that exists in
//! two places drifts. Add a constant here and a payload struct beside it; never
//! inline a string literal at an `emit` call site.

use serde::{Deserialize, Serialize};

use crate::models::{AnalysisProgress, ScanProgress, Track};

/// Analysis queue progress. Emitted on a timer while the queue runs, not per
/// track: at 8 workers a per-track emit floods the webview.
pub const ANALYSIS_PROGRESS: &str = "keystone://analysis-progress";

/// One track's row changed — analysis landed, a tag was written, a rating moved.
/// Carries the whole row so the table can patch in place without a refetch.
pub const TRACK_UPDATED: &str = "keystone://track-updated";

/// The queue drained. Distinct from a progress event reaching 100%, because the
/// sidebar's `Empty` state is a state, not a percentage.
pub const QUEUE_FINISHED: &str = "keystone://queue-finished";

/// Filesystem walk progress, separate from analysis: the walk is IO-bound and
/// finishes long before the CPU stage.
pub const SCAN_PROGRESS: &str = "keystone://scan-progress";

/// Startup reconciliation finished; invalidates the track and collection queries.
pub const RECONCILE_COMPLETE: &str = "keystone://reconcile-complete";

/// A waveform peak pyramid finished caching for this content hash.
pub const WAVEFORM_READY: &str = "keystone://waveform-ready";

/// Per-file progress of a cue-point export run, keyed by `run_id` so an
/// interrupted 5,000-track run can be reported and resumed.
pub const EXPORT_PROGRESS: &str = "keystone://export-progress";

/// Per-file progress of a re-tag or rename run. Same shape, different verb.
pub const TAG_PROGRESS: &str = "keystone://tag-progress";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgressEvent {
    pub progress: AnalysisProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackUpdatedEvent {
    pub track: Track,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueFinishedEvent {
    pub analyzed: usize,
    pub failed: usize,
    /// Cache hits: a moved, renamed or duplicate file that cost no DSP. Worth
    /// surfacing because it is the answer to "why was the re-scan instant".
    pub from_cache: usize,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressEvent {
    pub progress: ScanProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformReadyEvent {
    pub track_id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProgressEvent {
    pub run_id: String,
    pub done: usize,
    pub total: usize,
    pub failed: usize,
    pub current_path: Option<String>,
}
