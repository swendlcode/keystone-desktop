//! Loading and saving the **concrete** tag types.
//!
//! Every write in Keystone comes through here, and the reason is the rule in
//! the module docs: lofty's generic `Tag` drops anything without a mapping, so
//! one bulk replace over it would delete every Serato GEOB blob, Traktor blob
//! and ReplayGain frame in the library invisibly. `Id3v2Tag`, `Ilst`,
//! `VorbisComments` and `ApeTag` keep the frames they do not understand, so a
//! load-mutate-save round trip through them preserves them.
//!
//! The secondary blocks matter for the same reason in reverse. An MP3 with an
//! ID3v1 tag, or a WAV with a RIFF INFO list, holds a second copy of the
//! artist and title; writing only ID3v2 leaves the file saying two different
//! things, and which one a given player believes is not predictable. They are
//! loaded here so the write path can keep them in step.

use std::fs::File;
use std::path::Path;

use lofty::aac::AacFile;
use lofty::ape::{ApeFile, ApeTag};
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::{AudioFile, FileType};
use lofty::flac::FlacFile;
use lofty::id3::v1::Id3v1Tag;
use lofty::id3::v2::{Id3v2Tag, Id3v2Version};
use lofty::iff::aiff::{AiffFile, AiffTextChunks};
use lofty::iff::wav::{RiffInfoList, WavFile};
use lofty::mp4::{Ilst, Mp4File};
use lofty::mpeg::MpegFile;
use lofty::ogg::tag::VorbisComments;
use lofty::ogg::{OpusFile, SpeexFile, VorbisFile};
use lofty::tag::TagExt;
use lofty::wavpack::WavPackFile;

use super::tag_err;
use crate::error::Result;
use crate::models::Container;

/// The tag Keystone writes key, tempo, energy and grouping into.
pub enum Primary {
    Id3v2(Box<Id3v2Tag>),
    Ilst(Box<Ilst>),
    Vorbis(Box<VorbisComments>),
    Ape(Box<ApeTag>),
}

/// A second tag block already in the file that carries some of the same common
/// fields. Never created by Keystone — only kept in step when it is there.
pub enum Side {
    Id3v1(Box<Id3v1Tag>),
    Riff(Box<RiffInfoList>),
    AiffText(Box<AiffTextChunks>),
}

pub struct Tags {
    pub container: Container,
    pub file_type: FileType,
    pub primary: Primary,
    pub sides: Vec<Side>,
    /// lofty *always* upgrades ID3v2.2 and 2.3 to 2.4 on write unless told
    /// otherwise, and a re-tagged v2.3 file is exactly the case `ENGINE.md` §6
    /// flags as untested in rekordbox and Traktor. Writing the version the file
    /// already had keeps a working library working.
    pub keep_id3v23: bool,
}

impl Tags {
    pub fn write_options(&self) -> WriteOptions {
        let mut opts = WriteOptions::default();
        if self.keep_id3v23 {
            opts = opts.use_id3v23(true);
        }
        opts
    }

    /// Writes every block that was loaded, primary first.
    ///
    /// `path` is the working copy, never the user's file: the caller copies,
    /// modifies the copy, verifies and only then renames over the original.
    pub fn save(&self, path: &Path) -> Result<()> {
        let opts = self.write_options();
        match &self.primary {
            Primary::Id3v2(t) => t.save_to_path(path, opts),
            Primary::Ilst(t) => t.save_to_path(path, opts),
            Primary::Vorbis(t) => t.save_to_path(path, opts),
            Primary::Ape(t) => t.save_to_path(path, opts),
        }
        .map_err(|e| tag_err(path, e))?;

        for side in &self.sides {
            match side {
                Side::Id3v1(t) => t.save_to_path(path, opts),
                Side::Riff(t) => t.save_to_path(path, opts),
                Side::AiffText(t) => t.save_to_path(path, opts),
            }
            .map_err(|e| tag_err(path, e))?;
        }
        Ok(())
    }
}

/// Audio properties are not needed for a tag edit and parsing them costs a scan
/// of the stream, so they are off. Tags are what we came for.
fn parse_options() -> ParseOptions {
    ParseOptions::new().read_properties(false)
}

pub fn load(path: &Path) -> Result<Tags> {
    let file_type = super::file_type(path)?;
    let container = super::container_of(file_type, path)?;
    let mut handle = File::open(path)?;
    let mut sides = Vec::new();

    let primary = match read_id3_family(file_type, &mut handle, path, &mut sides)? {
        Some(primary) => primary,
        None => match read_vorbis_family(file_type, &mut handle, path)? {
            Some(primary) => primary,
            None => read_remaining(file_type, &mut handle, path, &mut sides)?,
        },
    };

    let keep_id3v23 = matches!(&primary, Primary::Id3v2(t)
        if matches!(t.original_version(), Id3v2Version::V2 | Id3v2Version::V3));

    Ok(Tags { container, file_type, primary, sides, keep_id3v23 })
}

/// The containers whose primary tag is ID3v2: MP3, raw AAC, AIFF and WAV. The
/// last two also carry a second block of their own, which is loaded so the
/// write path can keep it in step.
fn read_id3_family(
    file_type: FileType,
    handle: &mut File,
    path: &Path,
    sides: &mut Vec<Side>,
) -> Result<Option<Primary>> {
    Ok(Some(match file_type {
        FileType::Mpeg => {
            let mut f = MpegFile::read_from(handle, parse_options()).map_err(err(path))?;
            push_id3v1(sides, f.remove_id3v1());
            id3v2_primary(f.remove_id3v2())
        }
        FileType::Aac => {
            let mut f = AacFile::read_from(handle, parse_options()).map_err(err(path))?;
            push_id3v1(sides, f.remove_id3v1());
            id3v2_primary(f.remove_id3v2())
        }
        FileType::Aiff => {
            let mut f = AiffFile::read_from(handle, parse_options()).map_err(err(path))?;
            if let Some(t) = f.remove_text_chunks() {
                sides.push(Side::AiffText(Box::new(t)));
            }
            id3v2_primary(f.remove_id3v2())
        }
        FileType::Wav => {
            let mut f = WavFile::read_from(handle, parse_options()).map_err(err(path))?;
            if let Some(t) = f.remove_riff_info() {
                sides.push(Side::Riff(Box::new(t)));
            }
            id3v2_primary(f.remove_id3v2())
        }
        _ => return Ok(None),
    }))
}

fn read_vorbis_family(
    file_type: FileType,
    handle: &mut File,
    path: &Path,
) -> Result<Option<Primary>> {
    Ok(Some(match file_type {
        FileType::Flac => {
            let mut f = FlacFile::read_from(handle, parse_options()).map_err(err(path))?;
            Primary::Vorbis(Box::new(f.remove_vorbis_comments().unwrap_or_default()))
        }
        // The Ogg families hold their comment packet unconditionally, so there
        // is nothing to take: it is cloned instead.
        FileType::Vorbis => {
            let f = VorbisFile::read_from(handle, parse_options()).map_err(err(path))?;
            Primary::Vorbis(Box::new(f.vorbis_comments().clone()))
        }
        FileType::Opus => {
            let f = OpusFile::read_from(handle, parse_options()).map_err(err(path))?;
            Primary::Vorbis(Box::new(f.vorbis_comments().clone()))
        }
        FileType::Speex => {
            let f = SpeexFile::read_from(handle, parse_options()).map_err(err(path))?;
            Primary::Vorbis(Box::new(f.vorbis_comments().clone()))
        }
        _ => return Ok(None),
    }))
}

fn read_remaining(
    file_type: FileType,
    handle: &mut File,
    path: &Path,
    sides: &mut Vec<Side>,
) -> Result<Primary> {
    Ok(match file_type {
        FileType::Mp4 => {
            let mut f = Mp4File::read_from(handle, parse_options()).map_err(err(path))?;
            Primary::Ilst(Box::new(f.remove_ilst().unwrap_or_default()))
        }
        FileType::Ape => {
            let mut f = ApeFile::read_from(handle, parse_options()).map_err(err(path))?;
            push_id3v1(sides, f.remove_id3v1());
            Primary::Ape(Box::new(f.remove_ape().unwrap_or_default()))
        }
        FileType::WavPack => {
            let mut f = WavPackFile::read_from(handle, parse_options()).map_err(err(path))?;
            push_id3v1(sides, f.remove_id3v1());
            Primary::Ape(Box::new(f.remove_ape().unwrap_or_default()))
        }
        other => return Err(tag_err(path, format!("{other:?} is not writable"))),
    })
}

fn id3v2_primary(tag: Option<Id3v2Tag>) -> Primary {
    Primary::Id3v2(Box::new(tag.unwrap_or_default()))
}

fn push_id3v1(sides: &mut Vec<Side>, tag: Option<Id3v1Tag>) {
    if let Some(t) = tag {
        sides.push(Side::Id3v1(Box::new(t)));
    }
}

/// Keeps the per-arm `map_err` down to one token, since every arm needs the
/// path in the message and lofty's own errors do not carry it.
fn err(path: &Path) -> impl Fn(lofty::error::FileParseError) -> crate::error::KeystoneError + '_ {
    move |e| tag_err(path, e)
}
