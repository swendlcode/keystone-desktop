//! The hard gate on the write path.
//!
//! `ENGINE.md` risk 4: "a careless bulk tag write destroys every DJ tag in the
//! library", because lofty's generic `Tag` drops anything without a mapping.
//! The fixture below carries a `GEOB` frame standing in for a Serato
//! `Markers2` blob — the exact thing that would vanish — and the test fails if
//! it is not still there, byte for byte, after a write.

use std::path::Path;

use lofty::config::WriteOptions as LoftyWriteOptions;
use lofty::id3::v2::{BinaryFrame, Frame, FrameId, Id3v2Tag};
use lofty::tag::{Accessor, TagExt};

use super::*;
use crate::models::{TagField, Container};
use crate::tags::concrete::{self, Primary};
use crate::tags::fixtures::{
    synthetic_aiff, synthetic_flac, synthetic_mp3, FRAME_HEADER, GEOB_PAYLOAD,
};

fn geob(path: &Path) -> Option<Vec<u8>> {
    let tags = concrete::load(path).expect("load the concrete tag");
    let Primary::Id3v2(id3) = tags.primary else {
        panic!("an mp3 must load as ID3v2");
    };
    id3.into_iter().find_map(|frame| match frame {
        Frame::Binary(b) if b.id().as_str() == "GEOB" => Some(b.data.to_vec()),
        _ => None,
    })
}

/// Everything from the first MPEG sync word on. A tag write is allowed to move
/// this; it is not allowed to change it.
fn audio_region(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).expect("read back");
    let at = bytes
        .windows(FRAME_HEADER.len())
        .position(|w| w == FRAME_HEADER)
        .expect("the synthetic stream is still in there");
    bytes[at..].to_vec()
}

#[test]
fn a_serato_blob_survives_a_write_through_the_concrete_tag() {
    let dir = tempfile::tempdir().unwrap();
    let path = synthetic_mp3(dir.path(), "fixture.mp3");
    assert_eq!(geob(&path).as_deref(), Some(GEOB_PAYLOAD), "the fixture is wrong");

    let edit = TagEdit::default()
        .set(TagField::InitialKey, "Am")
        .set(TagField::Grouping, "7")
        .set(TagField::Bpm, "128");
    let report = write(&path, &edit, &WriteOptions::default()).expect("the write");

    assert!(report.verified, "the write reported itself unverified");
    assert_eq!(
        geob(&path).as_deref(),
        Some(GEOB_PAYLOAD),
        "the GEOB frame was lost — this is the bug the whole module exists to prevent"
    );

    let snapshot = crate::tags::read::read(&path).unwrap();
    assert_eq!(snapshot.initial_key.as_deref(), Some("Am"));
    assert_eq!(snapshot.grouping.as_deref(), Some("7"));
    assert_eq!(snapshot.bpm, Some(128.0));
    // The fields the edit did not mention are left alone.
    assert_eq!(snapshot.artist.as_deref(), Some("Swendl"));
}

#[test]
fn the_audio_is_untouched_and_the_mtime_is_put_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = synthetic_mp3(dir.path(), "fixture.mp3");

    let before_audio = audio_region(&path);
    let before_mtime = filetime::FileTime::from_last_modification_time(
        &std::fs::metadata(&path).unwrap(),
    );

    let edit = TagEdit::default().set(TagField::InitialKey, "F#m");
    write(&path, &edit, &WriteOptions::default()).expect("the write");

    assert_eq!(audio_region(&path), before_audio, "the audio stream changed");
    let after_mtime = filetime::FileTime::from_last_modification_time(
        &std::fs::metadata(&path).unwrap(),
    );
    // Without this, Serato, Traktor and rekordbox re-analyse the whole library
    // because Keystone re-tagged it.
    assert_eq!(after_mtime, before_mtime, "the mtime was not restored");
}

#[test]
fn the_replaced_tag_is_captured_so_a_backup_row_can_be_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = synthetic_mp3(dir.path(), "fixture.mp3");

    let edit = TagEdit::default().set(TagField::InitialKey, "Am");
    let report = write(&path, &edit, &WriteOptions::default()).unwrap();
    let blob = report.replaced_tag.expect("backup was asked for");
    assert!(blob.len() > 9, "the envelope is empty");

    // The pre-write values are in there, which is what restore applies.
    let text = String::from_utf8_lossy(&blob);
    assert!(text.contains("Swendl"), "the snapshot is missing the artist");

    let off = WriteOptions { backup: false, ..WriteOptions::default() };
    let report = write(&path, &edit, &off).unwrap();
    assert!(report.replaced_tag.is_none(), "backup was not asked for");
}

#[test]
fn clearing_the_key_takes_it_off_and_leaves_everything_else() {
    let dir = tempfile::tempdir().unwrap();
    let path = synthetic_mp3(dir.path(), "fixture.mp3");

    write(
        &path,
        &TagEdit::default().set(TagField::InitialKey, "Am"),
        &WriteOptions::default(),
    )
    .unwrap();
    crate::tags::initial_key::clear(&path, Container::Id3v2).expect("clear the key");

    let snapshot = crate::tags::read::read(&path).unwrap();
    assert_eq!(snapshot.initial_key, None);
    assert_eq!(snapshot.artist.as_deref(), Some("Swendl"));
    assert_eq!(geob(&path).as_deref(), Some(GEOB_PAYLOAD));
}

#[test]
fn an_id3v23_file_is_written_back_as_v23() {
    let dir = tempfile::tempdir().unwrap();
    let path = synthetic_mp3(dir.path(), "fixture.mp3");

    let mut tag = Id3v2Tag::new();
    tag.set_artist("Swendl".to_string());
    tag.save_to_path(&path, LoftyWriteOptions::default().use_id3v23(true))
        .unwrap();
    assert_eq!(&std::fs::read(&path).unwrap()[..4], b"ID3\x03", "the fixture is not v2.3");

    write(
        &path,
        &TagEdit::default().set(TagField::InitialKey, "Am"),
        &WriteOptions::default(),
    )
    .unwrap();

    // lofty upgrades to 2.4 unless told otherwise, and a silently upgraded tag
    // in rekordbox and Traktor is the untested case `ENGINE.md` flags.
    assert_eq!(&std::fs::read(&path).unwrap()[..4], b"ID3\x03", "it was upgraded to 2.4");
}

#[test]
fn an_aiff_keeps_its_geob_and_its_text_chunks_do_not_disagree() {
    use lofty::iff::aiff::AiffTextChunks;

    let dir = tempfile::tempdir().unwrap();
    let path = synthetic_aiff(dir.path());

    let mut id3 = Id3v2Tag::new();
    id3.set_artist("Swendl".to_string());
    id3.insert(Frame::Binary(BinaryFrame::new(
        FrameId::new("GEOB").unwrap(),
        GEOB_PAYLOAD.to_vec(),
    )));
    id3.save_to_path(&path, LoftyWriteOptions::default()).unwrap();
    AiffTextChunks {
        name: Some("Old Title".to_string()),
        author: None,
        copyright: None,
        annotations: None,
        comments: None,
    }
    .save_to_path(&path, LoftyWriteOptions::default())
    .unwrap();

    let edit = TagEdit::default()
        .set(TagField::Title, "Midnight")
        .set(TagField::InitialKey, "Am");
    write(&path, &edit, &WriteOptions::default()).expect("the write");

    assert_eq!(geob(&path).as_deref(), Some(GEOB_PAYLOAD), "the GEOB frame was lost");

    let snapshot = crate::tags::read::read(&path).unwrap();
    assert_eq!(snapshot.title.as_deref(), Some("Midnight"));
    assert_eq!(snapshot.initial_key.as_deref(), Some("Am"));

    // The second block carried the old title. Leaving it behind means the file
    // says two different things and which one a player believes is a coin toss.
    let tags = concrete::load(&path).unwrap();
    let side = tags
        .sides
        .iter()
        .find_map(|s| match s {
            crate::tags::concrete::Side::AiffText(t) => Some(t.name.clone()),
            _ => None,
        })
        .expect("the AIFF text chunk is still there");
    assert_eq!(side.as_deref(), Some("Midnight"), "the text chunk was left stale");
}

#[test]
fn a_flac_keeps_unmapped_vorbis_keys_and_gets_both_key_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = synthetic_flac(dir.path());

    let mut comments = lofty::ogg::tag::VorbisComments::new();
    comments.insert("ARTIST".to_string(), "Swendl".to_string());
    // Nothing in lofty's `ItemKey` table maps this, which is exactly why a
    // generic round trip would drop it.
    comments.insert("SERATO_MARKERS_V2".to_string(), "AQFBQUFB".to_string());
    comments
        .save_to_path(&path, LoftyWriteOptions::default())
        .unwrap();

    let edit = TagEdit::default().set(TagField::InitialKey, "Am");
    write(&path, &edit, &WriteOptions::default()).expect("the write");

    let tags = concrete::load(&path).unwrap();
    let Primary::Vorbis(comments) = tags.primary else {
        panic!("a flac must load as vorbis comments");
    };
    assert_eq!(comments.get("SERATO_MARKERS_V2"), Some("AQFBQUFB"));
    assert_eq!(comments.get("ARTIST"), Some("Swendl"));
    // FLAC and Ogg have no single convention for the key, so both go in.
    assert_eq!(comments.get("INITIALKEY"), Some("Am"));
    assert_eq!(comments.get("KEY"), Some("Am"));
}

#[test]
fn a_write_that_fails_verification_leaves_the_users_file_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = synthetic_mp3(dir.path(), "fixture.mp3");
    let before = std::fs::read(&path).unwrap();

    // A year that is not a year: the container cannot hold it, so the tag
    // cannot read back as asked and the write has to refuse.
    let edit = TagEdit::default().set(TagField::Year, "not a year");
    assert!(write(&path, &edit, &WriteOptions::default()).is_err());

    // The check runs on the copy, before the rename. A check that ran after
    // could only report that the file is already wrong.
    assert_eq!(std::fs::read(&path).unwrap(), before, "the original was replaced");
    let leftovers: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".keystone-"))
        .collect();
    assert!(leftovers.is_empty(), "a temp file was left behind: {leftovers:?}");
}
