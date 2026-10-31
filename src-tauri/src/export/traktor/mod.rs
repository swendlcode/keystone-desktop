//! Traktor NML. An event-stream rewrite, not a parse-and-reserialise.
//!
//! `~/Documents/Native Instruments/Traktor <x.y.z>/collection.nml`, the same
//! shape on macOS and Windows. Reading logic may be ported from
//! `JoergAtGithub/Traktor-Bridge`, which is MIT. Mixxx may be **read to
//! understand what Traktor does and never ported from** — it is
//! GPL-2.0-or-later, it is the best reference in open source, and it is
//! therefore the most likely way this project acquires a copyleft problem.

mod attr;
mod entry;
pub mod location;
pub mod rewrite;
pub mod unresolved;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_malformed;

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use crate::error::{KeystoneError, Result};
use crate::export::label::cue_label;
use crate::export::{safety, ExportFailure, ExportReport, ExportRequest, ExportTrack, Mode};
use crate::models::Settings;

pub use location::{detect_collection, location_key, LocationKey};
pub use rewrite::rewrite_document;
pub use unresolved::{hotcue_attr, musical_key_value, start_attr};

/// `CUE_V2/@TYPE`.
pub mod cue_type {
    pub const CUE: u8 = 0;
    pub const FADE_IN: u8 = 1;
    pub const FADE_OUT: u8 = 2;
    pub const LOAD: u8 = 3;
    /// The beat grid is a `CUE_V2` named `AutoGrid` with this type. The BPM
    /// itself lives in the separate `TEMPO` element.
    pub const GRID: u8 = 4;
}

pub const AUTOGRID_NAME: &str = "AutoGrid";

/// One cue as Traktor stores it. A loop is a cue with `LEN > 0`.
#[derive(Debug, Clone)]
pub struct TraktorCue {
    /// `None` for the grid marker, which is not one of the eight pads.
    pub slot: Option<u8>,
    pub name: String,
    pub position_ms: f64,
    pub len_ms: f64,
    pub cue_type: u8,
}

/// What a matched `ENTRY` has spliced into it. Every field is optional because
/// a track with no grid, or a run with `MUSICAL_KEY` left off, must change
/// nothing it does not have an answer for.
#[derive(Debug, Clone, Default)]
pub struct EntryEdit {
    pub cues: Vec<TraktorCue>,
    /// `INFO/@KEY`, the user-facing Key Text field read from the file tag, and
    /// the only one Mixed In Key writes.
    pub key_text: Option<String>,
    /// `MUSICAL_KEY/@VALUE`. Advanced opt-in only: Traktor's own Key column is
    /// backed by this and Traktor may overwrite it on re-analysis, and its
    /// numbering is unresolved in public sources.
    pub musical_key: Option<u8>,
    pub bpm: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct RewriteStats {
    pub entries_seen: usize,
    pub entries_matched: usize,
    pub entries_written: usize,
    /// Matched entries left verbatim because they already carried hot cues and
    /// the run was not allowed to overwrite.
    pub entries_skipped: usize,
    pub cues_written: usize,
    /// Existing cues actually dropped.
    pub cues_replaced: usize,
    /// Existing cues found on matched entries, whether dropped or not. This is
    /// the number the dry run shows as "would overwrite".
    pub cues_existing: usize,
    pub bytes_passed_through: usize,
    /// Which of our tracks were found, so the ones that were not can be named.
    pub matched_keys: Vec<LocationKey>,
}

/// Key Text, the default write target.
pub fn key_text(track: &ExportTrack) -> String {
    track.key_string.clone()
}

/// Single-pass streaming rewrite, then an atomic replace. Measured cost is
/// 1-3 s total for 5,000 tracks.
pub fn rewrite(
    nml: &Path,
    edits: &HashMap<LocationKey, EntryEdit>,
    overwrite_existing: bool,
) -> Result<RewriteStats> {
    let text = read_collection(nml)?;
    let (output, stats) = rewrite_document(&text, edits, overwrite_existing)?;
    // The last line of defence before the user's library is replaced: every
    // element we do not model must still be there, byte for byte.
    safety::assert_preserved(text.as_bytes(), output.as_bytes(), rewrite::MODELLED)?;
    safety::atomic_write(nml, |f| {
        f.write_all(output.as_bytes())?;
        Ok(())
    })?;
    Ok(stats)
}

/// Read as bytes and checked, rather than `read_to_string`: a collection in
/// some other encoding must be reported as an unexpected shape, not fail with
/// an io error that reads like a missing file.
fn read_collection(nml: &Path) -> Result<String> {
    if !nml.is_file() {
        return Err(KeystoneError::NotFound(format!(
            "no Traktor collection at {}. Point Keystone at collection.nml with CHANGE.",
            nml.display()
        )));
    }
    String::from_utf8(std::fs::read(nml)?).map_err(|_| {
        KeystoneError::Export(format!(
            "{} is not UTF-8 text, so it is not a collection Keystone recognises. \
             Nothing was written.",
            nml.display()
        ))
    })
}

/// Plan or write, through one code path.
pub fn run(
    request: &ExportRequest,
    tracks: &[ExportTrack],
    dest: &Path,
    settings: &Settings,
    mode: Mode,
) -> Result<ExportReport> {
    let text = read_collection(dest)?;
    let mut edits: HashMap<LocationKey, EntryEdit> = HashMap::new();
    let mut owners: HashMap<LocationKey, &ExportTrack> = HashMap::new();
    for track in tracks {
        let key = location_key(Path::new(&track.track.path));
        edits.insert(key.clone(), entry_edit(track, settings));
        owners.insert(key, track);
    }

    let (output, stats) = rewrite_document(&text, &edits, request.overwrite_existing)?;
    let mut report = report_from(request, tracks, &edits, &owners, &stats);
    if !mode.writes() {
        return Ok(report);
    }

    safety::assert_preserved(text.as_bytes(), output.as_bytes(), rewrite::MODELLED)?;
    safety::atomic_write(dest, |f| {
        f.write_all(output.as_bytes())?;
        Ok(())
    })?;
    report.written = stats.entries_written;
    report.verified = verify(dest, &stats)?;
    Ok(report)
}

fn report_from(
    request: &ExportRequest,
    tracks: &[ExportTrack],
    edits: &HashMap<LocationKey, EntryEdit>,
    owners: &HashMap<LocationKey, &ExportTrack>,
    stats: &RewriteStats,
) -> ExportReport {
    let mut report = ExportReport::for_request(request);
    report.planned = tracks.len();
    report.tracks_matched = stats.entries_matched;
    report.tracks_missing = edits.len().saturating_sub(stats.entries_matched);
    report.cues_to_write = edits.values().map(|e| e.cues.len()).sum();
    report.cues_to_overwrite = stats.cues_existing;
    report.skipped = stats.entries_skipped + report.tracks_missing;
    report.failures = owners
        .iter()
        .filter(|(key, _)| !stats.matched_keys.contains(key))
        .map(|(_, track)| ExportFailure {
            track_id: track.track.id.clone(),
            path: track.track.path.clone(),
            reason: "this file is not in the Traktor collection. Import the folder \
                     into Traktor first, then export again."
                .into(),
        })
        .collect();
    report
}

/// Re-parses what was written by running the rewrite again with no edits: that
/// proves the result is still a collection Keystone can read and that the entry
/// count survived. A file Traktor would reject is reported now, while the
/// backup is one click away.
fn verify(dest: &Path, before: &RewriteStats) -> Result<usize> {
    let text = read_collection(dest)?;
    let (_, after) = rewrite_document(&text, &HashMap::new(), false)?;
    if after.entries_seen != before.entries_seen {
        return Err(KeystoneError::Export(format!(
            "verification failed: the collection now holds {} entries, not {}. \
             Restore the backup from Settings.",
            after.entries_seen, before.entries_seen
        )));
    }
    Ok(before.entries_written)
}

/// The grid marker comes first because Traktor's own collections carry it
/// first, and a cue list that reads like Traktor's own is one less thing to
/// explain when something goes wrong.
fn entry_edit(track: &ExportTrack, settings: &Settings) -> EntryEdit {
    let mut cues: Vec<TraktorCue> = Vec::with_capacity(track.cues.len() + 1);
    if let Some(first) = track.beatgrid.first() {
        cues.push(TraktorCue {
            slot: None,
            name: AUTOGRID_NAME.to_string(),
            position_ms: first.position_ms,
            len_ms: 0.0,
            cue_type: cue_type::GRID,
        });
    }
    for cue in &track.cues {
        cues.push(TraktorCue {
            slot: Some(cue.slot),
            name: cue_label(cue),
            position_ms: cue.position_ms,
            // A loop is a cue with LEN > 0; everything else is a plain cue.
            len_ms: cue.end_ms.map(|e| (e - cue.position_ms).max(0.0)).unwrap_or(0.0),
            cue_type: cue_type::CUE,
        });
    }
    EntryEdit {
        cues,
        key_text: Some(track.key_string.clone()).filter(|k| !k.is_empty()),
        musical_key: musical_key_opt(track, settings),
        bpm: track.track.tempo_display.or(track.track.tempo_raw),
    }
}

/// Off by default. Traktor's Key column is reserved for its own analysis, Key
/// Text is the field the DJ apps read from the file tag, and the numbering this
/// would write is unresolved.
fn musical_key_opt(track: &ExportTrack, settings: &Settings) -> Option<u8> {
    if !settings.write_traktor_musical_key {
        return None;
    }
    let root = track.track.key_root?;
    let mode = track.track.key_mode?;
    Some(musical_key_value(root, mode))
}
