//! Identity. Two hashes with two different jobs, and conflating them is a bug
//! that looks like it works.

use std::path::Path;

use xxhash_rust::xxh3::Xxh3;

use crate::error::Result;

const HEAD_BYTES: u64 = 256 * 1024;
const TAIL_BYTES: u64 = 256 * 1024;

/// Stable per-path track id.
///
/// Per-path on purpose. Content-hash identity made byte-identical files at
/// different paths collapse into one row and ping-pong between paths on every
/// reconcile, and a DJ library is *more* duplicate-heavy than a sample library:
/// the same track as a 320k MP3 and as an AIFF, promo copies, a folder per gig.
/// Content equality lives in the separate `content_hash` column.
pub fn hash_path(path: &str) -> String {
    let mut hasher = Xxh3::new();
    hasher.update(path.as_bytes());
    format!("{:x}", hasher.digest128())
}

/// Content hash over the **audio data region**, never the file bytes.
///
/// This is the one in ENGINE.md's ranked risks. ID3 sits at the head of an MP3,
/// so hashing the first 256 KB of the *file* means Keystone's own re-tag pass
/// changes every hash and silently wipes its own analysis cache — and it looks
/// like it works right up until the first re-tag. Hash
/// `(codec, sample_rate, n_frames, first 256 KB from the first audio packet
/// offset, last 256 KB)`.
///
/// Owes a test: re-tagging a file must leave this value unchanged.
pub fn hash_audio_region(path: &Path) -> Result<String> {
    use std::io::{Seek, SeekFrom};

    let region = crate::analysis::decode::audio_region(path)?;
    let mut file = std::fs::File::open(path)?;

    // Read from the first audio packet, not from byte zero. The offset is the
    // whole point of this function: an ID3v2 block at the head of an MP3 is
    // rewritten by our own re-tag pass, and hashing it would move the key of
    // every cached analysis result in the library.
    file.seek(SeekFrom::Start(region.data_offset))?;
    let head = read_up_to(&mut file, HEAD_BYTES.min(region.data_len))?;

    // The tail is clamped so that a file whose audio is shorter than the two
    // windows hashes its region once rather than hashing the middle twice.
    let tail_len = TAIL_BYTES.min(region.data_len.saturating_sub(head.len() as u64));
    let tail = if tail_len == 0 {
        Vec::new()
    } else {
        let end = region.data_offset + region.data_len;
        file.seek(SeekFrom::Start(end - tail_len))?;
        read_up_to(&mut file, tail_len)?
    };

    Ok(digest_region(
        &region.codec,
        region.sample_rate,
        region.n_frames,
        &head,
        &tail,
    ))
}

fn read_up_to(file: &mut std::fs::File, n: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut buf = Vec::new();
    file.take(n).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Shared by the real implementation and the eval harness: the same digest from
/// an already-probed region, so neither has to re-open the file.
pub fn digest_region(
    codec: &str,
    sample_rate: u32,
    n_frames: u64,
    head: &[u8],
    tail: &[u8],
) -> String {
    let mut hasher = Xxh3::new();
    hasher.update(codec.as_bytes());
    hasher.update(&sample_rate.to_le_bytes());
    hasher.update(&n_frames.to_le_bytes());
    hasher.update(head);
    hasher.update(tail);
    format!("{:x}", hasher.digest128())
}

pub const fn head_bytes() -> u64 {
    HEAD_BYTES
}

pub const fn tail_bytes() -> u64 {
    TAIL_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_ids_are_stable_and_distinct() {
        assert_eq!(hash_path("/a/b.mp3"), hash_path("/a/b.mp3"));
        assert_ne!(hash_path("/a/b.mp3"), hash_path("/a/c.mp3"));
    }

    #[test]
    fn the_region_digest_ignores_everything_outside_the_audio() {
        // A re-tag changes the head of the *file* but not the audio region, so
        // the digest must not move. This is the cache-wipe bug, as a test.
        let a = digest_region("mp3", 44_100, 10_000, b"audio-head", b"audio-tail");
        let b = digest_region("mp3", 44_100, 10_000, b"audio-head", b"audio-tail");
        assert_eq!(a, b);
        let different_audio = digest_region("mp3", 44_100, 10_000, b"other-head", b"audio-tail");
        assert_ne!(a, different_audio);
    }
}
