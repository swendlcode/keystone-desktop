//! Synthetic audio files for the tag-write tests.
//!
//! Hand-built rather than checked in as binaries: the round-trip gate has to
//! run in CI with no fixture library, and a few hundred bytes of header is
//! enough for lofty to parse and write every one of these containers.

use std::path::{Path, PathBuf};

use lofty::config::WriteOptions as LoftyWriteOptions;
use lofty::id3::v2::{BinaryFrame, Frame, FrameId, Id3v2Tag};
use lofty::tag::{Accessor, TagExt};

/// Stands in for `Serato Markers2`: a frame lofty keeps as opaque binary and
/// has no `ItemKey` for.
pub const GEOB_PAYLOAD: &[u8] = b"application/octet-stream\0Serato Markers2\0AQFBQUFB";

/// One MPEG-1 Layer III frame at 128 kbps, 44.1 kHz, stereo. 144 * 128000 /
/// 44100 truncates to 417 bytes, header included.
pub const FRAME_HEADER: [u8; 4] = [0xFF, 0xFB, 0x90, 0x00];
const FRAME_BYTES: usize = 417;

pub fn synthetic_mp3(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let mut audio = Vec::new();
    for _ in 0..40 {
        audio.extend_from_slice(&FRAME_HEADER);
        audio.resize(audio.len() + FRAME_BYTES - FRAME_HEADER.len(), 0);
    }
    std::fs::write(&path, &audio).expect("write the synthetic stream");

    let mut tag = Id3v2Tag::new();
    tag.set_artist("Swendl".to_string());
    tag.set_title("Midnight".to_string());
    tag.insert(Frame::Binary(BinaryFrame::new(
        FrameId::new("GEOB").unwrap(),
        GEOB_PAYLOAD.to_vec(),
    )));
    tag.save_to_path(&path, LoftyWriteOptions::default())
        .expect("seed the fixture tag");
    path
}

/// A FORM/AIFF with the two chunks lofty needs to parse it: `COMM` and `SSND`.
/// The sample rate is an 80-bit IEEE 754 extended float, which is why it is a
/// byte table rather than a calculation.
pub fn synthetic_aiff(dir: &Path) -> PathBuf {
    const RATE_44100: [u8; 10] = [0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0];
    let mut comm = Vec::new();
    comm.extend_from_slice(&2u16.to_be_bytes()); // channels
    comm.extend_from_slice(&44100u32.to_be_bytes()); // sample frames
    comm.extend_from_slice(&16u16.to_be_bytes()); // bits
    comm.extend_from_slice(&RATE_44100);

    let mut ssnd = vec![0u8; 8];
    ssnd.extend_from_slice(&[0u8; 400]);

    let mut body = b"AIFF".to_vec();
    for (id, data) in [(b"COMM", &comm), (b"SSND", &ssnd)] {
        body.extend_from_slice(id);
        body.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
    }

    let mut out = b"FORM".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);

    let path = dir.join("fixture.aiff");
    std::fs::write(&path, out).unwrap();
    path
}

/// `fLaC` plus a STREAMINFO block. No audio frames are needed: FLAC states its
/// own duration in STREAMINFO, so lofty reads the properties straight out.
pub fn synthetic_flac(dir: &Path) -> PathBuf {
    let packed: u64 = (44100u64 << 44) | (1u64 << 41) | (15u64 << 36) | 44100u64;
    let mut info = Vec::new();
    info.extend_from_slice(&4096u16.to_be_bytes()); // min blocksize
    info.extend_from_slice(&4096u16.to_be_bytes()); // max blocksize
    info.extend_from_slice(&[0; 6]); // min/max framesize, unknown
    info.extend_from_slice(&packed.to_be_bytes());
    info.extend_from_slice(&[0; 16]); // md5 of the unencoded audio

    let mut out = b"fLaC".to_vec();
    out.push(0x80); // last metadata block, type 0 = STREAMINFO
    out.extend_from_slice(&(info.len() as u32).to_be_bytes()[1..]);
    out.extend_from_slice(&info);

    let path = dir.join("fixture.flac");
    std::fs::write(&path, out).unwrap();
    path
}
