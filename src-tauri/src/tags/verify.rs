//! Verification for the write path: the audio must be untouched and the tag
//! must read back as asked.

use std::path::Path;

use lofty::config::ParseOptions;
use lofty::file::AudioFile;
use lofty::probe::Probe;

use super::tag_err;
use crate::error::Result;
use crate::models::{TagField, TagSnapshot};

/// The audio facts that must be identical on both sides of a tag write. The
/// **byte** offset of the first frame moves whenever the ID3v2 block changes
/// size, which is normal, so comparing offsets would reject every correct
/// write. What must not move is the stream itself.
#[derive(Debug, PartialEq)]
pub(crate) struct AudioFacts {
    duration_ms: u128,
    sample_rate: Option<u32>,
    channels: Option<u8>,
    bit_depth: Option<u8>,
}

pub(crate) fn audio_facts(path: &Path) -> Result<AudioFacts> {
    let tagged = Probe::open(path)
        .map_err(|e| tag_err(path, e))?
        .options(ParseOptions::new().read_cover_art(false))
        .guess_file_type()
        .map_err(|e| tag_err(path, e))?
        .read()
        .map_err(|e| tag_err(path, e))?;
    let p = tagged.properties();
    Ok(AudioFacts {
        duration_ms: p.duration().as_millis(),
        sample_rate: p.sample_rate(),
        channels: p.channels(),
        bit_depth: p.bit_depth(),
    })
}

/// A CBR MP3 with no Xing header has its duration estimated from the audio byte
/// span, so padding the tag can move it by a frame. One frame is ~26 ms; a
/// write that actually damaged the stream moves it by far more than 100.
const DURATION_TOLERANCE_MS: u128 = 100;

pub(crate) fn facts_agree(before: &AudioFacts, after: &AudioFacts) -> bool {
    before.sample_rate == after.sample_rate
        && before.channels == after.channels
        && before.bit_depth == after.bit_depth
        && before.duration_ms.abs_diff(after.duration_ms) <= DURATION_TOLERANCE_MS
}

pub(crate) fn matches_field(snapshot: &TagSnapshot, field: TagField, expected: Option<&str>) -> bool {
    let actual = snapshot_field(snapshot, field);
    match (actual.as_deref(), expected) {
        (None, None) => true,
        (None, Some(v)) | (Some(v), None) => v.trim().is_empty(),
        (Some(a), Some(b)) => match field {
            // Numbers are compared as numbers: `128` and `128.0` are the same
            // tempo, and a container that can only hold an integer is not a
            // failed write.
            TagField::Bpm => parse_pair(a, b),
            TagField::Year => a.trim().parse::<i32>().ok() == b.trim().parse::<i32>().ok(),
            _ => a.trim() == b.trim(),
        },
    }
}

fn parse_pair(a: &str, b: &str) -> bool {
    match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        (Ok(x), Ok(y)) => (x - y).abs() < 1.0,
        _ => a.trim() == b.trim(),
    }
}

pub(crate) fn snapshot_field(snapshot: &TagSnapshot, field: TagField) -> Option<String> {
    match field {
        TagField::Artist => snapshot.artist.clone(),
        TagField::Title => snapshot.title.clone(),
        TagField::Album => snapshot.album.clone(),
        TagField::Genre => snapshot.genre.clone(),
        TagField::Grouping => snapshot.grouping.clone(),
        TagField::Comment => snapshot.comment.clone(),
        TagField::Year => snapshot.year.map(|y| y.to_string()),
        TagField::Bpm => snapshot.bpm.map(|b| format!("{b}")),
        TagField::InitialKey => snapshot.initial_key.clone(),
    }
}
