//! rekordbox XML — the only officially documented format (AlphaTheta's
//! `xml_format_list.pdf` v1.0.0). We write our own file; nothing of the user's
//! is touched, which is why this target ships first.

mod writer;

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::error::Result;
use crate::export::{ExportFailure, ExportReport, ExportRequest, ExportTrack, Mode};

/// The playlist node we emit beside the full collection.
///
/// Emitting one named node matters more than the format does, because
/// **Keystone cannot push anything into rekordbox.** The user must set
/// Preferences -> Advanced/Bridge -> Database -> Imported Library, enable the
/// `rekordbox xml` panel, right-click -> Import To Collection, and answer
/// **Yes** to "Do you want to load information in the tag of the library being
/// imported?" — which is the step that actually applies the cues. One node
/// gives them a single thing to right-click.
pub const PLAYLIST_NAME: &str = "Keystone Cue Points";

/// `POSITION_MARK/@Type`.
pub mod mark_type {
    pub const CUE: u8 = 0;
    pub const FADE_IN: u8 = 1;
    pub const FADE_OUT: u8 = 2;
    pub const LOAD: u8 = 3;
    pub const LOOP: u8 = 4;
}

/// `Rating` is one of six fixed values, not 0-5.
pub fn rating_attr(rating: u8) -> u16 {
    match rating.min(5) {
        0 => 0,
        1 => 51,
        2 => 102,
        3 => 153,
        4 => 204,
        _ => 255,
    }
}

/// `Location` must be a `file://localhost/...` URI with the path
/// percent-encoded. A plain path is silently ignored by rekordbox.
///
/// `/` and `:` are left alone: they are the path separator and the Windows
/// drive marker, and rekordbox's own exports carry both unencoded. Everything
/// outside the RFC 3986 unreserved set is encoded byte by byte, which is what
/// makes a UTF-8 filename survive.
pub fn location_uri(path: &Path) -> String {
    let as_posix = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://localhost");
    if !as_posix.starts_with('/') {
        out.push('/');
    }
    for b in as_posix.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Hot cue slot number. The 2011 spec documents `Num` 0/1/2 for hot cues A-C;
/// modern rekordbox uses **A-H = 0-7**, which maps our eight slots one to one.
/// Community-verified only, and it could change without notice.
pub fn hot_cue_num(slot: u8) -> i32 {
    (slot as i32).min(7)
}

/// `Kind`, as rekordbox labels containers in its own exports. Unknown
/// extensions are reported by extension rather than guessed into a format
/// rekordbox would then fail to open.
pub fn kind_attr(extension: &str) -> String {
    match extension.to_ascii_lowercase().as_str() {
        "mp3" => "MP3 File".into(),
        "wav" | "wave" => "WAV File".into(),
        "aif" | "aiff" => "AIFF File".into(),
        "flac" => "FLAC File".into(),
        "m4a" | "mp4" | "aac" => "M4A File".into(),
        other => format!("{} File", other.to_ascii_uppercase()),
    }
}

/// One `TEMPO` element.
#[derive(Debug, Clone, PartialEq)]
pub struct TempoMarker {
    /// `Inizio`, in seconds.
    pub inizio_s: f64,
    pub bpm: f64,
    /// `Metro`, e.g. `4/4`.
    pub metro: String,
    /// `Battito`, the beat's position in the bar, 1-based.
    pub battito: u8,
}

/// `TEMPO`: `Inizio` (seconds, float), `Bpm`, `Metro` (`"4/4"`), `Battito` (1-4).
///
/// **Two markers for a constant-tempo track, not one per beat.** rekordbox
/// 5.8.6+ writes one per beat, which is why real exports are enormous. What
/// matters is that rekordbox's own BPM and grid agree with ours, or **every
/// imported cue drifts** — correctness beats fidelity to rekordbox's output.
///
/// The second marker sits a whole number of bars after the first so its
/// `Battito` is the same downbeat, which is what pins the grid instead of
/// leaving rekordbox to infer it from one point.
///
/// No grid means no `TEMPO` element at all. A marker at `Inizio="0"` with a
/// plausible BPM is worse than none: rekordbox would accept it and every cue
/// would drift by the track's own offset.
pub fn tempo_markers(track: &ExportTrack) -> Vec<TempoMarker> {
    let grid = &track.beatgrid;
    if grid.is_empty() {
        return Vec::new();
    }
    let to_marker = |m: &crate::models::BeatgridMarker| TempoMarker {
        inizio_s: m.position_ms / 1000.0,
        bpm: m.bpm,
        metro: format!("{}/4", m.beats_per_bar.max(1)),
        battito: m.beat_in_bar.max(1),
    };
    if grid.len() > 1 {
        return grid.iter().map(to_marker).collect();
    }

    let first = to_marker(&grid[0]);
    let bar_s = 60.0 / first.bpm.max(1.0) * grid[0].beats_per_bar.max(1) as f64;
    let duration_s = track.track.duration_ms.unwrap_or(0) as f64 / 1000.0;
    let bars = (((duration_s - first.inizio_s) / bar_s).floor() as i64).max(1);
    let second = TempoMarker {
        inizio_s: first.inizio_s + bars as f64 * bar_s,
        ..first.clone()
    };
    vec![first, second]
}

/// Which notation goes in `Tonality` is **not stated anywhere public**: nothing
/// says what Mixed In Key puts there, and settling it needs one MIK export
/// inspected. Until then it follows the user's Key Notation setting, which is
/// already pre-formatted into `key_string` — so when the answer arrives it is a
/// change in one place rather than in every exporter.
pub fn tonality(track: &ExportTrack) -> String {
    track.key_string.clone()
}

/// Writes the whole document.
///
/// **Stream it with `quick_xml::Writer`, never build a DOM:** a 5,000-track
/// collection runs to tens of MB and millions of elements.
pub fn write_collection(dest: &Path, tracks: &[ExportTrack]) -> Result<usize> {
    let refs: Vec<&ExportTrack> = tracks.iter().collect();
    writer::write_document(dest, &refs)
}

/// Plan or write. Both go through the same counting pass, so the number the UI
/// shows before the run is produced by the code that performs it.
pub fn run(
    request: &ExportRequest,
    tracks: &[ExportTrack],
    dest: &Path,
    mode: Mode,
) -> Result<ExportReport> {
    let mut report = ExportReport::for_request(request);
    report.planned = tracks.len();

    // A track whose file has moved is reported, not written: rekordbox would
    // import an entry whose Location points at nothing and show it greyed out
    // with no explanation.
    let (present, missing): (Vec<&ExportTrack>, Vec<&ExportTrack>) = tracks
        .iter()
        .partition(|t| Path::new(&t.track.path).is_file());
    report.tracks_matched = present.len();
    report.tracks_missing = missing.len();
    report.skipped = missing.len();
    report.failures = missing
        .iter()
        .map(|t| ExportFailure {
            track_id: t.track.id.clone(),
            path: t.track.path.clone(),
            reason: "the file is no longer at this path".into(),
        })
        .collect();
    report.cues_to_write = present.iter().map(|t| t.cues.len()).sum();

    if !mode.writes() {
        return Ok(report);
    }

    let written = writer::write_document(dest, &present)?;
    report.written = written;
    report.verified = writer::verify_document(dest, written, report.cues_to_write)?;
    Ok(report)
}
