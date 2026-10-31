use serde::{Deserialize, Serialize};

use crate::models::{BeatgridMarker, DetectedCue, KeyMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResult {
    /// 0-11, 0 = C.
    pub root: u8,
    pub mode: KeyMode,
    pub camelot: String,
    pub confidence: f64,
    pub alt_root: Option<u8>,
    pub alt_mode: Option<KeyMode>,
    /// Set when the two voters disagree on mode or mode confidence is low. This
    /// drives the relative-pair display and the `Improve Tracks` collection —
    /// the one thing the incumbent does not do.
    pub mode_ambiguous: bool,
    pub tuning_cents: f64,
    /// Per-15-s-chunk keys. Stored but not displayed in v1; the deferred deck
    /// view reads them, and chunk averaging produces them for free.
    pub chunk_keys: Vec<ChunkKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkKey {
    pub start_ms: f64,
    pub end_ms: f64,
    pub root: u8,
    pub mode: KeyMode,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoResult {
    /// Unfolded. Folding happens at read time against the user's range so the
    /// Tempo setting never costs a re-analysis.
    pub raw_bpm: f64,
    pub octave_shift: i32,
    pub confidence: f64,
    /// Least-squares grid-fit residual sd in ms. Under 15 ms the display may
    /// show two decimals; the tempogram itself cannot resolve them.
    pub residual_ms: f64,
    pub variable: bool,
    pub beats_ms: Vec<f64>,
    pub downbeat_phase: u8,
    pub downbeat_confidence: f64,
    pub beats_per_bar: u8,
    pub phrase_bars: u8,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnergyFeatures {
    /// Onsets per beat, tempo-normalised.
    pub onset_rate: f32,
    /// Mean half-wave-rectified spectral flux divided by frame RMS.
    pub flux_mean: f32,
    pub low_ratio: f32,
    pub high_ratio: f32,
    pub centroid: f32,
    pub perc_ratio: f32,
    /// 95th minus 50th percentile of EBU R128 short-term loudness, inverted.
    /// The only absolute quantity in the set, and it carries the lowest weight.
    pub dynamic_span: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnergyResult {
    pub energy: u8,
    pub raw: f64,
    /// Stamped because the constants are a permanent compatibility surface:
    /// energy numbers get written into the user's file tags, so changing them
    /// is a migration, not a tweak.
    pub model_version: i32,
    pub features: EnergyFeatures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionKind {
    Intro,
    Build,
    Drop,
    Breakdown,
    Outro,
    Unknown,
}

impl RegionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RegionKind::Intro => "intro",
            RegionKind::Build => "build",
            RegionKind::Drop => "drop",
            RegionKind::Breakdown => "breakdown",
            RegionKind::Outro => "outro",
            RegionKind::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "intro" => Some(RegionKind::Intro),
            "build" => Some(RegionKind::Build),
            "drop" => Some(RegionKind::Drop),
            "breakdown" => Some(RegionKind::Breakdown),
            "outro" => Some(RegionKind::Outro),
            "unknown" => Some(RegionKind::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackRegion {
    pub id: String,
    pub track_id: String,
    pub start_ms: f64,
    pub end_ms: f64,
    pub bar_index: Option<i32>,
    pub bars: Option<i32>,
    pub kind: RegionKind,
    /// Computed with the identical frozen constants as the track energy, so the
    /// numbers are comparable.
    pub energy: Option<u8>,
    pub key_root: Option<u8>,
    pub key_mode: Option<KeyMode>,
}

/// What S9 produces, before it is a row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedRegion {
    pub start_ms: f64,
    pub end_ms: f64,
    pub bar_index: Option<i32>,
    pub bars: Option<i32>,
    pub kind: RegionKind,
    pub energy: Option<u8>,
    pub key_root: Option<u8>,
    pub key_mode: Option<KeyMode>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelsResult {
    pub peak_db: f64,
    pub true_peak_db: f64,
    pub lufs_integrated: f64,
    pub clipped_peaks: i64,
}

/// Everything one pass over one file produced. This is the `analysis_cache`
/// payload: keyed by content hash, so a moved file, a file Keystone itself
/// renamed, and a duplicate all hit it with zero DSP.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    pub content_hash: String,
    pub analysis_version: i32,
    pub energy_model_version: i32,
    pub duration_ms: i64,
    pub key: KeyResult,
    pub tempo: TempoResult,
    pub energy: EnergyResult,
    pub levels: LevelsResult,
    pub cues: Vec<DetectedCue>,
    pub regions: Vec<DetectedRegion>,
    pub beatgrid: Vec<BeatgridMarker>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueState {
    #[default]
    Idle,
    Running,
    Paused,
}

/// The sidebar's Analysis Queue object. `Empty` in the UI is `state == Idle`
/// with nothing queued, which is a state rather than a percentage — hence the
/// separate `QueueFinished` event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    pub state: QueueState,
    pub queued: usize,
    pub running: usize,
    pub done: usize,
    pub failed: usize,
    pub from_cache: usize,
    pub total: usize,
    pub current_path: Option<String>,
    /// Left out until the pipeline has been measured on Windows. All published
    /// timings are from an M2 Max and Windows is roughly two thirds of installs.
    pub eta_ms: Option<u64>,
}

/// One cached zoom level of the waveform. Served from disk under app data keyed
/// by `content_hash`, never from the `tracks` row — the virtualized table
/// selects that row on every page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformLevel {
    pub samples_per_bucket: u32,
    pub min: Vec<f32>,
    pub max: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Waveform {
    pub content_hash: String,
    pub duration_ms: i64,
    pub sample_rate: u32,
    pub levels: Vec<WaveformLevel>,
}
