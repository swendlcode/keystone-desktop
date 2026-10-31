//! Writes that cannot leave a half-written library behind.
//!
//! Temp file in the **same** directory (a rename across filesystems is a copy,
//! and a copy is not atomic), fsync the file, fsync the directory on unix so
//! the rename itself is durable, then rename over the target.

use std::collections::HashMap;
use std::fs::{File, Permissions};
use std::io::Write;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;
use tempfile::NamedTempFile;

use crate::error::{KeystoneError, Result};

/// The prefix makes an interrupted write identifiable in the user's own folder
/// rather than looking like a file their DJ app should open.
const TEMP_PREFIX: &str = ".keystone-";

/// Never write in place.
///
/// Windows `std::fs::rename` fails when the destination exists, so the replace
/// goes through `tempfile`'s `persist`, which uses `MoveFileExW` with
/// `MOVEFILE_REPLACE_EXISTING` there and `rename` on unix.
pub fn atomic_write(dest: &Path, write: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    let dir = dest.parent().ok_or_else(|| {
        KeystoneError::Export(format!("{} has no parent directory", dest.display()))
    })?;
    std::fs::create_dir_all(dir)?;

    let mut tmp = NamedTempFile::with_prefix_in(TEMP_PREFIX, dir)?;
    write(tmp.as_file_mut())?;
    tmp.as_file_mut().flush()?;

    // A temp file is created 0600. Carrying the destination's own mode over
    // means a rewritten collection.nml is still readable by the DJ app that
    // owns it, instead of silently becoming private to this user.
    if let Some(perms) = existing_permissions(dest) {
        tmp.as_file().set_permissions(perms)?;
    }
    tmp.as_file().sync_all()?;
    tmp.persist(dest)
        .map_err(|e| KeystoneError::Export(format!("could not replace {}: {}", dest.display(), e)))?;
    fsync_dir(dir);
    Ok(())
}

/// For audio files: copy, modify the copy, verify by re-parsing, then atomically
/// rename. A crash mid-write on an MP3 whose ID3v2 block grew destroys **audio**,
/// not just metadata.
///
/// The original mtime is restored on the way out, or Serato, Traktor and
/// rekordbox re-analyse the user's entire library because Keystone re-tagged it
/// — and Keystone's own mtime fast path invalidates itself.
pub fn copy_modify_verify(
    path: &Path,
    modify: impl FnOnce(&Path) -> Result<()>,
    verify: impl FnOnce(&Path) -> Result<bool>,
) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| KeystoneError::Export(format!("{} has no parent", path.display())))?;
    let meta = std::fs::metadata(path)?;
    let mtime = filetime::FileTime::from_last_modification_time(&meta);
    let atime = filetime::FileTime::from_last_access_time(&meta);

    let tmp = NamedTempFile::with_prefix_in(TEMP_PREFIX, dir)?;
    std::fs::copy(path, tmp.path())?;
    modify(tmp.path())?;
    if !verify(tmp.path())? {
        return Err(KeystoneError::Export(format!(
            "{} did not read back correctly after the write; the original is untouched",
            path.display()
        )));
    }
    tmp.as_file().set_permissions(meta.permissions())?;
    tmp.as_file().sync_all()?;
    filetime::set_file_handle_times(tmp.as_file(), Some(atime), Some(mtime))?;
    tmp.persist(path)
        .map_err(|e| KeystoneError::Export(format!("could not replace {}: {}", path.display(), e)))?;
    fsync_dir(dir);
    Ok(())
}

fn existing_permissions(dest: &Path) -> Option<Permissions> {
    std::fs::metadata(dest).ok().map(|m| m.permissions())
}

/// Best effort by design: a directory that cannot be opened for fsync has
/// already had the rename succeed, and failing the export at that point would
/// report a loss that did not happen.
fn fsync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(f) = File::open(dir) {
        let _ = f.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Never write an element you did not fully parse: byte-copy every NML element
/// not modelled. This is the assertion that keeps that honest in tests.
///
/// Whitespace and the modelled elements are excluded, because splicing children
/// legitimately changes both. Everything else must survive as the same bytes,
/// the same number of times.
pub fn assert_preserved(before: &[u8], after: &[u8], modelled: &[&str]) -> Result<()> {
    let old = tag_spans(before, modelled)?;
    let new = tag_spans(after, modelled)?;
    for (raw, count) in &old {
        let kept = new.get(raw).copied().unwrap_or(0);
        if kept < *count {
            return Err(KeystoneError::Export(format!(
                "the rewrite dropped an element it does not model: {}",
                raw.chars().take(120).collect::<String>()
            )));
        }
    }
    Ok(())
}

/// Raw source text of every tag whose name is not modelled, counted. Raw rather
/// than parsed on purpose: the point of the check is that the bytes are
/// identical, not that an equivalent element exists.
fn tag_spans(xml: &[u8], modelled: &[&str]) -> Result<HashMap<String, usize>> {
    let text = std::str::from_utf8(xml)
        .map_err(|_| KeystoneError::Export("the collection file is not valid UTF-8".into()))?;
    let mut reader = Reader::from_str(text);
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut pos = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| KeystoneError::Export(format!("malformed XML: {e}")))?;
        let end = reader.buffer_position() as usize;
        let raw = &text[pos..end];
        pos = end;
        let name = match &event {
            Event::Start(e) | Event::Empty(e) => Some(e.name().as_ref().to_string()),
            Event::End(e) => Some(e.name().as_ref().to_string()),
            Event::Eof => break,
            _ => None,
        };
        if let Some(name) = name {
            if !modelled.contains(&name.as_str()) {
                *counts.entry(raw.to_string()).or_insert(0) += 1;
            }
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn a_failed_write_leaves_the_destination_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("collection.nml");
        std::fs::write(&dest, b"original").unwrap();

        let err = atomic_write(&dest, |f| {
            f.write_all(b"half")?;
            Err(KeystoneError::Export("exploded mid-write".into()))
        });
        assert!(err.is_err());

        let mut got = String::new();
        File::open(&dest).unwrap().read_to_string(&mut got).unwrap();
        assert_eq!(got, "original", "a failed write must not be visible");
        // And no temp file is left looking like a library file.
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(TEMP_PREFIX))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn a_successful_write_replaces_the_whole_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.xml");
        std::fs::write(&dest, b"a much longer original file").unwrap();
        atomic_write(&dest, |f| Ok(f.write_all(b"short")?)).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"short");
    }

    #[test]
    fn verification_failing_keeps_the_original_audio_file() {
        let dir = tempfile::tempdir().unwrap();
        let track = dir.path().join("track.mp3");
        std::fs::write(&track, b"audio").unwrap();
        let r = copy_modify_verify(
            &track,
            |tmp| Ok(std::fs::write(tmp, b"corrupted")?),
            |_| Ok(false),
        );
        assert!(r.is_err());
        assert_eq!(std::fs::read(&track).unwrap(), b"audio");
    }

    #[test]
    fn a_dropped_unmodelled_element_is_caught() {
        let before = br#"<NML><ENTRY><LOUDNESS PEAK_DB="-1.0"></LOUDNESS><CUE_V2 NAME="a"></CUE_V2></ENTRY></NML>"#;
        let after_ok = br#"<NML><ENTRY><LOUDNESS PEAK_DB="-1.0"></LOUDNESS><CUE_V2 NAME="b"></CUE_V2></ENTRY></NML>"#;
        let after_bad = br#"<NML><ENTRY><CUE_V2 NAME="b"></CUE_V2></ENTRY></NML>"#;
        assert_preserved(before, after_ok, &["CUE_V2"]).unwrap();
        let err = assert_preserved(before, after_bad, &["CUE_V2"]).unwrap_err();
        assert!(err.to_string().contains("LOUDNESS"), "{err}");
    }
}
