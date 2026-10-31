use serde::{Deserialize, Serialize};

/// The container a file's tags actually live in. Dispatching on this rather than
/// on a generic tag type is load-bearing: lofty's generic `Tag` drops anything
/// without a mapping, so one bulk replace over it would silently delete every
/// Serato GEOB blob, Traktor blob and ReplayGain frame in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Container {
    Id3v2,
    Ilst,
    VorbisComments,
    Ape,
    Riff,
    Aiff,
}

impl Container {
    pub fn as_str(self) -> &'static str {
        match self {
            Container::Id3v2 => "id3v2",
            Container::Ilst => "ilst",
            Container::VorbisComments => "vorbis",
            Container::Ape => "ape",
            Container::Riff => "riff",
            Container::Aiff => "aiff",
        }
    }
}

/// Everything the tag-editing view shows, read from the file rather than from
/// the database, because the file is the source of truth for tags.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSnapshot {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub grouping: Option<String>,
    pub comment: Option<String>,
    pub year: Option<i32>,
    /// Read as decimal where the container allows it. ID3v2 has no decimal BPM
    /// frame, so the displayed decimal tempo always comes from our own database
    /// and never from a tag round trip.
    pub bpm: Option<f64>,
    pub initial_key: Option<String>,
    pub has_cover_art: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TagField {
    Artist,
    Title,
    Album,
    Genre,
    Grouping,
    Comment,
    Year,
    Bpm,
    InitialKey,
}

/// A set of field writes. Modelled as explicit pairs rather than a struct of
/// `Option<Option<String>>` so "clear this field" and "leave it alone" cannot
/// be confused at a call site.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagEdit {
    pub fields: Vec<(TagField, Option<String>)>,
}

impl TagEdit {
    pub fn set(mut self, field: TagField, value: impl Into<String>) -> Self {
        self.fields.push((field, Some(value.into())));
        self
    }

    pub fn clear(mut self, field: TagField) -> Self {
        self.fields.push((field, None));
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverArt {
    pub mime: String,
    /// Base64 so it survives the IPC boundary; the frontend renders it as a
    /// data URL rather than going through the asset protocol for embedded art.
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagWriteReport {
    pub run_id: String,
    pub written: usize,
    pub verified: usize,
    pub skipped: usize,
    pub failed: usize,
    pub failures: Vec<TagWriteFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagWriteFailure {
    pub track_id: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagBackupEntry {
    pub id: String,
    pub track_id: String,
    pub container: Container,
    pub size_bytes: usize,
    pub written_at: i64,
}

/// What a rename would do, shown before anything moves. Renaming is the only
/// irreversible operation in the app and one setting runs it automatically, so
/// the plan is a first-class object rather than a log line.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePlan {
    pub items: Vec<RenamePlanItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePlanItem {
    pub track_id: String,
    pub from_path: String,
    pub to_path: String,
    /// Set when the item will not be renamed, with the reason: already correct
    /// (idempotence), a collision on a different file, an illegal component on
    /// Windows, or no key/tempo to write.
    pub skipped: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameReport {
    pub renamed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub failures: Vec<TagWriteFailure>,
}
