use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyMode {
    Major,
    Minor,
}

impl KeyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyMode::Major => "major",
            KeyMode::Minor => "minor",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "major" => Some(KeyMode::Major),
            "minor" => Some(KeyMode::Minor),
            _ => None,
        }
    }
}

/// Where a value came from. A user override must survive re-analysis, so every
/// detected column carries its provenance rather than a single global flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueSource {
    Detected,
    User,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexStatus {
    Pending,
    Indexed,
    Missing,
    Error,
}

/// One library row, exactly as the table renders it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// xxh3 of the path. Per-path identity on purpose: content-hash identity
    /// collapses byte-identical files at different paths into one row and makes
    /// them ping-pong on every reconcile, and DJ libraries are duplicate-heavy.
    pub id: String,
    pub path: String,
    pub filename: String,
    /// The name before Keystone renamed it, which is what makes the inverse
    /// strip exact rather than a regex guess.
    pub original_filename: Option<String>,
    pub rename_pattern: Option<String>,
    pub extension: String,
    /// xxh3 of the AUDIO REGION, never the file head — ID3 lives at the head, so
    /// hashing it means Keystone's own re-tag pass wipes its own analysis cache.
    pub content_hash: Option<String>,

    pub size_bytes: Option<i64>,
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bit_depth: Option<u16>,
    pub bitrate_kbps: Option<u32>,
    pub codec: Option<String>,

    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub grouping: Option<String>,
    pub comment: Option<String>,
    pub year: Option<i32>,
    pub tag_bpm: Option<f64>,
    pub tag_key: Option<String>,
    pub has_cover_art: bool,

    /// As detected, unfolded. Kept separately from `tempo_display` so changing
    /// the Tempo range setting re-folds from the database with no re-analysis.
    pub tempo_raw: Option<f64>,
    pub tempo_display: Option<f64>,
    pub tempo_octave_shift: Option<i32>,
    pub tempo_confidence: Option<f64>,
    /// Beat-grid fit residual sd. Under 15 ms gates the two-decimal display;
    /// above it, two decimals are false precision.
    pub tempo_residual_ms: Option<f64>,
    pub tempo_variable: bool,

    pub key_root: Option<u8>,
    pub key_mode: Option<KeyMode>,
    pub camelot: Option<String>,
    pub key_confidence: Option<f64>,
    pub key_alt_root: Option<u8>,
    pub key_alt_mode: Option<KeyMode>,
    /// Most residual key error is mode, which is exactly Camelot's A/B letter.
    /// When this is set the UI shows the relative pair (`8A/8B`) instead of
    /// committing to a letter.
    pub key_mode_ambiguous: bool,
    pub tuning_cents: Option<f64>,

    pub energy: Option<u8>,
    pub energy_raw: Option<f64>,
    pub peak_db: Option<f64>,
    pub true_peak_db: Option<f64>,
    pub lufs_integrated: Option<f64>,
    pub clipped_peaks: Option<i64>,

    pub rating: Option<u8>,
    pub date_added: i64,
    pub last_played: Option<i64>,
    pub play_count: i64,

    pub key_source: Option<ValueSource>,
    pub tempo_source: Option<ValueSource>,
    pub energy_source: Option<ValueSource>,
    pub analysis_version: i32,
    pub energy_model_version: i32,
    pub analyzed_at: Option<i64>,
    pub index_status: IndexStatus,
    pub error_message: Option<String>,

    pub created_at: i64,
    pub updated_at: i64,
    pub last_seen_at: i64,
}

/// The row the walker writes before any analysis runs, so a freshly added
/// folder is visible in the table immediately.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackUpsert {
    pub id: String,
    pub path: String,
    pub filename: String,
    pub extension: String,
    pub size_bytes: Option<i64>,
    pub content_hash: Option<String>,
    pub tags: Option<crate::models::TagSnapshot>,
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bit_depth: Option<u16>,
    pub bitrate_kbps: Option<u32>,
    pub codec: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackSort {
    Artist,
    Title,
    Album,
    Genre,
    Tempo,
    KeyResult,
    Energy,
    DateAdded,
    Rating,
    CuePointCount,
    Bitrate,
    Year,
    Filename,
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackFilters {
    /// Free text, routed through the FTS5 MATCH path.
    pub query: String,
    /// The wheel selection, e.g. `8A`. When `camelot_include_compatible` is set
    /// this expands to the four-element compatibility set.
    pub camelot: Option<String>,
    pub camelot_include_compatible: bool,
    pub collection_id: Option<String>,
    pub energy_min: Option<u8>,
    pub energy_max: Option<u8>,
    pub tempo_min: Option<f64>,
    pub tempo_max: Option<f64>,
    pub genres: Vec<String>,
    pub rating_min: Option<u8>,
    pub analyzed: Option<bool>,
    /// Range-scanned as `path >= ? AND path < ?`. Never `LIKE 'prefix%'` —
    /// SQLite's LIKE is case-insensitive by default, which kills the prefix
    /// optimisation and forces a full scan.
    pub path_prefix: Option<String>,
    pub index_statuses: Vec<IndexStatus>,
    /// The spec's `Improve Tracks` collection: voter disagreement, low key
    /// confidence, clipping, or a missing analysis.
    pub improvable_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackQuery {
    pub filters: TrackFilters,
    pub sort: TrackSort,
    pub direction: SortDirection,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackPage {
    pub tracks: Vec<Track>,
    pub total: usize,
}

/// One count per wheel segment, so segments with nothing behind them can read
/// as empty instead of filtering to zero rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelCounts {
    /// 24 entries, `("1A", n)` .. `("12B", n)`.
    pub counts: Vec<(String, usize)>,
    pub unkeyed: usize,
}
