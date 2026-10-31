use serde::{Deserialize, Serialize};

/// The role S11 assigned to a marker. The label is worth more to a DJ than an
/// unlabelled confidence ranking, which is why the role is stored rather than
/// derived from the slot number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueKind {
    Intro,
    FirstBeat,
    Breakdown,
    Build,
    Drop,
    Outro,
    Loop,
    User,
}

impl CueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CueKind::Intro => "intro",
            CueKind::FirstBeat => "first_beat",
            CueKind::Breakdown => "breakdown",
            CueKind::Build => "build",
            CueKind::Drop => "drop",
            CueKind::Outro => "outro",
            CueKind::Loop => "loop",
            CueKind::User => "user",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "intro" => Some(CueKind::Intro),
            "first_beat" => Some(CueKind::FirstBeat),
            "breakdown" => Some(CueKind::Breakdown),
            "build" => Some(CueKind::Build),
            "drop" => Some(CueKind::Drop),
            "outro" => Some(CueKind::Outro),
            "loop" => Some(CueKind::Loop),
            "user" => Some(CueKind::User),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CueSource {
    Detected,
    User,
}

/// The eight CUE markers. `slot` is 0-7 and unique per track, which is what
/// makes `CUE 1`..`CUE 8` and the A-H hot-cue mapping one-to-one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CuePoint {
    pub id: String,
    pub track_id: String,
    pub slot: u8,
    pub kind: CueKind,
    /// Milliseconds as REAL, not an integer: rekordbox stores 1/150 s frames and
    /// Traktor a float, so rounding here loses positions on the way out.
    pub position_ms: f64,
    /// Loops only.
    pub end_ms: Option<f64>,
    pub bar_index: Option<i32>,
    /// 4 | 8 | 16 | 32.
    pub phrase_level: Option<u8>,
    pub label: Option<String>,
    pub color: Option<String>,
    pub confidence: Option<f64>,
    pub source: CueSource,
    pub created_at: i64,
}

/// What S11 produces, before it is a row. Has no id and no track, so the
/// analysis stage never needs to know how persistence identifies things.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedCue {
    pub slot: u8,
    pub kind: CueKind,
    pub position_ms: f64,
    pub end_ms: Option<f64>,
    pub bar_index: Option<i32>,
    pub phrase_level: Option<u8>,
    pub label: Option<String>,
    pub confidence: f64,
}

/// One tempo segment, mirroring rekordbox's TEMPO element so the export is
/// musically correct rather than a faithful copy of rekordbox's own output
/// (which writes one marker per beat and is enormous for no gain).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatgridMarker {
    pub id: String,
    pub track_id: String,
    pub position_ms: f64,
    pub bpm: f64,
    pub beats_per_bar: u8,
    pub beat_in_bar: u8,
}

/// A cue the user is adding or dragging. Separate from `CuePoint` because the
/// frontend never invents ids or timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CueEdit {
    pub track_id: String,
    pub slot: u8,
    pub position_ms: f64,
    pub end_ms: Option<f64>,
    pub kind: Option<CueKind>,
    pub label: Option<String>,
    pub color: Option<String>,
}
