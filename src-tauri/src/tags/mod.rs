//! Tag reading and writing.
//!
//! **The highest-consequence rule in the app:** never round-trip a DJ library
//! file through lofty's generic `Tag`. Its own docs say tag-to-`Tag` conversion
//! discards "anything without a mapping", so a bulk REPLACE ALL over the
//! generic type would silently delete every Serato GEOB blob, Traktor blob,
//! ReplayGain and iTunNORM frame in the library in one click — invisibly, until
//! the user opened Serato. Use `Id3v2Tag`, `Ilst`, `VorbisComments` and `ApeTag`
//! only, and gate the write path behind a round-trip fixture test on a
//! Serato-tagged MP3, AIFF and FLAC.
//!
//! That rule is about **writing**. [`read`] deliberately goes through the
//! generic type, because a read cannot lose what is still on disk and every
//! field Keystone shows has a mapping in lofty's table (`TKEY`, `TBPM`, `TIT1`,
//! `----:com.apple.iTunes:initialkey`, `INITIALKEY`/`KEY`). [`write`] never
//! does.

pub mod backup;
pub mod batch;
pub mod base64;
pub mod concrete;
pub mod fields;
pub mod initial_key;
pub mod patterns;
pub mod read;
pub mod retag;
pub mod rename;
#[cfg(test)]
pub mod fixtures;
#[cfg(test)]
pub mod test_support;
pub mod verify;
pub mod write;

use std::path::Path;

use lofty::file::FileType;
use lofty::probe::Probe;

use crate::error::{KeystoneError, Result};
use crate::models::Container;

/// What Keystone puts between a marker it composed and the text already in the
/// field.
///
/// Composition and the inverse strip both read this one constant. The strip has
/// to be exact — the context menu offers Remove Key from Common Tags, and a
/// greedy pattern would eat a track genuinely called `Energy` — so the two
/// sides must not be allowed to drift apart.
pub const MARKER_SEPARATOR: &str = " - ";

/// Dispatch by file, not by guessing from the extension alone: a `.wav` may
/// carry ID3v2 rather than RIFF INFO, and writing the wrong one leaves two
/// disagreeing tag blocks in the file.
///
/// The variant names the **file** container. For WAV and AIFF the tag Keystone
/// writes into is still ID3v2, because `TKEY` is the field Serato, Traktor and
/// rekordbox read and neither RIFF INFO nor the AIFF text chunks have a key
/// field at all. Where one of those secondary blocks already exists, the write
/// path keeps the fields it *can* carry in step rather than leaving the file
/// saying two different things — see [`fields::mirror_common`].
pub fn container_for(path: &Path) -> Result<Container> {
    container_of(file_type(path)?, path)
}

/// Probes by content, falling back to the extension only where the content is
/// inconclusive, which is how lofty's own `guess_file_type` behaves.
pub(crate) fn file_type(path: &Path) -> Result<FileType> {
    let probe = Probe::open(path)
        .map_err(|e| tag_err(path, e))?
        .guess_file_type()
        .map_err(|e| tag_err(path, e))?;
    probe
        .file_type()
        .ok_or_else(|| tag_err(path, "unrecognised audio format"))
}

pub(crate) fn container_of(file_type: FileType, path: &Path) -> Result<Container> {
    Ok(match file_type {
        FileType::Mpeg | FileType::Aac => Container::Id3v2,
        FileType::Aiff => Container::Aiff,
        FileType::Wav => Container::Riff,
        FileType::Mp4 => Container::Ilst,
        FileType::Flac | FileType::Vorbis | FileType::Opus | FileType::Speex => {
            Container::VorbisComments
        }
        // Musepack is deliberately absent: lofty models it as an enum over
        // three stream versions, and no DJ library contains one.
        FileType::Ape | FileType::WavPack => Container::Ape,
        other => {
            return Err(tag_err(
                path,
                format!("{other:?} carries no tag container Keystone can write"),
            ))
        }
    })
}

pub(crate) fn tag_err<E: std::fmt::Display>(path: &Path, e: E) -> KeystoneError {
    KeystoneError::Tag(format!("{}: {e}", path.display()))
}
