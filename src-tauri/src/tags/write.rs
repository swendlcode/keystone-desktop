use std::fs::File;
use std::path::Path;

use lofty::picture::{MimeType, Picture, PictureType};

use super::concrete::{self, Primary, Tags};
use super::verify::{audio_facts, facts_agree, matches_field, AudioFacts};
use super::{fields, tag_err};
use crate::error::{KeystoneError, Result};
use crate::models::{Container, CoverArt, TagEdit};

#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Restore the original mtime after the write.
    ///
    /// Not politeness. Without it, Serato, Traktor and rekordbox all re-analyse
    /// the user's entire library because Keystone re-tagged it, and Keystone's
    /// own mtime fast path invalidates itself at the same time.
    pub restore_mtime: bool,
    /// Store the replaced tag bytes in `tag_backup` first. This is what makes
    /// RE-TAG EXISTING FILES a shippable button rather than a terrifying one.
    ///
    /// [`write`] has no database handle, so it captures the bytes into
    /// [`WriteReport::replaced_tag`]; [`write_with_backup`] is the entry point
    /// that persists them, and it is the one the commands call.
    pub backup: bool,
    /// Re-parse after writing and confirm the tag reads back and that the audio
    /// stream was not disturbed.
    pub verify: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self { restore_mtime: true, backup: true, verify: true }
    }
}

#[derive(Debug, Clone)]
pub struct WriteReport {
    pub container: Container,
    pub verified: bool,
    pub backup_id: Option<String>,
    /// The tag block as it was before the write, when `backup` was asked for.
    pub replaced_tag: Option<Vec<u8>>,
}

/// Copy, modify the copy, verify, then atomically rename over the original.
///
/// Never in place. A crash mid-write on an MP3 whose ID3v2 block grew destroys
/// **audio**, not just metadata, and these are irreplaceable libraries. On
/// Windows `std::fs::rename` fails when the destination exists, so the replace
/// goes through `tempfile`'s `persist`, which uses `MoveFileEx` with
/// `MOVEFILE_REPLACE_EXISTING`.
///
/// lofty would otherwise upgrade an ID3v2.2 or 2.3 tag to 2.4 here;
/// [`Tags::write_options`] writes the version the file already had, because a
/// re-tagged v2.3 file in rekordbox and Traktor is exactly the untested case.
pub fn write(path: &Path, edit: &TagEdit, opts: &WriteOptions) -> Result<WriteReport> {
    apply(path, opts, Some(edit), |tags| {
        for (field, value) in &edit.fields {
            fields::set(&mut tags.primary, *field, value.as_deref())?;
            for side in &mut tags.sides {
                fields::mirror_common(side, *field, value.as_deref());
            }
        }
        Ok(())
    })
}

/// [`write`], plus the `tag_backup` row. The commands call this one: the backup
/// is the difference between RE-TAG EXISTING FILES being a button and a dare.
///
/// The row is written **before** the file. It costs a second tag parse, and it
/// is worth it: a row that fails to insert after a successful write leaves the
/// user with a changed file and no way back.
pub fn write_with_backup(
    db: &std::sync::Arc<crate::db::DatabasePool>,
    track_id: &str,
    path: &Path,
    edit: &TagEdit,
    opts: &WriteOptions,
) -> Result<WriteReport> {
    let backup_id = if opts.backup {
        let tags = concrete::load(path)?;
        let blob = super::backup::capture(path, &tags)?;
        Some(super::backup::save(db, track_id, tags.container, &blob)?)
    } else {
        None
    };
    // The row already holds the replaced tag, so the write does not capture it
    // a second time.
    let inner = WriteOptions { backup: false, ..opts.clone() };
    let mut report = write(path, edit, &inner)?;
    report.backup_id = backup_id;
    Ok(report)
}

pub fn set_cover_art(
    path: &Path,
    art: Option<&CoverArt>,
    opts: &WriteOptions,
) -> Result<WriteReport> {
    let picture = art.map(decode_picture).transpose()?;
    apply(path, opts, None, move |tags| {
        set_picture(&mut tags.primary, picture)
    })
}

/// Re-parses and confirms the write landed without touching the audio.
///
/// It compares the fields the edit asked for, not the whole tag: an unrelated
/// frame differing is not this write's business, and demanding byte equality
/// would fail on every file lofty re-pads.
pub fn verify(path: &Path, expected: &TagEdit) -> Result<bool> {
    let snapshot = super::read::read(path)?;
    Ok(expected
        .fields
        .iter()
        .all(|(field, value)| matches_field(&snapshot, *field, value.as_deref())))
}

/// Copy, modify the copy, verify **the copy**, then rename.
///
/// Verifying before the rename is the whole point of the order: a check that
/// runs after the replace can only tell the user their file is already wrong.
fn apply<F>(
    path: &Path,
    opts: &WriteOptions,
    expect: Option<&TagEdit>,
    mutate: F,
) -> Result<WriteReport>
where
    F: FnOnce(&mut Tags) -> Result<()>,
{
    let before = audio_facts(path)?;
    let original_times = std::fs::metadata(path)?;
    let mut tags = concrete::load(path)?;
    let replaced_tag = if opts.backup {
        Some(super::backup::capture(path, &tags)?)
    } else {
        None
    };
    mutate(&mut tags)?;

    let dir = path
        .parent()
        .ok_or_else(|| tag_err(path, "has no parent directory"))?;
    let temp = stage_copy(path, dir, &tags)?;

    if opts.verify {
        check_copy(path, temp.path(), &before, expect)?;
    }

    File::open(temp.path())?.sync_all()?;
    temp.persist(path)
        .map_err(|e| KeystoneError::Io(e.error))?;
    sync_dir(dir);

    if opts.restore_mtime {
        let _ = filetime::set_file_times(
            path,
            filetime::FileTime::from_last_access_time(&original_times),
            filetime::FileTime::from_last_modification_time(&original_times),
        );
    }

    Ok(WriteReport {
        container: tags.container,
        // The audio stream — and, when an edit was handed in, the tag itself —
        // was checked on the copy before the rename.
        verified: opts.verify,
        backup_id: None,
        replaced_tag,
    })
}

/// The user's file, copied beside itself with the new tags written into the copy.
///
/// Two things here are not interchangeable with the obvious alternatives. The
/// temp file is created *in the same directory*, because the final rename is
/// only atomic within one filesystem. And the bytes get there by `copy` rather
/// than by writing a fresh file, because `copy` carries the permission bits —
/// otherwise the user's track comes back as 0600 after a re-tag.
fn stage_copy(path: &Path, dir: &Path, tags: &Tags) -> Result<tempfile::NamedTempFile> {
    let temp = tempfile::Builder::new()
        .prefix(".keystone-")
        .tempfile_in(dir)?;
    std::fs::copy(path, temp.path())?;
    tags.save(temp.path())?;
    Ok(temp)
}

/// Everything that must hold before the copy is allowed to replace the original.
///
/// Takes `original` only to name it in the error: every read here is of the
/// copy, which is the point — a check that runs after the replace can only tell
/// the user their file is already wrong.
fn check_copy(
    original: &Path,
    copy: &Path,
    before: &AudioFacts,
    expect: Option<&TagEdit>,
) -> Result<()> {
    let after = audio_facts(copy)?;
    if !facts_agree(before, &after) {
        return Err(tag_err(
            original,
            format!("the write moved the audio stream: {before:?} -> {after:?}"),
        ));
    }
    if let Some(edit) = expect {
        if !verify(copy, edit)? {
            return Err(tag_err(original, "the tag did not read back as written"));
        }
    }
    Ok(())
}

/// On unix the rename is only durable once the directory entry is flushed. Best
/// effort: a filesystem that refuses to fsync a directory is not a reason to
/// fail a write that already landed.
fn sync_dir(dir: &Path) {
    if cfg!(unix) {
        if let Ok(handle) = File::open(dir) {
            let _ = handle.sync_all();
        }
    }
}

fn decode_picture(art: &CoverArt) -> Result<Picture> {
    let data = super::base64::decode(&art.data_base64)
        .map_err(|e| KeystoneError::Tag(format!("cover art: {e}")))?;
    Ok(Picture::unchecked(data)
        .pic_type(PictureType::CoverFront)
        .mime_type(MimeType::from_str(&art.mime))
        .build())
}

fn set_picture(primary: &mut Primary, picture: Option<Picture>) -> Result<()> {
    match primary {
        Primary::Id3v2(t) => {
            t.remove_picture_type(PictureType::CoverFront);
            if let Some(p) = picture {
                t.insert_picture(p);
            }
        }
        Primary::Ilst(t) => {
            t.remove_pictures();
            if let Some(p) = picture {
                t.insert_picture(p);
            }
        }
        Primary::Vorbis(t) => {
            use lofty::ogg::OggPictureStorage;
            let _ = t.remove_pictures();
            if let Some(p) = picture {
                // Vorbis pictures carry width, depth and colour count in the
                // block itself, so lofty has to read the image to write one.
                // A file that is not a readable PNG or JPEG is a refusal, not
                // something to swallow.
                t.insert_picture(p, None)
                    .map_err(|e| KeystoneError::Tag(format!("cover art: {e}")))?;
            }
        }
        // APE carries pictures as binary items with no settled convention, and
        // no DJ app reads them. Leaving them alone beats writing something a
        // player will show as a corrupt image.
        Primary::Ape(_) => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
