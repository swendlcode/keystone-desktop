//! Opening a container, the header-only facts, and where the audio sits in the
//! file.
//!
//! Everything here reads headers. No frame is decoded on this path, because
//! [`super::probe`] runs during the walk for every file in the library and
//! `audio_region` runs again for every content hash.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use symphonia::core::codecs::{CodecParameters, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, ReadBytes};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::{AudioProbe, AudioRegion};
use crate::error::{KeystoneError, Result};

/// Everything established before a single audio frame is decoded.
pub struct Opened {
    pub reader: Box<dyn FormatReader>,
    pub track_id: u32,
    pub params: CodecParameters,
    pub file_len: u64,
}

/// A container that gives no packet for its own audio track inside this many
/// packets is broken, not slow. The bound exists so a malformed file cannot
/// turn a probe into a full-file scan.
const MAX_PACKET_PROBES: usize = 64;

pub fn fail(path: &Path, what: &str) -> KeystoneError {
    KeystoneError::Decode(format!("{}: {what}", path.display()))
}

pub fn open(path: &Path) -> Result<Opened> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len();
    // A zero-byte file otherwise fails three layers down with a message about
    // container probing. Name it here: an interrupted download and a
    // half-copied file are both ordinary in a DJ library.
    if file_len == 0 {
        return Err(fail(path, "the file is empty"));
    }

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // The extension is a hint and never a decision: probing sniffs the
    // container, so a WAV named `.mp3` by a ripper nobody reconfigured opens.
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(&ext.to_lowercase());
    }

    // Gapless: MP3 and AAC carry encoder delay and padding. Counting those
    // frames puts every duration tens of milliseconds long and shifts the beat
    // grid against the file the DJ loaded in their own player.
    let fmt_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &MetadataOptions::default())
        .map_err(|e| fail(path, &format!("unsupported or damaged container ({e})")))?;

    let reader = probed.format;
    let (track_id, params) = {
        let track = reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL && t.codec_params.sample_rate.is_some())
            .ok_or_else(|| fail(path, "no audio track"))?;
        (track.id, track.codec_params.clone())
    };

    Ok(Opened {
        reader,
        track_id,
        params,
        file_len,
    })
}

/// The header-only facts, from an already-opened container.
pub fn describe(path: &Path, opened: &Opened) -> Result<AudioProbe> {
    let p = &opened.params;
    let sample_rate = p
        .sample_rate
        .ok_or_else(|| fail(path, "the track declares no sample rate"))?;
    if sample_rate == 0 {
        return Err(fail(path, "the track declares a zero sample rate"));
    }

    let channels = header_channels(p).unwrap_or(1);
    let n_frames = p.n_frames;
    let duration_ms = n_frames.map(|n| (n as i64 * 1000) / sample_rate as i64);

    Ok(AudioProbe {
        codec: codec_name(p),
        sample_rate,
        channels,
        // Lossy codecs have no bit depth and must report none rather than a
        // plausible-looking 16: the field is shown to the user as a fact.
        bit_depth: p
            .bits_per_sample
            .or(p.bits_per_coded_sample)
            .map(|b| b as u16),
        bitrate_kbps: bitrate_kbps(opened.file_len, duration_ms),
        duration_ms,
        n_frames,
    })
}

/// What the container says about the channel count, which for MP4 is nothing.
fn header_channels(params: &CodecParameters) -> Option<u16> {
    params
        .channels
        .or_else(|| params.channel_layout.map(|l| l.into_channels()))
        .map(|c| c.count().max(1) as u16)
}

/// The header-only facts, with the channel count repaired when the container
/// did not carry one.
///
/// MP4 keeps the channel configuration inside the codec's own setup data, so
/// AAC and ALAC tracks declare none and the fallback would call every one of
/// them mono — a wrong fact, in the library table, about half the lossless
/// files a DJ owns. Decoding a single packet is the cheapest truthful answer
/// and costs a millisecond on the walk, and only for those containers.
pub fn probe(path: &Path) -> Result<AudioProbe> {
    let opened = open(path)?;
    let mut facts = describe(path, &opened)?;
    if header_channels(&opened.params).is_none() {
        if let Some(channels) = channels_from_first_frame(opened) {
            facts.channels = channels;
        }
    }
    Ok(facts)
}

fn channels_from_first_frame(opened: Opened) -> Option<u16> {
    let Opened {
        mut reader,
        track_id,
        params,
        ..
    } = opened;
    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions { verify: false })
        .ok()?;
    for _ in 0..MAX_PACKET_PROBES {
        let packet = reader.next_packet().ok()?;
        if packet.track_id() != track_id {
            continue;
        }
        if let Ok(audio) = decoder.decode(&packet) {
            return Some(audio.spec().channels.count().max(1) as u16);
        }
    }
    None
}

/// The codec's short name, which is also hashed into the content hash — so it
/// has to be the registry's stable string, not a `Debug` rendering.
fn codec_name(params: &CodecParameters) -> String {
    symphonia::default::get_codecs()
        .get_codec(params.codec)
        .map(|d| d.short_name.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Bits per millisecond is kilobits per second, so this is the whole formula.
///
/// Measured over the file rather than the audio region: for a VBR MP3 there is
/// no single frame bitrate to report, and the average is what every other tool
/// shows for one.
fn bitrate_kbps(file_len: u64, duration_ms: Option<i64>) -> Option<u32> {
    let ms = duration_ms.filter(|ms| *ms > 0)? as u64;
    let kbps = file_len.saturating_mul(8) / ms;
    (kbps > 0).then_some(kbps as u32)
}

pub fn audio_region(path: &Path) -> Result<AudioRegion> {
    let opened = open(path)?;
    let probe = describe(path, &opened)?;
    let Opened {
        mut reader,
        track_id,
        params,
        file_len,
    } = opened;

    // A `Packet` does not carry its byte offset, so read one packet and ask the
    // stream where it now is. symphonia's own MP3 demuxer computes a frame's
    // position exactly this way, and the arithmetic is sound wherever the
    // packet payload is the container's audio payload. For Ogg it lands a page
    // header early, which is still inside the first audio page and still moves
    // with the audio — which is all the content hash asks of it.
    let mut payload = None;
    for _ in 0..MAX_PACKET_PROBES {
        match reader.next_packet() {
            Ok(p) if p.track_id() == track_id => {
                payload = Some(p.data.len() as u64);
                break;
            }
            Ok(_) => continue,
            Err(e) => return Err(fail(path, &format!("no audio packet ({e})"))),
        }
    }
    let payload = payload.ok_or_else(|| fail(path, "no audio packet for the chosen track"))?;

    let pos = reader.into_inner().pos();
    let data_offset = pos.saturating_sub(payload).min(file_len);
    let data_len = region_len(path, &probe, &params, data_offset, file_len);

    Ok(AudioRegion {
        codec: probe.codec,
        sample_rate: probe.sample_rate,
        n_frames: probe.n_frames.unwrap_or(0),
        data_offset,
        data_len,
    })
}

/// How many bytes of audio follow `data_offset`.
///
/// Uncompressed audio knows exactly, which is worth taking: a WAV or AIFF can
/// carry tag chunks *after* its sample data, and including those would move the
/// content hash on a re-tag — the one thing this region exists to prevent.
/// Everything else runs to the end of the file, minus any trailing tag block.
fn region_len(
    path: &Path,
    probe: &AudioProbe,
    params: &CodecParameters,
    data_offset: u64,
    file_len: u64,
) -> u64 {
    let available = file_len.saturating_sub(data_offset);
    if let Some(exact) = pcm_len(probe, params) {
        return exact.min(available);
    }
    available.saturating_sub(trailing_tag_bytes(path, file_len))
}

fn pcm_len(probe: &AudioProbe, params: &CodecParameters) -> Option<u64> {
    if !probe.codec.starts_with("pcm_") {
        return None;
    }
    let bits = u64::from(params.bits_per_sample.or(params.bits_per_coded_sample)?);
    if bits == 0 || bits % 8 != 0 {
        return None;
    }
    Some(probe.n_frames? * u64::from(probe.channels) * (bits / 8))
}

/// Bytes of tag at the *end* of the file.
///
/// ID3v1 and APEv2 both live there, and a re-tag that adds or rewrites one
/// would otherwise shift the hashed tail window and invalidate every cached
/// analysis for the file. Best-effort by design: an unreadable tail is zero
/// trailing bytes, not an error.
fn trailing_tag_bytes(path: &Path, file_len: u64) -> u64 {
    let Ok(mut file) = File::open(path) else {
        return 0;
    };
    let mut total = 0u64;

    if let Some(start) = file_len.checked_sub(128) {
        if read_at(&mut file, start, 3).as_deref() == Some(b"TAG".as_slice()) {
            total += 128;
        }
    }
    // An APEv2 footer sits before any ID3v1 tag, so it is looked for behind
    // whatever the first check already claimed.
    if let Some(start) = file_len.checked_sub(total + 32) {
        if let Some(footer) = read_at(&mut file, start, 32) {
            if footer.starts_with(b"APETAGEX") {
                let size = u32::from_le_bytes([footer[12], footer[13], footer[14], footer[15]]);
                let flags = u32::from_le_bytes([footer[16], footer[17], footer[18], footer[19]]);
                let header = if flags & 0x8000_0000 != 0 { 32 } else { 0 };
                total += u64::from(size) + header;
            }
        }
    }
    total.min(file_len)
}

fn read_at(file: &mut File, offset: u64, len: usize) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf).ok()?;
    Some(buf)
}
