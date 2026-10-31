use std::path::Path;

use lofty::config::ParseOptions;
use lofty::file::{TaggedFile, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, Tag};

use super::tag_err;
use crate::error::Result;
use crate::models::{CoverArt, TagSnapshot};

/// Tag reads invalidate on **mtime**, unlike analysis which invalidates on
/// content hash plus version. A tag edit changes mtime but not audio, and must
/// not cost a re-analysis.
///
/// This is the one place Keystone touches lofty's generic `Tag`, and it is safe
/// because a read cannot lose what stays on disk. Every field below has a
/// mapping in lofty's own table — `TKEY`, `TBPM`/`tmpo`, `TIT1`/`©grp`/
/// `GROUPING`, `----:com.apple.iTunes:initialkey`, `INITIALKEY`/`KEY` — so
/// there is nothing to reimplement per container. The **write** path never does
/// this: see [`super::write`].
pub fn read(path: &Path) -> Result<TagSnapshot> {
    let tagged = open(path, true)?;
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        // An untagged file is normal, not an error: the table still shows it by
        // filename and the analysis still runs.
        return Ok(TagSnapshot::default());
    };

    Ok(TagSnapshot {
        artist: owned(tag.artist().as_deref()),
        title: owned(tag.title().as_deref()),
        album: owned(tag.album().as_deref()),
        genre: owned(tag.genre().as_deref()),
        grouping: grouping(tag),
        comment: owned(tag.comment().as_deref()),
        year: year(tag),
        bpm: bpm(tag),
        initial_key: owned(tag.get_string(ItemKey::InitialKey)),
        has_cover_art: tag.picture_count() > 0,
    })
}

/// Separate from [`read`] because the table shows a cover for every visible row
/// and the snapshot is read for one track at a time.
pub fn read_cover_art(path: &Path) -> Result<Option<CoverArt>> {
    let tagged = open(path, true)?;
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return Ok(None);
    };
    Ok(front_cover(tag).map(|p| CoverArt {
        mime: p.mime_type().map_or_else(
            || "application/octet-stream".to_string(),
            |m| m.as_str().to_string(),
        ),
        data_base64: super::base64::encode(p.data()),
    }))
}

/// Just the flag, for the walk. Decoding every embedded image during a
/// 20,000-track scan is not worth a column.
pub fn has_cover_art(path: &Path) -> Result<bool> {
    let tagged = open(path, false)?;
    Ok(tagged.tags().iter().any(|t| t.picture_count() > 0))
}

pub fn modified_unix(path: &Path) -> Result<i64> {
    let modified = std::fs::metadata(path)?.modified()?;
    Ok(modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64))
}

/// With `read_cover_art(false)` lofty keeps the picture frame and drops only the
/// image bytes, so the count is still right. That is what makes
/// [`has_cover_art`] cheap enough to run during the walk.
fn open(path: &Path, with_cover_art: bool) -> Result<TaggedFile> {
    let options = ParseOptions::new()
        .read_properties(false)
        .read_cover_art(with_cover_art);
    Probe::open(path)
        .map_err(|e| tag_err(path, e))?
        .options(options)
        .guess_file_type()
        .map_err(|e| tag_err(path, e))?
        .read()
        .map_err(|e| tag_err(path, e))
}

/// Prefers `TIT1`, ID3v2's own content group description, and falls back to
/// Apple's `GRP1`. Files written by iTunes carry only the second one.
fn grouping(tag: &Tag) -> Option<String> {
    owned(tag.get_string(ItemKey::ContentGroup))
        .or_else(|| owned(tag.get_string(ItemKey::AppleId3v2ContentGroup)))
}

fn year(tag: &Tag) -> Option<i32> {
    tag.get_string(ItemKey::Year)
        .or_else(|| tag.get_string(ItemKey::RecordingDate))
        .and_then(|v| v.get(..4).unwrap_or(v).trim().parse::<i32>().ok())
}

/// Decimal first, then the integer frame. ID3v2 has no decimal BPM frame at
/// all, so an MP3 can only ever answer with the integer — which is why the
/// displayed decimal tempo comes from the database and not from here.
fn bpm(tag: &Tag) -> Option<f64> {
    tag.get_string(ItemKey::Bpm)
        .or_else(|| tag.get_string(ItemKey::IntegerBpm))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// The front cover, or the first picture when nothing is labelled as one —
/// plenty of libraries carry a single untyped `APIC`.
fn front_cover(tag: &Tag) -> Option<&Picture> {
    tag.get_picture_type(PictureType::CoverFront)
        .or_else(|| tag.pictures().first())
}

/// Empty strings are the same as absent everywhere in the UI, and a tag full of
/// `""` would otherwise make a row look populated.
fn owned(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}
